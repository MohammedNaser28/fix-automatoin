use crate::sys::distros::Distro;
use std::path::Path;
use std::process::Command;

pub struct Debian;

impl Distro for Debian {
    fn name(&self) -> &'static str {
        "Debian/Ubuntu Family"
    }

    fn grub_config_path(&self) -> &Path {
        Path::new("/boot/grub/grub.cfg")
    }

    fn initramfs_cmd(&self) -> Vec<&'static str> {
        vec!["update-initramfs", "-u", "-k", "all"]
    }

    fn post_grub_hook(&self, chroot_path: &Path) -> std::io::Result<()> {
        // Debian expects update-grub to run to synchronize configurations safely
        let status = Command::new("chroot")
            .arg(chroot_path)
            .args(["update-grub"])
            .status()?;

        if !status.success() {
            return Err(std::io::Error::other("update-grub failed"));
        }
        Ok(())
    }
}
