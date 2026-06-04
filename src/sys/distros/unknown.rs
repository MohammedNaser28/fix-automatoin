use crate::sys::distros::Distro;
use std::path::Path;

pub struct UnknownDistro {
    pub detected_name: String,
}

impl UnknownDistro {
    /// Attempts to parse a clean display name from os-release, falling back to a generic string.
    pub fn new(target_mount: &Path) -> Self {
        let os_release_path = target_mount.join("etc/os-release");
        let mut name = "Generic Linux Environment".to_string();

        if let Ok(content) = std::fs::read_to_string(os_release_path) {
            for line in content.lines() {
                if line.starts_with("NAME=") {
                    // Turn NAME="Void Linux" into Void Linux
                    let raw_name = line.replace("NAME=", "");
                    name = raw_name.trim_matches('"').to_string();
                    break;
                }
            }
        }

        UnknownDistro {
            detected_name: name,
        }
    }
}

impl Distro for UnknownDistro {
    fn name(&self) -> &'static str {
        // We leverage the runtime-allocated String safely via a leaked reference
        // to conform to the static trait lifetime signature requirement
        Box::leak(self.detected_name.clone().into_boxed_str())
    }

    fn grub_config_path(&self) -> &Path {
        // Standard vanilla upstream GNU GRUB default directory target location
        Path::new("/boot/grub/grub.cfg")
    }

    fn initramfs_cmd(&self) -> Vec<&'static str> {
        // Because initramfs generation completely diverges on niche distros
        // (Alpine uses mkinitfs, Gentoo uses genkernel, Void uses dracut),
        // we return an empty vector here to intentionally skip this stage
        // rather than executing a destructive guess.
        vec![]
    }

    fn post_grub_hook(&self, _chroot_path: &Path) -> std::io::Result<()> {
        // No-op for generic engines
        Ok(())
    }
}
