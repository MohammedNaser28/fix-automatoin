use crate::sys::chroot;
use crate::sys::distros::Distro;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
pub enum BootType {
    Efi { efi_mount_inside_chroot: String },
    Bios { target_disk: String },
}

// TODO: Check Os-Prober and enable it
pub fn ensure_os_prober_enabled(chroot_path: &Path, distro: &dyn Distro) -> std::io::Result<()> {
    let relative_path = distro.default_grub_file_path();
    let default_grub_path = chroot_path.join(relative_path);

    if !default_grub_path.exists() {
        if let Some(parent) = default_grub_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let baseline_config = concat!(
            "# Generated programmatically by Fix-Automation rescue tool\n",
            "GRUB_TIMEOUT=5\n",
            "GRUB_DISTRIBUTOR=\"Linux\"\n",
            "GRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash\"\n",
            "GRUB_DISABLE_OS_PROBER=false\n"
        );
        return fs::write(&default_grub_path, baseline_config);
    }

    let content = fs::read_to_string(&default_grub_path)?;
    if content.contains("GRUB_DISABLE_OS_PROBER=false") {
        return Ok(());
    }

    if content.contains("GRUB_DISABLE_OS_PROBER=true") {
        let updated_content = content.replace(
            "GRUB_DISABLE_OS_PROBER=true",
            "GRUB_DISABLE_OS_PROBER=false",
        );
        return fs::write(&default_grub_path, updated_content);
    }

    let mut file = OpenOptions::new().append(true).open(&default_grub_path)?;
    writeln!(
        file,
        "\n# Enabled programmatically by Fix-Automation rescue tool"
    )?;
    writeln!(file, "GRUB_DISABLE_OS_PROBER=false")?;
    Ok(())
}
pub fn check_presence_of_grub(chroot_path: &Path, distro: &dyn Distro) -> std::io::Result<()> {
    let install_bin = format!("usr/bin/{}", distro.grub_install_bin());
    let mkconfig_bin = format!("usr/bin/{}", distro.grub_mkconfig_bin());
    let path_to_check_presence = [install_bin, mkconfig_bin];
    for binary_path in path_to_check_presence.iter() {
        if !chroot_path.join(binary_path).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Required binary '{}' is missing inside the target environment.",
                    binary_path
                ),
            ));
        }
    }
    Ok(())
}

pub fn execute_grub_repair(
    chroot_path: &Path,
    distro: &dyn Distro,
    boot_type: &BootType,
) -> std::io::Result<()> {
    check_presence_of_grub(chroot_path, distro)?;

    let img_args = distro.initramfs_cmd();
    if !img_args.is_empty() {
        let status = chroot::run_in_chroot(chroot_path, img_args[0], &img_args[1..])?;
        if !status.success() {
            return Err(std::io::Error::other("Initramfs regeneration failed"));
        }
    }

    let install_bin = distro.grub_install_bin();
    let install_status = match boot_type {
        BootType::Efi {
            efi_mount_inside_chroot,
        } => chroot::run_in_chroot(
            chroot_path,
            install_bin,
            &[
                "--target=x86_64-efi",
                &format!("--efi-directory={}", efi_mount_inside_chroot),
                "--bootloader-id=GRUB",
                "--recheck",
            ],
        )?,
        BootType::Bios { target_disk } => chroot::run_in_chroot(
            chroot_path,
            install_bin,
            &["--target=i386-pc", target_disk, "--recheck"],
        )?,
    };

    if !install_status.success() {
        return Err(std::io::Error::other(format!(
            "{} execution failed",
            install_bin
        )));
    }

    ensure_os_prober_enabled(chroot_path, distro)?;

    let relative_cfg = distro.grub_config_path();
    let cfg_str = relative_cfg.to_str().unwrap_or("/boot/grub/grub.cfg");
    let mkconfig_bin = distro.grub_mkconfig_bin();

    let status = chroot::run_in_chroot(chroot_path, mkconfig_bin, &["-o", cfg_str])?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "{} execution failed",
            mkconfig_bin
        )));
    }

    distro.post_grub_hook(chroot_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::distros::arch::ArchLinux;

    fn write_default_grub(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("etc/default/grub");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
        dir
    }

    fn read_default_grub(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(dir.path().join("etc/default/grub")).unwrap()
    }

    #[test]
    fn os_prober_creates_baseline_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        ensure_os_prober_enabled(dir.path(), &ArchLinux).unwrap();
        let content = read_default_grub(&dir);
        assert!(content.contains("GRUB_DISABLE_OS_PROBER=false"));
    }

    #[test]
    fn os_prober_flips_true_to_false() {
        let dir = write_default_grub("GRUB_TIMEOUT=5\nGRUB_DISABLE_OS_PROBER=true\n");
        ensure_os_prober_enabled(dir.path(), &ArchLinux).unwrap();
        let content = read_default_grub(&dir);
        assert!(content.contains("GRUB_DISABLE_OS_PROBER=false"));
        assert!(!content.contains("GRUB_DISABLE_OS_PROBER=true"));
        // Existing settings preserved
        assert!(content.contains("GRUB_TIMEOUT=5"));
    }

    #[test]
    fn os_prober_appends_when_absent() {
        let dir = write_default_grub("GRUB_TIMEOUT=5\n");
        ensure_os_prober_enabled(dir.path(), &ArchLinux).unwrap();
        let content = read_default_grub(&dir);
        assert!(content.contains("GRUB_TIMEOUT=5"));
        assert!(content.contains("GRUB_DISABLE_OS_PROBER=false"));
    }

    #[test]
    fn os_prober_noop_when_already_enabled() {
        let dir = write_default_grub("GRUB_DISABLE_OS_PROBER=false\n");
        ensure_os_prober_enabled(dir.path(), &ArchLinux).unwrap();
        assert_eq!(read_default_grub(&dir), "GRUB_DISABLE_OS_PROBER=false\n");
    }

    #[test]
    fn presence_check_passes_when_bins_exist() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("usr/bin")).unwrap();
        std::fs::write(dir.path().join("usr/bin/grub-install"), "").unwrap();
        std::fs::write(dir.path().join("usr/bin/grub-mkconfig"), "").unwrap();
        assert!(check_presence_of_grub(dir.path(), &ArchLinux).is_ok());
    }

    #[test]
    fn presence_check_fails_on_missing_mkconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("usr/bin")).unwrap();
        std::fs::write(dir.path().join("usr/bin/grub-install"), "").unwrap();
        let err = check_presence_of_grub(dir.path(), &ArchLinux).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn presence_check_fails_when_nothing_exists() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_presence_of_grub(dir.path(), &ArchLinux).is_err());
    }
}
