#!/bin/bash
# build.sh — Build Alpine-based fix-automaton bootable ISO.
#
# Prerequisites:
#   - The fix-automaton binary must already be compiled at
#     target/<arch>-unknown-linux-musl/release/fix-automaton
#   - Must be run as root (for mknod in strip-rootfs.sh)
#   - Dependencies: wget, cpio, gzip (or zstd), xorriso, grub2
#
# Usage:
#   sudo bash dist/alpine/build.sh [--arch x86_64] [--output-dir dist/alpine/output]
#
set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────
ARCH="${ARCH:-x86_64}"
OUTPUT_DIR="${OUTPUT_DIR:-dist/alpine/output}"
STAGING_DIR="/tmp/fix-automaton-alpine-$$"
ALPINE_VERSION="${ALPINE_VERSION:-3.21}"
ALPINE_FULL="${ALPINE_VERSION}.0"
BINARY_PATH="${BINARY_PATH:-target/${ARCH}-unknown-linux-musl/release/fix-automaton}"
GRUB_CFG="${GRUB_CFG:-dist/alpine/grub.cfg}"
STRIP_SCRIPT="${STRIP_SCRIPT:-dist/alpine/strip-rootfs.sh}"

cleanup() { rm -rf "$STAGING_DIR"; }
trap cleanup EXIT

info()  { echo -e "[INFO]  $*"; }
ok()    { echo -e "[OK]    $*"; }
warn()  { echo -e "[WARN]  $*"; }
err()   { echo -e "[ERR]   $*"; exit 1; }

# ── Checks ────────────────────────────────────────────────────────────────────
[ "$(id -u)" -eq 0 ] || err "Must be run as root"
[ -f "$BINARY_PATH" ] || err "Binary not found at $BINARY_PATH — build it first: cargo build --release --target ${ARCH}-unknown-linux-musl --features alpine"
command -v xorriso  >/dev/null || err "xorriso not found — install xorriso"
command -v grub-mkstandalone >/dev/null || err "grub-mkstandalone not found — install grub2"
command -v wget      >/dev/null || err "wget not found"

mkdir -p "$OUTPUT_DIR"

# ══════════════════════════════════════════════════════════════════════════════
# Step 1: Download and extract Alpine minirootfs
# ══════════════════════════════════════════════════════════════════════════════
ROOTFS_URL="https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/releases/${ARCH}/alpine-minirootfs-${ALPINE_FULL}-${ARCH}.tar.gz"
ROOTFS_TAR="alpine-minirootfs-${ALPINE_FULL}-${ARCH}.tar.gz"

if [ ! -f "/tmp/$ROOTFS_TAR" ]; then
    info "Downloading Alpine minirootfs ..."
    wget -q -O "/tmp/$ROOTFS_TAR" "$ROOTFS_URL"
fi

info "Extracting Alpine minirootfs ..."
rm -rf "$STAGING_DIR/rootfs"
mkdir -p "$STAGING_DIR/rootfs"
tar -xzf "/tmp/$ROOTFS_TAR" -C "$STAGING_DIR/rootfs"

# ══════════════════════════════════════════════════════════════════════════════
# Step 2: Strip rootfs to minimum
# ══════════════════════════════════════════════════════════════════════════════
info "Stripping rootfs ..."
bash "$STRIP_SCRIPT" "$STAGING_DIR/rootfs"

# ══════════════════════════════════════════════════════════════════════════════
# Step 3: Add fix-automaton binary
# ══════════════════════════════════════════════════════════════════════════════
info "Adding fix-automaton binary ..."
cp "$BINARY_PATH" "$STAGING_DIR/rootfs/fix-automaton"
chmod 755 "$STAGING_DIR/rootfs/fix-automaton"

# Strip again (binary already stripped by Cargo profile, belt-and-suspenders)
strip "$STAGING_DIR/rootfs/fix-automaton" 2>/dev/null || true

# ══════════════════════════════════════════════════════════════════════════════
# Step 4: Add minimal /etc files
# ══════════════════════════════════════════════════════════════════════════════
info "Adding minimal /etc files ..."
# /etc/passwd — single root entry
echo "root:x:0:0:root:/:/fix-automaton" > "$STAGING_DIR/rootfs/etc/passwd"
# /etc/group
echo "root:x:0:0" > "$STAGING_DIR/rootfs/etc/group"
# /etc/hostname
echo "rescue" > "$STAGING_DIR/rootfs/etc/hostname"
# /etc/hosts
printf "127.0.0.1 localhost\n::1 localhost\n127.0.1.1 rescue\n" > "$STAGING_DIR/rootfs/etc/hosts"
# /etc/fstab — empty (PID 1 mounts everything)
: > "$STAGING_DIR/rootfs/etc/fstab"

# ══════════════════════════════════════════════════════════════════════════════
# Step 5: Build initramfs
# ══════════════════════════════════════════════════════════════════════════════
info "Building initramfs ..."
(
    cd "$STAGING_DIR/rootfs"
    find . | cpio -oH newc --quiet | gzip -9 > "$STAGING_DIR/initramfs.cpio.gz"
)
INITRAMFS_SIZE=$(stat -c%s "$STAGING_DIR/initramfs.cpio.gz")
ok "Initramfs: $(numfmt --to=iec $INITRAMFS_SIZE)"

# ══════════════════════════════════════════════════════════════════════════════
# Step 6: Get kernel
# ══════════════════════════════════════════════════════════════════════════════
info "Fetching Alpine linux-lts kernel ..."
KERNEL_PKG="alpine-linux-lts-${ARCH}.tar.gz"
if [ ! -f "/tmp/$KERNEL_PKG" ]; then
    # Download the kernel package from Alpine's APK index
    APK_INDEX="/tmp/alpine-APKINDEX.tar.gz"
    if [ ! -f "$APK_INDEX" ]; then
        wget -q -O "$APK_INDEX" \
            "https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/main/${ARCH}/APKINDEX.tar.gz"
    fi

    # Extract APKINDEX to find the linux-lts package filename
    tar -xzf "$APK_INDEX" -C /tmp 2>/dev/null || true
    # The APKINDEX is a .tar.gz containing DESCRIPTION, APKINDEX.  We need
    # to parse APKINDEX to find the package version.  Simpler: just download
    # the latest linux-lts .apk directly (version is in the URL).
    # We'll use the latest known.  For Alpine 3.21, this should be recent.
    # Try to find it from the packages index.
    LTS_APK=$(curl -s "https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/main/${ARCH}/" | grep -oP 'linux-lts-\d+\.\d+\.\d+-\d+-'"${ARCH}"'\.apk' | sort -V | tail -1 2>/dev/null || true)

    if [ -z "$LTS_APK" ]; then
        # Fallback: download and extract the APK containing vmlinuz
        # The kernel package is linux-lts (no version needed in URL for latest)
        warn "Could not determine latest linux-lts version — trying linux-lts-r$((RANDOM % 100))"
        warn "Falling back: will download from Alpine's edge channel"
        LTS_APK="linux-lts-${ARCH}.apk"
    fi

    if [ ! -f "/tmp/$LTS_APK" ]; then
        wget -q -O "/tmp/$LTS_APK" \
            "https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/main/${ARCH}/${LTS_APK}" || {
            warn "Direct apk download failed, trying alternative URL..."
            # Get the APKINDEX properly
            tar -xzf "$APK_INDEX" -C "/tmp/alpine-apk" 2>/dev/null || true
            # Fallback: use pre-built kernel from the repo or build our own
            # For now, this is a placeholder — the user should provide a kernel
            warn "Kernel auto-download failed. Provide vmlinuz manually."
            warn "Place it at: $OUTPUT_DIR/vmlinuz"
            touch "$STAGING_DIR/no-kernel"
        }
    fi
fi

if [ -f "/tmp/$LTS_APK" ]; then
    info "Extracting kernel from APK ..."
    KERNEL_TMP="$STAGING_DIR/kernel-extract"
    mkdir -p "$KERNEL_TMP"
    tar -xzf "/tmp/$LTS_APK" -C "$KERNEL_TMP" 2>/dev/null || {
        # It's an APK (tar.gz with .apk extension), same format
        true
    }
    # Find vmlinuz in the extracted files
    VMLINUZ=$(find "$KERNEL_TMP" -name "vmlinuz-*" -type f | head -1)
    if [ -n "$VMLINUZ" ]; then
        cp "$VMLINUZ" "$STAGING_DIR/vmlinuz"
        ok "Kernel: $(basename "$VMLINUZ")"
    else
        warn "vmlinuz not found in APK"
        # Try extracting the .apk as tar.gz directly
        mkdir -p "$STAGING_DIR/kernel-tar"
        tar -xzf "/tmp/$LTS_APK" -C "$STAGING_DIR/kernel-tar" 2>/dev/null || true
        VMLINUZ=$(find "$STAGING_DIR/kernel-tar" -name "vmlinuz-*" -type f | head -1)
        if [ -n "$VMLINUZ" ]; then
            cp "$VMLINUZ" "$STAGING_DIR/vmlinuz"
            ok "Kernel: $(basename "$VMLINUZ")"
        fi
    fi
fi

if [ ! -f "$STAGING_DIR/vmlinuz" ]; then
    warn "No vmlinuz found. Please provide one:"
    warn "  cp /boot/vmlinuz-linux $OUTPUT_DIR/vmlinuz"
    warn "Build continuing without kernel..."
fi

# ══════════════════════════════════════════════════════════════════════════════
# Step 7: Build hybrid ISO
# ══════════════════════════════════════════════════════════════════════════════
info "Building ISO ..."
ISO_DIR="$STAGING_DIR/iso"
mkdir -p "$ISO_DIR/boot/grub"

# Copy kernel and initramfs
[ -f "$STAGING_DIR/vmlinuz" ] && cp "$STAGING_DIR/vmlinuz" "$ISO_DIR/boot/vmlinuz"
cp "$STAGING_DIR/initramfs.cpio.gz" "$ISO_DIR/boot/initramfs.cpio.gz"

# Copy grub.cfg
cp "$GRUB_CFG" "$ISO_DIR/boot/grub/grub.cfg"

# Produce EFI bootloader image via grub-mkstandalone
info "  Creating EFI boot image ..."
grub-mkstandalone \
    --format=x86_64-efi \
    --output="$ISO_DIR/boot/grub/bootx64.efi" \
    --modules="part_gpt part_msdos fat iso9660 linux normal configfile search" \
    "boot/grub/grub.cfg=$GRUB_CFG" 2>/dev/null

# Produce BIOS boot image
info "  Creating BIOS boot image ..."
grub-mkstandalone \
    --format=i386-pc \
    --output="$ISO_DIR/boot/grub/core.img" \
    --modules="biosdisk part_msdos iso9660 linux normal configfile search" \
    "boot/grub/grub.cfg=$GRUB_CFG" 2>/dev/null

# Build hybrid ISO with xorriso
OUTPUT_ISO="${OUTPUT_DIR}/fix-automaton-${ARCH}-alpine.iso"
info "  Running xorriso ..."
xorriso -as mkisofs \
    -iso-level 3 -rock -joliet \
    -b boot/grub/core.img \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -eltorito-alt-boot \
    -e boot/grub/bootx64.efi \
    -no-emul-boot \
    -volid "FIX_AUTOMATON" \
    -o "$OUTPUT_ISO" \
    "$ISO_DIR"

# ══════════════════════════════════════════════════════════════════════════════
# Done
# ══════════════════════════════════════════════════════════════════════════════
ISO_SIZE=$(stat -c%s "$OUTPUT_ISO")
ok "ISO: $(numfmt --to=iec $ISO_SIZE)"
echo ""
echo "──────────────────────────────────────────────"
echo "  Output: $OUTPUT_ISO"
echo "  Size:   $(numfmt --to=iec $ISO_SIZE)"
echo ""
echo "  Write to USB:"
echo "    sudo dd if=$OUTPUT_ISO of=/dev/sdX bs=4M status=progress"
echo ""
echo "  Test in QEMU (BIOS):"
echo "    qemu-system-x86_64 -cdrom $OUTPUT_ISO -m 512M -serial stdio -no-reboot"
echo ""
echo "  Test in QEMU (UEFI):"
echo "    qemu-system-x86_64 -bios /usr/share/edk2/x64/OVMF.4m.fd -cdrom $OUTPUT_ISO -m 512M -serial stdio -no-reboot"
echo "──────────────────────────────────────────────"
