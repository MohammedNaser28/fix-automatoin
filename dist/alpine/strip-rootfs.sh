#!/bin/sh
# strip-rootfs.sh — Minimize Alpine minirootfs to bare minimum.
# Usage: strip-rootfs.sh /path/to/rootfs
# Must be run as root (for mknod).
set -eu

ROOTFS="${1:?Usage: $0 /path/to/rootfs}"

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: must be run as root (for mknod)"
    exit 1
fi

echo "Stripping rootfs at $ROOTFS ..."

# --- Remove package manager ---
rm -rf "$ROOTFS/sbin/apk" "$ROOTFS/usr/sbin/apk" "$ROOTFS/etc/apk"
find "$ROOTFS/usr/lib" -name "apk" -type d -exec rm -rf {} + 2>/dev/null || true

# --- Remove BusyBox symlinks (keep binary for module loading) ---
find "$ROOTFS/bin" "$ROOTFS/sbin" "$ROOTFS/usr/bin" "$ROOTFS/usr/sbin" -type l -delete 2>/dev/null || true

# --- Remove shared libraries (fully static binary) ---
find "$ROOTFS/lib" -maxdepth 1 -name "*.so*" -delete
find "$ROOTFS/lib" -maxdepth 1 -name "*.a" -delete
find "$ROOTFS/usr/lib" -name "*.so*" -delete 2>/dev/null || true
find "$ROOTFS/usr/lib" -name "*.a" -delete 2>/dev/null || true

# Verify lib/modules survived
if [ -d "$ROOTFS/lib/modules" ] && [ -n "$(ls -A "$ROOTFS/lib/modules" 2>/dev/null)" ]; then
    echo "  lib/modules preserved"
else
    echo "  WARNING: lib/modules is empty or missing — kernel modules will not be available"
fi

# --- Remove firmware (not needed in initramfs) ---
rm -rf "$ROOTFS/lib/firmware"

# --- Remove /var cruft ---
rm -rf "$ROOTFS/var/cache" "$ROOTFS/var/log" "$ROOTFS/var/tmp" \
       "$ROOTFS/var/empty" "$ROOTFS/var/lock"
mkdir -p "$ROOTFS/var/log"

# --- Remove /usr/share (locale, terminfo, zoneinfo, doc, man) ---
rm -rf "$ROOTFS/usr/share"

# --- Strip /etc ---
rm -f "$ROOTFS/etc/shadow" "$ROOTFS/etc/gshadow" "$ROOTFS/etc/shells" \
      "$ROOTFS/etc/securetty" "$ROOTFS/etc/profile" "$ROOTFS/etc/inittab" \
      "$ROOTFS/etc/issue" "$ROOTFS/etc/motd"
rm -rf "$ROOTFS/etc/init.d" "$ROOTFS/etc/conf.d" "$ROOTFS/etc/logrotate.d" \
       "$ROOTFS/etc/periodic" "$ROOTFS/etc/terminfo" "$ROOTFS/etc/ssl" \
       "$ROOTFS/etc/modprobe.d"
rm -f "$ROOTFS/etc/profile.d/*" "$ROOTFS/etc/modprobe.d/*" 2>/dev/null || true

# --- Remove OpenRC / runit / starterd ---
rm -f "$ROOTFS/sbin/openrc" "$ROOTFS/sbin/openrc-run" \
      "$ROOTFS/sbin/starterd" 2>/dev/null || true
rm -rf "$ROOTFS/etc/rc.conf" "$ROOTFS/etc/rc.d" "$ROOTFS/etc/runit" 2>/dev/null || true

# --- Remove stale /dev entries ---
rm -rf "$ROOTFS/dev/*" 2>/dev/null || true

# --- Remove static libraries and dev files ---
find "$ROOTFS/usr" -name "*.a" -delete
find "$ROOTFS/usr" -name "*.la" -delete
find "$ROOTFS/usr" -name "*.pc" -delete
find "$ROOTFS/usr" -name "*.h" -delete

# --- Create static /dev nodes (required before devtmpfs mounts) ---
mkdir -p "$ROOTFS/dev"
mknod -m 622 "$ROOTFS/dev/console" c 5 1
mknod -m 666 "$ROOTFS/dev/null"    c 1 3

echo "Stripping complete."
du -sh "$ROOTFS"
