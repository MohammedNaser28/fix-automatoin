use std::fs;
use std::path::Path;
#[cfg(not(feature = "alpine"))]
use std::process::Command;

const MOUNT_PATH: &str = "/mnt";
const MOUNT_EFI_PATH: &str = "/mnt/boot/efi";

pub fn mount(device: &str) {
    let target = Path::new(MOUNT_PATH);
    if !target.exists() {
        fs::create_dir_all(target).expect("failed to create /mnt");
    }

    #[cfg(feature = "alpine")]
    nix::mount::mount::<str, str, str, str>(
        Some(device),
        MOUNT_PATH,
        None::<&str>,
        nix::mount::MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .expect("mount failed");

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("mount")
            .args([device, MOUNT_PATH])
            .status()
            .expect("failed to execute mount");
        if !status.success() {
            panic!("failed to mount {} to {}", device, MOUNT_PATH);
        }
    }
}

pub fn mount_efi(efi_device: &str) {
    let efi_path = Path::new(MOUNT_EFI_PATH);
    if !efi_path.exists() {
        fs::create_dir_all(efi_path).expect("failed to create efi directory");
    }

    #[cfg(feature = "alpine")]
    nix::mount::mount::<str, str, str, str>(
        Some(efi_device),
        MOUNT_EFI_PATH,
        None::<&str>,
        nix::mount::MsFlags::empty(),
        None::<&str>,
    )
    .expect("mount efi failed");

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("mount")
            .args([efi_device, MOUNT_EFI_PATH])
            .status()
            .expect("failed to execute mount for EFI");
        if !status.success() {
            panic!("failed to mount EFI {} to {}", efi_device, MOUNT_EFI_PATH);
        }
    }
}

pub fn mount_bind() {
    let binds = ["/dev", "/proc", "/run", "/sys"];

    #[cfg(feature = "alpine")]
    for bind in &binds {
        let target = format!("{}{}", MOUNT_PATH, bind);
        nix::mount::mount::<str, str, str, str>(
            Some(bind),
            &target,
            None::<&str>,
            nix::mount::MsFlags::MS_BIND | nix::mount::MsFlags::MS_REC,
            None::<&str>,
        )
        .expect("bind mount failed");
    }

    #[cfg(not(feature = "alpine"))]
    for bind in binds {
        let target = format!("{}{}", MOUNT_PATH, bind);
        let status = Command::new("mount")
            .args(["--bind", bind, &target])
            .status()
            .expect("failed to execute mount --bind");
        if !status.success() {
            panic!("failed to bind mount {} to {}", bind, target);
        }
    }
}

pub fn umount(mount_dir: &str) {
    #[cfg(feature = "alpine")]
    {
        // Recursive unmount by parsing /proc/mounts and unmounting in reverse order
        let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
        let mut submounts: Vec<&str> = mounts
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let mp = parts[1];
                    if mp.starts_with(mount_dir) && mp != mount_dir {
                        Some(mp)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        // Unmount deepest first
        submounts.sort_by(|a, b| b.len().cmp(&a.len()));
        for mp in &submounts {
            let _ = nix::mount::umount(Path::new(mp));
        }
        nix::mount::umount(mount_dir).expect("umount failed");
    }

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("umount")
            .args(["-R", mount_dir])
            .status()
            .expect("failed to execute umount");
        if !status.success() {
            panic!("failed to umount {}", mount_dir);
        }
    }
}
