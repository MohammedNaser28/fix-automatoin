#!/bin/bash
# write-usb.sh — Write Alpine ISO to USB drive.
# Usage: write-usb.sh /path/to/fix-automaton-x86_64-alpine.iso
set -euo pipefail

ISO="${1:?Usage: $0 /path/to/fix-automaton-x86_64-alpine.iso}"

if [ ! -f "$ISO" ]; then
    echo "ERROR: $ISO not found"
    exit 1
fi

echo "Available block devices:"
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT | grep -E "(disk|NAME)"

echo ""
read -p "Enter target device (e.g. sdb, not sdb1): " DEV
TARGET="/dev/$DEV"

if [ ! -b "$TARGET" ]; then
    echo "ERROR: $TARGET is not a block device"
    exit 1
fi

echo "WARNING: $TARGET will be overwritten!"
read -p "Type 'yes' to continue: " CONFIRM
if [ "$CONFIRM" != "yes" ]; then
    echo "Aborted."
    exit 1
fi

echo "Writing $ISO to $TARGET ..."
sudo dd if="$ISO" of="$TARGET" bs=4M status=progress oflag=sync
sudo sync
echo "Done. Boot from $TARGET on your target machine."
