#!/bin/sh
# build.sh — Build Alpine-based fix-automation bootable ISO.
#
# Prerequisites:
#   - The fix-automation binary must already be compiled at
#     target/<arch>-unknown-linux-musl/release/fix-automation
#   - Must be run as root (for mknod in strip-rootfs.sh)
#   - Dependencies: wget, cpio, gzip (or zstd), xorriso, grub2
#
# Usage:
#   sudo bash dist/alpine/build.sh [--arch x86_64] [--output-dir dist/alpine/output]
#
set -eu

# ── Config ────────────────────────────────────────────────────────────────────
ARCH="${ARCH:-x86_64}"
OUTPUT_DIR="${OUTPUT_DIR:-dist/alpine/output}"
STAGING_DIR="/tmp/fix-automation-alpine-$$"
ALPINE_VERSION="${ALPINE_VERSION:-3.21}"
ALPINE_FULL="${ALPINE_VERSION}.0"
BINARY_PATH="${BINARY_PATH:-target/${ARCH}-unknown-linux-musl/release/fix-automation}"
GRUB_CFG="${GRUB_CFG:-dist/alpine/grub.cfg}"
STRIP_SCRIPT="${STRIP_SCRIPT:-dist/alpine/strip-rootfs.sh}"
VMLINUZ_PATH="${VMLINUZ_PATH:-/vmlinuz}"

cleanup() { rm -rf "$STAGING_DIR"; }
trap cleanup EXIT

info()  { echo -e "[INFO]  $*"; }
ok()    { echo -e "[OK]    $*"; }
warn()  { echo -e "[WARN]  $*"; }
err()   { echo -e "[ERR]   $*"; exit 1; }

numfmt_to_iec() {
    local bytes=$1
    if [ "$bytes" -ge 1073741824 ]; then
        echo "$((bytes / 1073741824)).$(((bytes % 1073741824) / 107374182))G"
    elif [ "$bytes" -ge 1048576 ]; then
        echo "$((bytes / 1048576)).$(((bytes % 1048576) / 104857))M"
    elif [ "$bytes" -ge 1024 ]; then
        echo "$((bytes / 1024))K"
    else
        echo "${bytes}B"
    fi
}

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
sh "$STRIP_SCRIPT" "$STAGING_DIR/rootfs"

# ══════════════════════════════════════════════════════════════════════════════
# Step 3: Add fix-automation binary
# ══════════════════════════════════════════════════════════════════════════════
info "Adding fix-automation binary ..."
cp "$BINARY_PATH" "$STAGING_DIR/rootfs/fix-automation"
chmod 755 "$STAGING_DIR/rootfs/fix-automation"

# Strip again (binary already stripped by Cargo profile, belt-and-suspenders)
strip "$STAGING_DIR/rootfs/fix-automation" 2>/dev/null || true

# ══════════════════════════════════════════════════════════════════════════════
# Step 4: Add minimal /etc files
# ══════════════════════════════════════════════════════════════════════════════
info "Adding minimal /etc files ..."
# /etc/passwd — single root entry
echo "root:x:0:0:root:/:/fix-automation" > "$STAGING_DIR/rootfs/etc/passwd"
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
ok "Initramfs: $(numfmt_to_iec $INITRAMFS_SIZE)"

# ══════════════════════════════════════════════════════════════════════════════
# Step 6: Get kernel (pre-installed by Dockerfile at /vmlinuz)
# ══════════════════════════════════════════════════════════════════════════════
info "Fetching kernel ..."
if [ -f "$VMLINUZ_PATH" ]; then
    cp "$VMLINUZ_PATH" "$STAGING_DIR/vmlinuz"
    ok "Kernel: $(basename "$VMLINUZ_PATH") ($(numfmt_to_iec $(stat -c%s "$STAGING_DIR/vmlinuz")))"
elif [ -f "/boot/vmlinuz-lts" ]; then
    cp "/boot/vmlinuz-lts" "$STAGING_DIR/vmlinuz"
    ok "Kernel: vmlinuz-lts"
elif [ -f "/boot/vmlinuz-linux" ]; then
    cp "/boot/vmlinuz-linux" "$STAGING_DIR/vmlinuz"
    ok "Kernel: vmlinuz-linux"
else
    warn "No kernel found at $VMLINUZ_PATH or /boot/vmlinuz-*"
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

# Produce EFI bootloader image via grub-mkstandalone (UEFI only)
info "  Creating EFI boot image ..."
grub-mkstandalone \
    --format=x86_64-efi \
    --output="$ISO_DIR/boot/grub/bootx64.efi" \
    --modules="part_gpt part_msdos fat iso9660 linux normal configfile search serial terminal" \
    "boot/grub/grub.cfg=$GRUB_CFG"

# Also create standard UEFI fallback path (some firmware scans for this)
mkdir -p "$ISO_DIR/EFI/BOOT"
cp "$ISO_DIR/boot/grub/bootx64.efi" "$ISO_DIR/EFI/BOOT/BOOTX64.EFI"

# Build UEFI-only ISO with xorriso
OUTPUT_ISO="${OUTPUT_DIR}/fix-automation-${ARCH}-alpine.iso"
info "  Running xorriso ..."
xorriso -as mkisofs \
    -iso-level 3 -rock -joliet \
    -eltorito-alt-boot \
    -e boot/grub/bootx64.efi \
    -no-emul-boot \
    -volid "FIX_AUTOMATION" \
    -o "$OUTPUT_ISO" \
    "$ISO_DIR"

# ══════════════════════════════════════════════════════════════════════════════
# Done
# ══════════════════════════════════════════════════════════════════════════════
ISO_SIZE=$(stat -c%s "$OUTPUT_ISO")
ok "ISO: $(numfmt_to_iec $ISO_SIZE)"
echo ""
echo "──────────────────────────────────────────────"
echo "  Output: $OUTPUT_ISO"
echo "  Size:   $(numfmt_to_iec $ISO_SIZE)"
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
