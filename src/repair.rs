use crate::app::{Action, LogLine};
use crate::sys::blkdev::DiskInfo;
/// Background repair thread — sends LogLine messages through `tx`.
/// All blocking system calls live here so the TUI remains responsive.
use std::sync::mpsc::Sender;

pub fn run(
    tx: Sender<LogLine>,
    action: Action,
    root: DiskInfo,
    efi: Option<DiskInfo>,
    is_uefi: bool,
) {
    macro_rules! send {
        ($line:expr) => {
            let _ = tx.send($line);
        };
    }

    // ── Step 1: Mount root ────────────────────────────────────────────────────
    send!(LogLine::step("mounting root partition"));
    let root_dev = format!("/dev/{}", root.name);
    crate::sys::mount::mount(&root_dev);
    send!(LogLine::ok(format!("mounted {} → /mnt", root_dev)));

    // ── Step 2: Mount EFI ─────────────────────────────────────────────────────
    if is_uefi && let Some(ref efi_disk) = efi {
        send!(LogLine::step("mounting EFI partition"));
        let efi_dev = format!("/dev/{}", efi_disk.name);
        crate::sys::mount::mount_efi(&efi_dev);
        send!(LogLine::ok(format!("mounted {} → /mnt/boot/efi", efi_dev)));
    }

    // ── Step 3: Bind mounts ───────────────────────────────────────────────────
    send!(LogLine::step("bind mounting /dev /proc /sys /run"));
    crate::sys::mount::mount_bind();
    send!(LogLine::ok("bind mounts ready"));

    // ── Step 4: Detect distro ─────────────────────────────────────────────────
    send!(LogLine::step("detecting distribution"));
    let distro = crate::sys::distros::detect(std::path::Path::new("/mnt"));
    send!(LogLine::ok(format!("detected: {}", distro.name())));

    // ── Steps 5+: Action-specific repair ─────────────────────────────────────
    match action {
        Action::FixGrub | Action::FixGrubAndFstab => {
            run_grub_repair(&tx, &*distro, is_uefi, efi.as_ref());
        }
        Action::FixFstab => {
            send!(LogLine::step("auditing /etc/fstab"));
            let live_devs = [crate::sys::fstab::LivePartition {
                path: format!("/dev/{}", root.name),
                uuid: root.uuid.unwrap_or_default(),
                fstype: root.fstype.unwrap_or_default(),
                current_mount: root.mountpoint,
            }];
            match crate::sys::fstab::FstabAuditor::audit_fstab(
                std::path::Path::new("/mnt"),
                &live_devs,
            ) {
                Ok(issues) if issues.is_empty() => {
                    send!(LogLine::ok("fstab validated — no issues found"));
                }
                Ok(issues) => {
                    send!(LogLine::warn(format!(
                        "found {} issues in fstab",
                        issues.len()
                    )));
                }
                Err(e) => {
                    send!(LogLine::error(format!("fstab audit failed: {}", e)));
                }
            }
        }
        Action::OpenChrootShell => {
            send!(LogLine::warn(
                "chroot shell — switch to TTY and run: chroot /mnt"
            ));
        }
        _ => {
            send!(LogLine::warn("this action is not yet implemented"));
        }
    }

    // ── Final: Unmount ────────────────────────────────────────────────────────
    send!(LogLine::step("unmounting all filesystems"));
    crate::sys::mount::umount("/mnt");
    send!(LogLine::ok("repair complete — safe to reboot"));

    send!(LogLine::done());
}

#[cfg_attr(feature = "alpine", allow(unused_variables))]
fn run_grub_repair(
    tx: &Sender<LogLine>,
    distro: &dyn crate::sys::distros::Distro,
    is_uefi: bool,
    efi: Option<&DiskInfo>,
) {
    macro_rules! send {
        ($line:expr) => {{
            let _ = tx.send($line);
        }};
    }

    send!(LogLine::step("running GRUB repair"));
    let chroot_path = std::path::Path::new("/mnt");

    let boot_type = if is_uefi {
        crate::sys::grub::BootType::Efi {
            efi_mount_inside_chroot: "/boot/efi".into(),
        }
    } else {
        crate::sys::grub::BootType::Bios {
            target_disk: "/dev/sda".into(),
        }
    };

    match crate::sys::grub::execute_grub_repair(chroot_path, distro, &boot_type) {
        Ok(()) => send!(LogLine::ok("GRUB repair completed")),
        Err(e) => send!(LogLine::error(format!("GRUB repair failed: {}", e))),
    }
}

#[cfg_attr(feature = "alpine", allow(unused_variables))]
pub fn run_diagnosis(
    tx: Sender<LogLine>,
    root: DiskInfo,
    efi: Option<DiskInfo>,
    is_uefi: bool,
    disks: Vec<DiskInfo>,
) {
    macro_rules! send {
        ($line:expr) => {
            let _ = tx.send($line);
        };
    }

    // ── Step 1: Mount root ────────────────────────────────────────────────────
    send!(LogLine::step("mounting root partition"));
    let root_dev = format!("/dev/{}", root.name);
    crate::sys::mount::mount(&root_dev);
    send!(LogLine::ok(format!("mounted {} → /mnt", root_dev)));

    // ── Step 2: Detect distro ─────────────────────────────────────────────────
    send!(LogLine::step("detecting distribution"));
    let distro = crate::sys::distros::detect(std::path::Path::new("/mnt"));
    send!(LogLine::ok(format!("detected: {}", distro.name())));

    // ── Step 3: Check GRUB ────────────────────────────────────────────────────
    send!(LogLine::step("checking GRUB installation"));
    let mut grub_broken = false;
    match crate::sys::grub::check_presence_of_grub(std::path::Path::new("/mnt"), &*distro) {
        Ok(_) => {
            send!(LogLine::ok("GRUB binaries found"));
        }
        Err(_) => {
            grub_broken = true;
            send!(LogLine::warn("GRUB binaries are missing or corrupted"));
        }
    }

    // ── Step 4: Audit fstab ───────────────────────────────────────────────────
    send!(LogLine::step("auditing /etc/fstab"));
    let mut fstab_broken = false;
    let live_devs: Vec<crate::sys::fstab::LivePartition> = disks
        .into_iter()
        .map(|d| crate::sys::fstab::LivePartition {
            path: format!("/dev/{}", d.name),
            uuid: d.uuid.unwrap_or_default(),
            fstype: d.fstype.unwrap_or_default(),
            current_mount: d.mountpoint,
        })
        .collect();

    match crate::sys::fstab::FstabAuditor::audit_fstab(std::path::Path::new("/mnt"), &live_devs) {
        Ok(issues) => {
            if issues.is_empty() {
                send!(LogLine::ok("fstab is valid"));
            } else {
                fstab_broken = true;
                send!(LogLine::warn(format!(
                    "found {} issues in fstab",
                    issues.len()
                )));
            }
        }
        Err(_) => {
            fstab_broken = true;
            send!(LogLine::warn("could not read /etc/fstab"));
        }
    }

    // ── Final: Analyze & Unmount ──────────────────────────────────────────────
    send!(LogLine::step("unmounting filesystems"));
    crate::sys::mount::umount("/mnt");
    send!(LogLine::ok("diagnosis complete"));

    let mut summary = Vec::new();
    let recommended_action = if grub_broken && fstab_broken {
        summary.push("Diagnosis: GRUB is missing and fstab contains errors.".to_string());
        Some(Action::FixGrubAndFstab)
    } else if grub_broken {
        summary.push("Diagnosis: GRUB installation is broken or missing.".to_string());
        Some(Action::FixGrub)
    } else if fstab_broken {
        summary.push("Diagnosis: /etc/fstab contains invalid UUIDs or mounts.".to_string());
        Some(Action::FixFstab)
    } else {
        summary.push("Diagnosis: No major issues found. Select an action manually.".to_string());
        None
    };

    send!(LogLine {
        kind: crate::app::LogKind::DiagnosisResult(summary, recommended_action),
        text: String::new(),
    });
    send!(LogLine::done());
}
