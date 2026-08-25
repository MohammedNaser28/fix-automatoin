#[cfg(not(feature = "alpine"))]
use serde::Deserialize;
use std::collections::HashSet;
#[cfg(not(feature = "alpine"))]
use std::process::Command;

/// GPT GUID of the EFI System Partition
const ESP_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";

/// Decides whether a partition is an EFI System Partition.
/// GPT: match by partition type GUID (reliable — a FAT data partition is not an ESP).
/// MBR (`0xef`) also counts. When the type is unknown (Alpine sysfs path), fall back
/// to the vfat heuristic.
pub fn is_esp(parttype: Option<&str>, fstype: Option<&str>) -> bool {
    match parttype {
        Some(t) => t.eq_ignore_ascii_case(ESP_GUID) || t.eq_ignore_ascii_case("0xef"),
        None => fstype == Some("vfat"),
    }
}

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
    parttype: Option<String>,
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
        .args(["--json", "-o", "NAME,SIZE,FSTYPE,PARTTYPE,LABEL,UUID,MOUNTPOINT"])
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
            let is_efi = is_esp(dev.parttype.as_deref(), dev.fstype.as_deref());
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
                let is_efi = is_esp(part.parttype.as_deref(), part.fstype.as_deref());
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

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000_000 {
        format!("{:.1}T", bytes as f64 / 1_000_000_000_000.0)
    } else if bytes >= 1_000_000_000 {
        format!("{:.1}G", bytes as f64 / 1_000_000_000.0)
    } else {
        format!("{:.0}M", bytes as f64 / 1_000_000.0)
    }
}

#[cfg(not(feature = "alpine"))]
fn read_sysfs_size(name: &str) -> String {
    std::fs::read_to_string(format!("/sys/block/{name}/size"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|sectors| format_size(sectors * 512))
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
        let size = format_size(bytes);

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
    // No partition-type GUID available on the sysfs path — vfat heuristic fallback
    let is_efi = is_esp(None, fstype.as_deref());
    let label = read_first_line(&format!("/sys/block/{}/uevent", parent(name)))
        .or_else(|| Some(name.to_string()));
    let uuid = uuid_for(name);
    (fstype, mountpoint, label, uuid, is_efi)
}

/// Resolves a partition's UUID by walking the `/dev/disk/by-uuid` symlinks.
/// Pure std — no blkid required. Matching logic lives in [`uuid_from_links`]
/// so it stays unit-testable without root or real devices.
#[cfg(feature = "alpine")]
fn uuid_for(name: &str) -> Option<String> {
    let entries = std::fs::read_dir("/dev/disk/by-uuid").ok()?;
    let links = entries.flatten().filter_map(|entry| {
        let uuid = entry.file_name().to_string_lossy().to_string();
        let target = std::fs::read_link(entry.path())
            .ok()?
            .file_name()?
            .to_string_lossy()
            .to_string();
        Some((uuid, target))
    });
    uuid_from_links(name, links)
}

#[cfg(feature = "alpine")]
fn uuid_from_links<I>(name: &str, mut links: I) -> Option<String>
where
    I: Iterator<Item = (String, String)>,
{
    links
        .find(|(_, dev)| dev == name)
        .map(|(uuid, _)| uuid)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_bytes() {
        // Decimal (SI) units, rounded to whole M
        assert_eq!(format_size(536_870_912), "537M");
        assert_eq!(format_size(0), "0M");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(500_000_000_000), "500.0G");
    }

    #[test]
    fn format_size_terabytes() {
        assert_eq!(format_size(2_000_000_000_000), "2.0T");
    }

    #[test]
    fn format_size_boundary_below_tb() {
        // Just under 1TB stays in G
        assert_eq!(format_size(999_999_999_999), "1000.0G");
    }

    #[test]
    fn disk_info_is_efi_only_for_vfat() {
        let d = DiskInfo {
            name: "sda1".into(),
            size: "1G".into(),
            fstype: Some("ext4".into()),
            label: None,
            uuid: None,
            mountpoint: None,
            is_efi: false,
            contents: None,
        };
        assert!(!d.is_efi);
    }

    #[test]
    fn esp_guid_is_detected_case_insensitive() {
        assert!(is_esp(
            Some("C12A7328-F81F-11D2-BA4B-00A0C93EC93B"),
            Some("vfat")
        ));
        assert!(is_esp(
            Some("c12a7328-f81f-11d2-ba4b-00a0c93ec93b"),
            Some("vfat")
        ));
    }

    #[test]
    fn mbr_efi_type_0xef_is_detected() {
        assert!(is_esp(Some("0xef"), Some("vfat")));
    }

    #[test]
    fn fat_data_partition_on_gpt_is_not_esp() {
        // FAT filesystem but wrong GPT type (e.g. Microsoft Basic Data)
        assert!(!is_esp(
            Some("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7"),
            Some("vfat")
        ));
    }

    #[test]
    fn unknown_parttype_falls_back_to_vfat_heuristic() {
        assert!(is_esp(None, Some("vfat")));
        assert!(!is_esp(None, Some("ext4")));
        assert!(!is_esp(None, None));
    }

    #[cfg(feature = "alpine")]
    #[test]
    fn uuid_matching_from_by_uuid_links() {
        let links = [
            ("aaaa-1111".to_string(), "sda1".to_string()),
            ("bbbb-2222".to_string(), "nvme0n1p2".to_string()),
        ];
        assert_eq!(
            uuid_from_links("nvme0n1p2", links.iter().cloned()),
            Some("bbbb-2222".to_string())
        );
        assert_eq!(uuid_from_links("sdb9", links.iter().cloned()), None);
        assert_eq!(uuid_from_links("anything", std::iter::empty()), None);
    }
}
