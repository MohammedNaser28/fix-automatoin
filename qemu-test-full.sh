#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# qemu-test.sh — Test fix-automation in QEMU (Alpine & Buildroot)
# Usage:
#   ./qemu-test.sh alpine-setup — build Alpine ISO + create victim disk
#   ./qemu-test.sh alpine       — boot Alpine ISO (UEFI)
#   ./qemu-test.sh setup        — create USB image + victim disk (Buildroot)
#   ./qemu-test.sh uefi         — boot Buildroot USB with UEFI
#   ./qemu-test.sh bios         — boot Buildroot USB with BIOS
#   ./qemu-test.sh clean        — delete all test images
# ─────────────────────────────────────────────────────────────────────────────
set -e

# ── Config ────────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Project root: where Cargo.toml lives. Override via FIX_PROJECT env var.
if [ -n "$FIX_PROJECT" ]; then
    PROJECT_DIR="$FIX_PROJECT"
elif [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    PROJECT_DIR="$SCRIPT_DIR"
else
    # Walk up from SCRIPT_DIR looking for Cargo.toml
    PROJECT_DIR="$SCRIPT_DIR"
    while [ "$PROJECT_DIR" != "/" ]; do
        [ -f "$PROJECT_DIR/Cargo.toml" ] && break
        PROJECT_DIR="$(dirname "$PROJECT_DIR")"
    done
    [ -f "$PROJECT_DIR/Cargo.toml" ] || { echo "Cannot find project root (Cargo.toml)"; exit 1; }
fi
WORK_DIR="$PROJECT_DIR/qemu-test-images"
USB_IMG="$WORK_DIR/rescue-usb.img"
VICTIM_IMG="$WORK_DIR/victim-disk.img"
ZIP_FILE="$WORK_DIR/grub-rescue-usb.zip"
ALPINE_ISO="/tmp/fix-automation-x86_64-alpine.iso"
OVMF="/usr/share/edk2/x64/OVMF.4m.fd"
MEM="512M"
KVM=""
[[ -e /dev/kvm ]] && KVM="-cpu host -enable-kvm"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()    { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()      { echo -e "${GREEN}[OK]${NC}   $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()     { echo -e "${RED}[ERR]${NC}  $*"; exit 1; }

# ── Dependency check ─────────────────────────────────────────────────────────
check_deps() {
    local missing=()
    for cmd in qemu-system-x86_64 parted mkfs.vfat mkfs.ext4 losetup xorriso grub-mkstandalone; do
        command -v "$cmd" &>/dev/null || missing+=("$cmd")
    done
    [ ${#missing[@]} -eq 0 ] || err "Missing: ${missing[*]}\nInstall: sudo pacman -S qemu-full edk2-ovmf parted dosfstools e2fsprogs util-linux libisoburn mtools grub"
}

# ── Build Alpine ISO from source ─────────────────────────────────────────────
alpine_build_iso() {
    info "Building Alpine ISO from source ..."

    BINARY="$PROJECT_DIR/target/x86_64-unknown-linux-musl/release/fix-automation"
    ROOTFS="/tmp/build-alpine-iso/rootfs"
    STAGING="/tmp/iso-build-$$"

    [ -f "$BINARY" ] || err "Binary not found at $BINARY — run 'cargo build --release --target x86_64-unknown-linux-musl --features alpine' first"
    [ -d "$ROOTFS" ] || err "Rootfs not found at $ROOTFS — run 'sudo bash $PROJECT_DIR/dist/alpine/build.sh' first"

    cp "$BINARY" "$ROOTFS/fix-automation"
    strip "$ROOTFS/fix-automation" 2>/dev/null || true

    mkdir -p "$STAGING"
    (cd "$ROOTFS" && find . | cpio -oH newc --quiet | gzip -9 > "$STAGING/initramfs.cpio.gz")

    for k in /vmlinuz /tmp/build-alpine-iso/vmlinuz /boot/vmlinuz-lts /boot/vmlinuz-linux; do
        [ -f "$k" ] && { cp "$k" "$STAGING/vmlinuz"; break; }
    done
    [ -f "$STAGING/vmlinuz" ] || err "No kernel found at /vmlinuz or /boot/vmlinuz-*"

    ISO_DIR="$STAGING/iso"
    mkdir -p "$ISO_DIR/boot/grub"
    cp "$STAGING/vmlinuz" "$ISO_DIR/boot/vmlinuz"
    cp "$STAGING/initramfs.cpio.gz" "$ISO_DIR/boot/initramfs.cpio.gz"
    cp "$PROJECT_DIR/dist/alpine/grub.cfg" "$ISO_DIR/boot/grub/grub.cfg"

    grub-mkstandalone \
        --format=x86_64-efi \
        --output="$ISO_DIR/boot/grub/bootx64.efi" \
        --modules="part_gpt part_msdos fat iso9660 linux normal configfile search serial terminal relocator all_video gfxterm gfxmenu gfxmode video video_bochs" \
        "boot/grub/grub.cfg=$PROJECT_DIR/dist/alpine/grub.cfg" >/dev/null 2>&1

    EFI_IMG="$STAGING/efi.img"
    dd if=/dev/zero of="$EFI_IMG" bs=1M count=32 2>/dev/null
    mkfs.fat -F 16 "$EFI_IMG" >/dev/null 2>&1
    mmd -i "$EFI_IMG" ::EFI ::EFI/BOOT ::boot
    mmd -i "$EFI_IMG" ::boot/grub ::boot/grub/themes ::boot/grub/themes/yorha
    mcopy -i "$EFI_IMG" "$ISO_DIR/boot/grub/bootx64.efi" ::EFI/BOOT/BOOTX64.EFI
    mcopy -i "$EFI_IMG" "$ISO_DIR/boot/vmlinuz"           ::boot/vmlinuz
    mcopy -i "$EFI_IMG" "$ISO_DIR/boot/initramfs.cpio.gz"  ::boot/initramfs.cpio.gz
    THEME_DIR="$PROJECT_DIR/dist/alpine/theme"
    for f in "$THEME_DIR"/*; do
        mcopy -i "$EFI_IMG" "$f" ::boot/grub/themes/yorha/
    done
    cp "$EFI_IMG" "$ISO_DIR/boot/grub/efi.img"

    xorriso -as mkisofs \
        -iso-level 3 -rock -joliet \
        -eltorito-alt-boot \
        -e boot/grub/efi.img \
        -no-emul-boot \
        -volid "FIX_AUTOMATION" \
        -output "$ALPINE_ISO" \
        "$ISO_DIR" 2>&1 | grep -E "Written|completed"

    rm -rf "$STAGING"
    ok "Alpine ISO: $ALPINE_ISO ($(du -h "$ALPINE_ISO" | cut -f1))"
}

# ── Create shared victim disk ─────────────────────────────────────────────────
create_victim_disk() {
    [ -f "$VICTIM_IMG" ] && { warn "Victim disk already exists — skipping"; return; }

    info "Creating victim disk (4GB GPT with EFI + ext4 root) ..."
    qemu-img create -f raw "$VICTIM_IMG" 4G

    sudo losetup -fP "$VICTIM_IMG"
    LOOP=$(losetup -j "$VICTIM_IMG" | cut -d: -f1)

    sudo parted "$LOOP" --script \
        mklabel gpt \
        mkpart EFI  fat32  1MiB   513MiB \
        set 1 esp on \
        mkpart root ext4   513MiB 100%

    sudo partprobe "$LOOP"
    sleep 1

    sudo mkfs.vfat -F32 "${LOOP}p1"
    sudo mkfs.ext4 -F   "${LOOP}p2"

    local root_mnt="$WORK_DIR/victim-root"
    local efi_mnt="$WORK_DIR/victim-efi"
    mkdir -p "$root_mnt" "$efi_mnt"

    sudo mount "${LOOP}p2" "$root_mnt"
    sudo mkdir -p "$root_mnt"/{boot/efi,etc,proc,sys,dev,run,usr/bin,var}
    sudo mount "${LOOP}p1" "$efi_mnt"
    sudo mkdir -p "$efi_mnt/EFI/arch"

    printf 'ID=arch\nID_LIKE=arch\nPRETTY_NAME="Arch Linux"\n' \
        | sudo tee "$root_mnt/etc/os-release" > /dev/null

    printf '# broken fstab for testing\n/dev/sda2  /       ext4  defaults  0 1\n/dev/sda1  /boot/efi  vfat  defaults  0 2\n' \
        | sudo tee "$root_mnt/etc/fstab" > /dev/null

    sudo umount "$efi_mnt"
    sudo umount "$root_mnt"
    sudo losetup -d "$LOOP"
    ok "Victim disk ready: $VICTIM_IMG"
}

# ── Alpine setup ──────────────────────────────────────────────────────────────
cmd_alpine_setup() {
    check_deps
    alpine_build_iso
    create_victim_disk
    echo ""
    echo -e "${GREEN}Alpine test setup complete.${NC} Run:"
    echo -e "  ${CYAN}$0 alpine${NC} — boot Alpine ISO in QEMU"
}

# ── Alpine boot ───────────────────────────────────────────────────────────────
cmd_alpine_boot() {
    [ -f "$ALPINE_ISO" ]  || err "Alpine ISO not found at $ALPINE_ISO — run '$0 alpine-setup'"
    [ -f "$OVMF" ]         || err "OVMF not found at $OVMF — Install: sudo pacman -S edk2-ovmf"

    info "Booting Alpine ISO (UEFI) ..."
    info "Controls: Ctrl+A X = quit | Ctrl+A C = QEMU monitor"
    echo ""

    local qemu_args=(
        -bios "$OVMF"
        -cdrom "$ALPINE_ISO"
        -m "$MEM"
        -vga std
        -serial mon:stdio
        -no-reboot
    )
    [ -n "$KVM" ] && qemu_args+=($KVM)

    if [ -f "$VICTIM_IMG" ]; then
        qemu_args+=(-drive file="$VICTIM_IMG",format=raw,if=virtio,index=1)
    else
        warn "No victim disk found — test boot only (no disk detection)"
    fi

    qemu-system-x86_64 "${qemu_args[@]}"
}

# ── Alpine boot (headless, for CI/automation) ─────────────────────────────────
cmd_alpine_headless() {
    [ -f "$ALPINE_ISO" ]  || err "Alpine ISO not found at $ALPINE_ISO — run '$0 alpine-setup'"

    local qemu_args=(
        -bios "$OVMF"
        -cdrom "$ALPINE_ISO"
        -m "$MEM"
        -vga std
        -serial stdio
        -no-reboot
        -display egl-headless
    )

    if [ -f "$VICTIM_IMG" ]; then
        qemu_args+=(-drive file="$VICTIM_IMG",format=raw,if=virtio,index=1)
    fi

    qemu-system-x86_64 "${qemu_args[@]}"
}

# ── Legacy Buildroot commands ─────────────────────────────────────────────────

# (unchanged — find_zip, cmd_setup, cmd_uefi, cmd_bios, cmd_info, cmd_clean)

# ── Find the ZIP ──────────────────────────────────────────────────────────────
find_zip() {
    if [ -n "$2" ] && [ -f "$2" ]; then
        ZIP_FILE="$2"
        return
    fi
    local found
    found=$(find . -maxdepth 2 -name "grub-rescue-usb.zip" 2>/dev/null | head -1)
    if [ -n "$found" ]; then
        ZIP_FILE="$(realpath "$found")"
        ok "Found ZIP: $ZIP_FILE"
        return
    fi
    err "No grub-rescue-usb.zip found. Pass as argument: $0 setup /path/to/grub-rescue-usb.zip"
}

# ── Setup: build USB image + victim disk ─────────────────────────────────────
cmd_setup() {
    find_zip "$@"
    check_deps
    mkdir -p "$WORK_DIR"

    info "Creating rescue USB image (256MB GPT + EFI)..."
    qemu-img create -f raw "$USB_IMG" 256M

    sudo losetup -fP "$USB_IMG"
    USB_LOOP=$(losetup -j "$USB_IMG" | cut -d: -f1)

    sudo parted "$USB_LOOP" --script \
        mklabel gpt \
        mkpart EFI fat32 1MiB 100% \
        set 1 esp on

    sudo partprobe "$USB_LOOP"
    sleep 1

    sudo mkfs.vfat -F32 "${USB_LOOP}p1"

    info "Extracting ZIP to USB image..."
    local mnt="$WORK_DIR/usb-mnt"
    mkdir -p "$mnt"
    sudo mount "${USB_LOOP}p1" "$mnt"

    sudo unzip -o "$ZIP_FILE" -d "$mnt"

    if [ -f "$mnt/EFI/BOOT/grub.cfg" ]; then
        sudo sed -i 's/console=tty0/console=tty0 console=ttyS0,115200/' "$mnt/EFI/BOOT/grub.cfg"
    fi

    sudo sync

    info "USB image contents:"
    find "$mnt" -type f | sort | while read -r f; do
        echo "  $f ($(du -h "$f" | cut -f1))"
    done

    sudo umount "$mnt"
    sudo losetup -d "$USB_LOOP"
    ok "Rescue USB image ready: $USB_IMG"

    create_victim_disk
}

# ── UEFI boot ────────────────────────────────────────────────────────────────
cmd_uefi() {
    [ -f "$USB_IMG" ]    || err "USB image not found — run: $0 setup"
    [ -f "$VICTIM_IMG" ] || err "Victim disk not found — run: $0 setup"
    [ -f "$OVMF" ]       || err "OVMF not found at $OVMF\nInstall: sudo pacman -S edk2-ovmf"

    info "Booting in UEFI mode..."
    info "Controls: Ctrl+A X = quit | Ctrl+A C = QEMU monitor"
    echo ""

    qemu-system-x86_64 \
        -bios "$OVMF" \
        -drive file="$USB_IMG",format=raw,if=virtio,index=0 \
        -drive file="$VICTIM_IMG",format=raw,if=virtio,index=1 \
        -m "$MEM" \
        $KVM \
        -vga std \
        -serial mon:stdio \
        -no-reboot
}

# ── BIOS boot ────────────────────────────────────────────────────────────────
cmd_bios() {
    [ -f "$USB_IMG" ]    || err "USB image not found — run: $0 setup"
    [ -f "$VICTIM_IMG" ] || err "Victim disk not found — run: $0 setup"

    info "Booting in BIOS mode..."
    info "Controls: Ctrl+A X = quit | Ctrl+A C = QEMU monitor"
    echo ""

    qemu-system-x86_64 \
        -drive file="$USB_IMG",format=raw,if=virtio,index=0 \
        -drive file="$VICTIM_IMG",format=raw,if=virtio,index=1 \
        -m "$MEM" \
        -nographic \
        -serial mon:stdio \
        -no-reboot
}

# ── Clean ────────────────────────────────────────────────────────────────────
cmd_clean() {
    info "Cleaning up test images..."
    for img in "$USB_IMG" "$VICTIM_IMG"; do
        loop=$(losetup -j "$img" 2>/dev/null | cut -d: -f1)
        [ -n "$loop" ] && sudo losetup -d "$loop" && info "Detached $loop"
    done
    rm -rf "$WORK_DIR"
    ok "Cleaned."
}

# ── Info ─────────────────────────────────────────────────────────────────────
cmd_info() {
    echo -e "${CYAN}fix-automation QEMU test script${NC}"
    echo ""
    echo "Work directory: $WORK_DIR"
    echo "Victim disk:    $VICTIM_IMG ($([ -f "$VICTIM_IMG" ] && echo 'exists' || echo 'missing'))"
    echo "Alpine ISO:     $ALPINE_ISO ($([ -f "$ALPINE_ISO" ] && echo 'exists' || echo 'missing'))"
    echo "USB image:      $USB_IMG ($([ -f "$USB_IMG" ] && echo 'exists' || echo 'missing'))"
    echo "OVMF firmware:  $OVMF"
    echo ""
    echo "Alpine commands:"
    echo "  $0 alpine-setup       — build ISO + create victim disk"
    echo "  $0 alpine             — boot Alpine ISO (UEFI, graphical)"
    echo "  $0 alpine-headless    — boot Alpine ISO (headless, serial)"
    echo ""
    echo "Buildroot commands:"
    echo "  $0 setup [path.zip]   — extract USB ZIP + create victim disk"
    echo "  $0 uefi               — boot Buildroot USB (UEFI)"
    echo "  $0 bios               — boot Buildroot USB (BIOS)"
    echo ""
    echo "  $0 clean              — delete all test images"
    echo ""
    echo "Victim disk layout:"
    echo "  /dev/vdb1  512MB  vfat   EFI partition (EFI/arch/)"
    echo "  /dev/vdb2  3.5GB  ext4   Root (Arch Linux, broken fstab)"
}

# ── Main ─────────────────────────────────────────────────────────────────────
case "${1:-info}" in
    alpine-setup)    cmd_alpine_setup          ;;
    alpine)          cmd_alpine_boot           ;;
    alpine-headless) cmd_alpine_headless       ;;
    setup)           cmd_setup "$@"            ;;
    uefi)            cmd_uefi                  ;;
    bios)            cmd_bios                  ;;
    clean)           cmd_clean                 ;;
    info)            cmd_info                  ;;
    *)
        echo "Unknown command: $1"
        cmd_info
        exit 1
        ;;
esac
