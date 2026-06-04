import os
import sys
import psutil

def is_linux_removable(device_path):
    """Checks if a Linux block device is flagged as removable by the kernel."""
    try:
        # device_path is usually something like /dev/sdb1 or /dev/nvme0n1p1
        dev_name = os.path.basename(device_path)

        # Look for the device in /sys/class/block
        sys_path = f"/sys/class/block/{dev_name}"
        if not os.path.exists(sys_path):
            # If it's a partition (sdb1), find its parent disk (sdb)
            # Strip trailing numbers
            parent_name = ''.join([c for c in dev_name if not c.isdigit()])
            sys_path = f"/sys/class/block/{parent_name}"

        removable_file = os.path.join(sys_path, "removable")
        if os.path.exists(removable_file):
            with open(removable_file, "r") as f:
                return f.read().strip() == "1"
    except Exception:
        pass
    return False

def get_flash_drives():
    flash_drives = []
    partitions = psutil.disk_partitions(all=False)

    # Filesystems we want to ignore (internal Linux system stuff)
    ignored_fs = {'tmpfs', 'devtmpfs', 'proc', 'sysfs', 'squashfs', 'overlay'}

    for part in partitions:
        if not part.fstype or part.fstype.lower() in ignored_fs:
            continue

        is_removable = False

        # --- WINDOWS LOGIC ---
        if sys.platform.startswith("win"):
            import win32file
            try:
                drive_type = win32file.GetDriveType(part.device)
                if drive_type == win32file.DRIVE_REMOVABLE:
                    is_removable = True
            except Exception:
                if 'removable' in part.opts:
                    is_removable = True

        # --- LINUX LOGIC ---
        elif sys.platform.startswith("linux"):
            # 1. Check kernel "removable" flag
            if is_linux_removable(part.device):
                is_removable = True
            # 2. Fallback: Common flash drive filesystems if mounted to /mnt or /media
            elif part.fstype.lower() in {'vfat', 'exfat', 'ntfs', 'msdos'}:
                if "/mnt/" in part.mountpoint or "/media/" in part.mountpoint:
                    is_removable = True

        if is_removable:
            try:
                usage = psutil.disk_usage(part.mountpoint)
                total_gb = round(usage.total / (1024**3), 2)
                free_gb = round(usage.free / (1024**3), 2)
            except PermissionError:
                total_gb, free_gb = "Unknown", "Unknown"

            flash_drives.append({
                "device": part.device,
                "mountpoint": part.mountpoint,
                "filesystem": part.fstype,
                "total_size_gb": total_gb,
                "free_space_gb": free_gb
            })

    return flash_drives

def main():
    print("Scanning for connected Flash Drives...")
    drives = get_flash_drives()

    if not drives:
        print("No removable flash drives found.")
        return

    print(f"\nFound {len(drives)} flash drive(s):")
    print("-" * 60)

    for drive in drives:
        print(f"Device Path:  {drive['device']}")
        print(f"Mount Point:  {drive['mountpoint']}")
        print(f"File System:  {drive['filesystem']}")
        print(f"Total Size:   {drive['total_size_gb']} GB")
        print(f"Free Space:   {drive['free_space_gb']} GB")
        print("-" * 60)

if __name__ == "__main__":
    main()