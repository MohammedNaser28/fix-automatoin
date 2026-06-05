#!/bin/bash
set -e

# Setup working directories
WORK_DIR="/tmp/patch-rescue-usb"
sudo rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

# 1. Extract the old zip
unzip -q /mnt/vms/grub-rescue-usb/grub-rescue-usb.zip

# 2. Extract the rootfs with sudo to preserve device nodes
mkdir rootfs
cd rootfs
sudo zcat ../EFI/BOOT/rootfs.cpio.gz | sudo cpio -idm --quiet

# 3. Replace the symlink with the wrapper script
sudo rm -f sbin/init
sudo bash -c "cat << 'EOF' > sbin/init
#!/bin/sh
export PATH=/sbin:/bin:/usr/sbin:/usr/bin
export TERM=linux
echo \"Mounting virtual filesystems...\"
mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev
echo \"Starting Fix-Automation Rescue Environment...\"
/usr/bin/fix-automation
echo \"Exiting... rebooting system in 3 seconds.\"
sleep 3
reboot -f
EOF"
sudo chmod +x sbin/init

# 4. Repack the rootfs
sudo find . | sudo cpio -H newc -o --quiet | gzip -9 > ../EFI/BOOT/rootfs.cpio.gz

# 5. Clean up and recreate the zip
cd ..
sudo rm -rf rootfs
rm -f /mnt/vms/grub-rescue-usb/grub-rescue-usb.zip
zip -r /mnt/vms/grub-rescue-usb/grub-rescue-usb.zip .

echo "Successfully patched the ZIP file!"
