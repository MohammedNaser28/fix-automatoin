// TODO: Run Chroot to make the fix steps inside it
use std::path::Path;
use std::process::{Command, ExitStatus};

pub fn run_in_chroot(
    chroot_path: &Path,
    program: &str,
    args: &[&str],
) -> std::io::Result<ExitStatus> {
    Command::new("chroot")
        .arg(chroot_path)
        .arg(program)
        .args(args)
        .status()
}
