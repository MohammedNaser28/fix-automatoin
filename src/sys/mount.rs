use std::fs;
use std::path::Path;
#[cfg(not(feature = "alpine"))]
use std::process::Command;

const MOUNT_PATH: &str = "/mnt";
const MOUNT_EFI_PATH: &str = "/mnt/boot/efi";

pub fn mount(device: &str) -> std::io::Result<()> {
    let target = Path::new(MOUNT_PATH);
    if !target.exists() {
        fs::create_dir_all(target)?;
    }

    #[cfg(feature = "alpine")]
    nix::mount::mount::<str, str, str, str>(
        Some(device),
        MOUNT_PATH,
        None::<&str>,
        nix::mount::MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .map_err(|e| std::io::Error::other(format!("mount {device} -> {MOUNT_PATH}: {e}")))?;

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("mount").args([device, MOUNT_PATH]).status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "failed to mount {device} to {MOUNT_PATH}"
            )));
        }
    }
    Ok(())
}

pub fn mount_efi(efi_device: &str) -> std::io::Result<()> {
    let efi_path = Path::new(MOUNT_EFI_PATH);
    if !efi_path.exists() {
        fs::create_dir_all(efi_path)?;
    }

    #[cfg(feature = "alpine")]
    nix::mount::mount::<str, str, str, str>(
        Some(efi_device),
        MOUNT_EFI_PATH,
        None::<&str>,
        nix::mount::MsFlags::empty(),
        None::<&str>,
    )
    .map_err(|e| std::io::Error::other(format!("mount {efi_device} -> {MOUNT_EFI_PATH}: {e}")))?;

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("mount")
            .args([efi_device, MOUNT_EFI_PATH])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "failed to mount EFI {efi_device} to {MOUNT_EFI_PATH}"
            )));
        }
    }
    Ok(())
}

pub fn mount_bind() -> std::io::Result<()> {
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
        .map_err(|e| std::io::Error::other(format!("bind mount {bind} -> {target}: {e}")))?;
    }

    #[cfg(not(feature = "alpine"))]
    for bind in binds {
        let target = format!("{}{}", MOUNT_PATH, bind);
        let status = Command::new("mount")
            .args(["--bind", bind, &target])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "failed to bind mount {bind} to {target}"
            )));
        }
    }
    Ok(())
}

pub fn umount(mount_dir: &str) -> std::io::Result<()> {
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
        submounts.sort_by_key(|b| std::cmp::Reverse(b.len()));
        for mp in &submounts {
            let _ = nix::mount::umount(Path::new(mp));
        }
        nix::mount::umount(mount_dir)
            .map_err(|e| std::io::Error::other(format!("umount {mount_dir}: {e}")))?;
    }

    #[cfg(not(feature = "alpine"))]
    {
        let status = Command::new("umount").args(["-R", mount_dir]).status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "failed to umount {mount_dir}"
            )));
        }
    }
    Ok(())
}
