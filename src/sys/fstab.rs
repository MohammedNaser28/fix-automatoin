// TODO: Add the functionality for check fstype file and UUID and check it's the same to resolve resizing and editing problems
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum FstabIssue {
    UuidMismatch {
        mount_point: String,
        expected_uuid: String,
        live_uuid: Option<String>,
    },
    MissingSwap {
        live_swap_uuid: String,
    },
    UnusualFsType {
        mount_point: String,
        fstab_type: String,
        live_type: String,
    },
}

#[derive(Debug, Clone)]
pub struct LivePartition {
    pub path: String,
    pub uuid: String,
    pub fstype: String,
    pub current_mount: Option<String>,
}
pub struct FstabAuditor;

impl FstabAuditor {
    pub fn audit_fstab(
        chroot_path: &Path,
        live_devs: &[LivePartition],
    ) -> std::io::Result<Vec<FstabIssue>> {
        let fstab_path = chroot_path.join("etc/fstab");
        let mut issues = Vec::new();

        if !fstab_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "The target /etc/fstab configuration file is missing completely.",
            ));
        }

        let content = fs::read_to_string(&fstab_path)?;
        let mut checked_mount_points = Vec::new();

        // Parse and analyze existing configuration entries line-by-line
        for line in content.lines() {
            let line = line.trim();
            // Skip comments and blank spaces cleanly
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Split by arbitrary whitespace tokens (tabs/spaces)
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 3 {
                continue; // Malformed or unusual line entry format skipped safely
            }

            let fs_spec = fields[0]; // Device identifier: UUID=... or /dev/sda1
            let mount_point = fields[1].to_string(); // e.g., "/", "/boot/efi"
            let fstab_type = fields[2].to_string(); // e.g., "ext4", "swap"

            checked_mount_points.push(mount_point.clone());

            // Isolate UUID targets out of the spec definition column string
            let target_uuid = if fs_spec.starts_with("UUID=") {
                Some(
                    fs_spec
                        .replace("UUID=", "")
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                )
            } else if fs_spec.starts_with("/dev/") {
                // If the entry uses a raw node path, search our live table to locate its UUID footprint
                live_devs
                    .iter()
                    .find(|p| p.path == fs_spec)
                    .map(|p| p.uuid.clone())
            } else {
                None
            };

            if let Some(ref uuid) = target_uuid {
                // Look up matching live hardware based on the parsed UUID string
                let matching_live_partition = live_devs.iter().find(|p| p.uuid == *uuid);

                match matching_live_partition {
                    Some(live_part) => {
                        // CRITICAL CHECK A: Verify if filesystem types mismatch
                        if live_part.fstype != fstab_type && fstab_type != "none" {
                            issues.push(FstabIssue::UnusualFsType {
                                mount_point: mount_point.clone(),
                                fstab_type: fstab_type.clone(),
                                live_type: live_part.fstype.clone(),
                            });
                        }
                    }
                    None => {
                        // CRITICAL CHECK B: The configured UUID cannot be found on the physical system
                        // Let's check if another live partition claims this target mount point instead
                        let true_live_partition = live_devs
                            .iter()
                            .find(|p| p.current_mount.as_ref() == Some(&mount_point));

                        issues.push(FstabIssue::UuidMismatch {
                            mount_point: mount_point.clone(),
                            expected_uuid: uuid.clone(),
                            live_uuid: true_live_partition.map(|p| p.uuid.clone()),
                        });
                    }
                }
            }
        }

        // Check for unmapped Swap targets (System optimization audit)
        // Scan the live environment for any unassigned partitions labeled as 'swap'
        for live_part in live_devs {
            if live_part.fstype == "swap" {
                // If a physical swap partition exists but isn't explicitly declared, flag it
                let is_swap_in_fstab = checked_mount_points
                    .iter()
                    .any(|m| m == "none" || m == "swap");
                if !is_swap_in_fstab {
                    issues.push(FstabIssue::MissingSwap {
                        live_swap_uuid: live_part.uuid.clone(),
                    });
                }
            }
        }

        Ok(issues)
    }
}
