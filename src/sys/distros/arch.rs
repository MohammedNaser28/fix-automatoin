use crate::sys::distros::Distro;
use std::path::Path;

pub struct ArchLinux;

impl Distro for ArchLinux {
    fn name(&self) -> &'static str {
        "Arch Linux"
    }

    fn grub_config_path(&self) -> &Path {
        Path::new("/boot/grub/grub.cfg")
    }

    fn initramfs_cmd(&self) -> Vec<&'static str> {
        vec!["mkinitcpio", "-P"]
    }

    fn post_grub_hook(&self, _chroot_path: &Path) -> std::io::Result<()> {
        // Arch does not use wrapper helper commands like update-grub
        Ok(())
    }
}
