#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# qemu-test.sh — Boot fix-automation Alpine ISO in QEMU with victim disk
# Usage:
#   ./qemu-test.sh /path/to/fix-automation-x86_64-alpine.iso
#   ./qemu-test.sh clean
# ─────────────────────────────────────────────────────────────────────────────
set -e

ISO="${1:-}"
CMD="${1:-info}"

WORK_DIR="$(dirname "$(realpath "$0")")/qemu-test-images"
VICTIM_IMG="$WORK_DIR/victim-disk.img"
OVMF="/usr/share/edk2/x64/OVMF.4m.fd"
MEM="512M"
KVM=""
[[ -e /dev/kvm ]] && KVM="-cpu host -enable-kvm"

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info() { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC}   $*"; }
err()  { echo -e "${RED}[ERR]${NC}  $*"; exit 1; }

check_deps() {
    local missing=()
    for cmd in qemu-system-x86_64 parted mkfs.vfat mkfs.ext4 losetup; do
        command -v "$cmd" &>/dev/null || missing+=("$cmd")
    done
    [ ${#missing[@]} -eq 0 ] || err "Missing: ${missing[*]}"
}

cmd_setup() {
    [ -f "$ISO" ] || err "ISO not found: $ISO"
    check_deps
    mkdir -p "$WORK_DIR"

    if [ -f "$VICTIM_IMG" ]; then
        warn "Victim disk already exists — skipping"
    else
        info "Creating victim disk (4GB GPT: EFI + ext4 root) ..."
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
        ok "Victim disk: $VICTIM_IMG"
    fi

    echo ""
    echo -e "${GREEN}Ready.${NC} Run:"
    echo -e "  ${CYAN}$(basename "$0") boot${NC}   — boot with victim disk"
}

cmd_boot() {
    [ -f "$ISO" ]        || err "ISO not found — run '$0 /path/to/iso' first"
    [ -f "$VICTIM_IMG" ] || warn "No victim disk — run '$0 /path/to/iso' to create one"
    [ -f "$OVMF" ]       || err "OVMF not found at $OVMF"

    local args=(
        -bios "$OVMF"
        -cdrom "$ISO"
        -m "$MEM"
        $KVM
        -vga std
        -serial mon:stdio
        -no-reboot
    )
    [ -f "$VICTIM_IMG" ] && args+=(-drive file="$VICTIM_IMG",format=raw,if=virtio,index=1)

    info "Booting Alpine ISO (UEFI) ..."
    info "Controls: Ctrl+A X = quit | Ctrl+A C = QEMU monitor"
    qemu-system-x86_64 "${args[@]}"
}

cmd_clean() {
    for img in "$VICTIM_IMG"; do
        loop=$(losetup -j "$img" 2>/dev/null | cut -d: -f1)
        [ -n "$loop" ] && sudo losetup -d "$loop" 2>/dev/null
    done
    rm -rf "$WORK_DIR"
    ok "Cleaned."
}

case "$CMD" in
    boot)  cmd_boot ;;
    clean) cmd_clean ;;
    *)
        [ -f "$ISO" ] || { echo "Usage: $0 /path/to/fix-automation-x86_64-alpine.iso"; echo "       $0 clean"; exit 1; }
        cmd_setup
        ;;
esac
