#[cfg(not(feature = "alpine"))]
use serde::Deserialize;
use std::collections::HashSet;
#[cfg(not(feature = "alpine"))]
use std::process::Command;

#[derive(Clone, Debug)]
pub struct DiskInfo {
    pub name: String,
    pub size: String,
    pub fstype: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub mountpoint: Option<String>,
    pub is_efi: bool,
    pub contents: Option<String>,
}

#[cfg(not(feature = "alpine"))]
#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

#[cfg(not(feature = "alpine"))]
#[derive(Deserialize)]
struct BlockDevice {
    name: String,
    size: String,
    fstype: Option<String>,
    label: Option<String>,
    uuid: Option<String>,
    mountpoint: Option<String>,
    children: Option<Vec<BlockDevice>>,
}

pub fn get_disks() -> Vec<DiskInfo> {
    #[cfg(feature = "alpine")]
    return get_disks_sysfs();

    #[cfg(not(feature = "alpine"))]
    get_disks_lsblk()
}

#[cfg(not(feature = "alpine"))]
fn get_disks_lsblk() -> Vec<DiskInfo> {
    let _ = Command::new("udevadm").args(["settle"]).output();

    let output = match Command::new("lsblk")
        .args(["--json", "-o", "NAME,SIZE,FSTYPE,LABEL,UUID,MOUNTPOINT"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("lsblk failed: {} - is util-linux installed?", e);
            return Vec::new();
        }
    };

    let decoded: LsblkOutput = match serde_json::from_slice(&output.stdout) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("lsblk JSON parse error: {}", e);
            eprintln!("raw output: {}", String::from_utf8_lossy(&output.stdout));
            return Vec::new();
        }
    };

    let mut seen = HashSet::new();
    let mut disks = Vec::new();

    for dev in decoded.blockdevices {
        let has_children = dev.children.as_ref().is_some_and(|c| !c.is_empty());

        if !has_children {
            let is_efi = dev.fstype.as_deref() == Some("vfat");
            if seen.insert(dev.name.clone()) {
                disks.push(DiskInfo {
                    name: dev.name,
                    size: dev.size,
                    fstype: dev.fstype,
                    label: dev.label,
                    uuid: dev.uuid,
                    mountpoint: dev.mountpoint,
                    is_efi,
                    contents: if is_efi {
                        Some("Scanning...".into())
                    } else {
                        None
                    },
                });
            }
            continue;
        }

        if let Some(partitions) = dev.children {
            for part in partitions {
                let is_efi = part.fstype.as_deref() == Some("vfat");
                if seen.insert(part.name.clone()) {
                    disks.push(DiskInfo {
                        name: part.name,
                        size: part.size,
                        fstype: part.fstype,
                        label: part.label,
                        uuid: part.uuid,
                        mountpoint: part.mountpoint,
                        is_efi,
                        contents: if is_efi {
                            Some("Scanning...".into())
                        } else {
                            None
                        },
                    });
                }
            }
        }
    }

    add_sysfs_fallback(&mut seen, &mut disks);
    disks
}

#[cfg(not(feature = "alpine"))]
fn add_sysfs_fallback(seen: &mut HashSet<String>, disks: &mut Vec<DiskInfo>) {
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if name.starts_with("loop")
                || name.starts_with("ram")
                || name.starts_with("zram")
                || name.starts_with("dm-")
                || seen.contains(&name)
            {
                continue;
            }
            seen.insert(name.clone());

            let size = read_sysfs_size(&name);

            disks.push(DiskInfo {
                name,
                size,
                fstype: None,
                label: None,
                uuid: None,
                mountpoint: None,
                is_efi: false,
                contents: None,
            });
        }
    }
}

#[cfg(not(feature = "alpine"))]
fn read_sysfs_size(name: &str) -> String {
    std::fs::read_to_string(format!("/sys/block/{name}/size"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|sectors| {
            let bytes = sectors * 512;
            if bytes >= 1_000_000_000_000 {
                format!("{:.1}T", bytes as f64 / 1_000_000_000_000.0)
            } else if bytes >= 1_000_000_000 {
                format!("{:.1}G", bytes as f64 / 1_000_000_000.0)
            } else {
                format!("{:.0}M", bytes as f64 / 1_000_000.0)
            }
        })
        .unwrap_or_default()
}

#[cfg(feature = "alpine")]
fn get_disks_sysfs() -> Vec<DiskInfo> {
    let mut disks = Vec::new();
    let mut seen = HashSet::new();

    // 1. Read /proc/partitions for device names and sizes
    let proc_partitions = std::fs::read_to_string("/proc/partitions").unwrap_or_default();
    for line in proc_partitions.lines().skip(2) {
        // Format: major minor blocks name
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[3].to_string();
        let blocks: u64 = parts[2].parse().unwrap_or(0);

        // Skip loop, ram, zram, dm-
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("dm-")
            || seen.contains(&name)
        {
            continue;
        }
        seen.insert(name.clone());

        let bytes = blocks * 1024; // /proc/partitions reports in 1K blocks
        let size = if bytes >= 1_000_000_000_000 {
            format!("{:.1}T", bytes as f64 / 1_000_000_000_000.0)
        } else if bytes >= 1_000_000_000 {
            format!("{:.1}G", bytes as f64 / 1_000_000_000.0)
        } else {
            format!("{:.0}M", bytes as f64 / 1_000_000.0)
        };

        let is_partition = name.chars().any(|c| c.is_ascii_digit());

        // For partitions, try to detect filesystem from /proc/mounts
        let (fstype, mountpoint, label, uuid, is_efi) = if is_partition {
            probe_partition(&name)
        } else {
            (None, None, None, None, false)
        };

        disks.push(DiskInfo {
            name,
            size,
            fstype,
            label,
            uuid,
            mountpoint,
            is_efi,
            contents: None,
        });
    }

    disks
}

#[cfg(feature = "alpine")]
fn probe_partition(
    name: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
) {
    let (fstype, mountpoint) = mount_info(name);
    let is_efi = fstype.as_deref() == Some("vfat");
    let label = read_first_line(&format!("/sys/block/{}/uevent", parent(name)))
        .or_else(|| Some(name.to_string()));
    let uuid = None; // blkid not available; skipped for now
    (fstype, mountpoint, label, uuid, is_efi)
}

#[cfg(feature = "alpine")]
fn mount_info(name: &str) -> (Option<String>, Option<String>) {
    // Parse /proc/mounts for this device
    let dev_path = format!("/dev/{}", name);
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    for line in mounts.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        if parts[0] == dev_path {
            return (Some(parts[2].to_string()), Some(parts[1].to_string()));
        }
    }
    (None, None)
}

#[cfg(feature = "alpine")]
fn parent(name: &str) -> &str {
    // nvme0n1p1 -> nvme0n1, sda1 -> sda
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.is_empty() { name } else { trimmed }
}

#[cfg(feature = "alpine")]
fn read_first_line(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
}
