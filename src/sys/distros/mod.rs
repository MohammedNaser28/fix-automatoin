// src/sys/distro/mod.rs

use std::path::Path;

pub trait Distro {
    /// Returns the user-facing name of the distribution (e.g., "Arch Linux").
    fn name(&self) -> &'static str;

    /// Returns the absolute target path where the GRUB configuration file should live.
    fn grub_config_path(&self) -> &Path;

    /// Returns the specific binary and arguments required to rebuild the initramfs/initrd inside a chroot environment.
    fn initramfs_cmd(&self) -> Vec<&'static str>;

    /// Handles any unique logic required post-installation (e.g., executing update-grub on Debian variants).
    fn post_grub_hook(&self, chroot_path: &Path) -> std::io::Result<()>;

    fn grub_install_bin(&self) -> &'static str {
        "grub-install"
    }
    fn grub_mkconfig_bin(&self) -> &'static str {
        "grub-mkconfig"
    }

    fn default_grub_file_path(&self) -> &Path {
        Path::new("etc/default/grub")
    }
}

pub mod arch;
pub mod debian;
pub mod fedora;
pub mod unknown;

/// Inspects a mounted root partition filesystem's `/etc/os-release`
/// to dynamically identify the underlying distribution family.
pub fn detect(target_mount: &Path) -> Box<dyn Distro> {
    let os_release_path = target_mount.join("etc/os-release");

    if let Ok(content) = std::fs::read_to_string(os_release_path) {
        let mut id = String::new();
        let mut id_like = String::new();

        // Robust, lightweight line parser that strips shell quotes cleanly
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || !line.contains('=') {
                continue;
            }

            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                // Strip both single and double quotes, and normalize to lowercase
                let val = val
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_lowercase();

                if key == "ID" {
                    id = val;
                } else if key == "ID_LIKE" {
                    id_like = val;
                }
            }
        }

        // Match normalized distribution identifiers
        if id == "arch" || id_like.contains("arch") {
            return Box::new(arch::ArchLinux);
        } else if id == "debian" || id == "ubuntu" || id_like.contains("debian") {
            return Box::new(debian::Debian);
        } else if id == "fedora"
            || id == "rhel"
            || id_like.contains("fedora")
            || id_like.contains("rhel")
        {
            return Box::new(fedora::Fedora);
        }
    }

    // Fall back to the smart unknown handler if the file is missing, unreadable, or unrecognized
    Box::new(unknown::UnknownDistro::new(target_mount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::distros::arch::ArchLinux;
    use crate::sys::distros::debian::Debian;
    use crate::sys::distros::fedora::Fedora;

    fn write_os_release(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("etc")).unwrap();
        std::fs::write(dir.path().join("etc/os-release"), content).unwrap();
        dir
    }

    #[test]
    fn detect_arch_by_id() {
        let dir = write_os_release("ID=arch\nNAME=\"Arch Linux\"\n");
        let d = detect(dir.path());
        assert_eq!(d.name(), "Arch Linux");
        assert_eq!(d.initramfs_cmd(), vec!["mkinitcpio", "-P"]);
        assert_eq!(d.grub_install_bin(), "grub-install");
    }

    #[test]
    fn detect_arch_by_id_like() {
        // EndeavourOS / Manjaro style
        let dir = write_os_release("ID=endeavouros\nID_LIKE=\"arch\"\n");
        assert_eq!(detect(dir.path()).name(), "Arch Linux");
    }

    #[test]
    fn detect_debian_family() {
        for id in ["debian", "ubuntu"] {
            let dir = write_os_release(&format!("ID={id}\n"));
            assert_eq!(detect(dir.path()).name(), "Debian/Ubuntu Family", "id={id}");
        }
    }

    #[test]
    fn unrecognized_id_falls_back_to_unknown() {
        let dir = write_os_release("ID=linuxmint\n");
        assert_eq!(detect(dir.path()).name(), "Generic Linux Environment");
    }

    #[test]
    fn detect_debian_by_id_like() {
        let dir = write_os_release("ID=kali\nID_LIKE=\"debian\"\n");
        assert_eq!(detect(dir.path()).name(), "Debian/Ubuntu Family");
    }

    #[test]
    fn debian_uses_update_grub_hook_paths() {
        let d = Debian;
        assert_eq!(d.initramfs_cmd(), vec!["update-initramfs", "-u", "-k", "all"]);
        assert_eq!(
            d.grub_config_path(),
            Path::new("/boot/grub/grub.cfg")
        );
    }

    #[test]
    fn detect_fedora_family() {
        for id in ["fedora", "rhel"] {
            let dir = write_os_release(&format!("ID={id}\n"));
            assert_eq!(detect(dir.path()).name(), "Fedora Linux", "id={id}");
        }
    }

    #[test]
    fn fedora_uses_grub2_layout() {
        let d = Fedora;
        assert_eq!(d.grub_install_bin(), "grub2-install");
        assert_eq!(d.grub_mkconfig_bin(), "grub2-mkconfig");
        assert_eq!(
            d.grub_config_path(),
            Path::new("/boot/grub2/grub.cfg")
        );
        assert_eq!(
            d.initramfs_cmd(),
            vec!["dracut", "--regenerate-all", "--force"]
        );
    }

    #[test]
    fn missing_os_release_falls_back_to_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let d = detect(dir.path());
        assert_eq!(d.name(), "Generic Linux Environment");
        // Unknown distro must not guess a destructive initramfs command
        assert!(d.initramfs_cmd().is_empty());
    }

    #[test]
    fn unknown_distro_parses_name_from_release() {
        let dir = write_os_release("ID=nixos\nNAME=\"NixOS\"\n");
        let d = detect(dir.path());
        assert_eq!(d.name(), "NixOS");
        assert!(d.initramfs_cmd().is_empty());
    }

    #[test]
    fn quoted_values_parsed_case_insensitive() {
        let dir = write_os_release("ID=\"ARCH\"\n");
        assert_eq!(detect(dir.path()).name(), "Arch Linux");
    }

    #[test]
    fn comments_and_garbage_lines_ignored() {
        let dir = write_os_release("# comment\nNOEQUALSHERE\nID=arch\n");
        assert_eq!(detect(dir.path()).name(), "Arch Linux");
    }

    #[test]
    fn arch_post_grub_hook_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ArchLinux.post_grub_hook(dir.path()).is_ok());
    }
}
