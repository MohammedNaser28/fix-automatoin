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
