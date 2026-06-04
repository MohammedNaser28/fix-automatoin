use serde::Deserialize;
use std::collections::HashSet;
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

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

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
    let _ = Command::new("udevadm").args(["settle"]).output();

    let output = match Command::new("lsblk")
        .args(["--json", "-o", "NAME,SIZE,FSTYPE,LABEL,UUID,MOUNTPOINT"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("lsblk failed: {} — is util-linux installed?", e);
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
        let has_children = dev.children.as_ref().map_or(false, |c| !c.is_empty());

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
                    contents: if is_efi { Some("Scanning...".into()) } else { None },
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

    // Also detect raw devices via /sys/block (catches NVMe drives without
    // a partition table, or drives udev hasn't fully probed yet).
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if name.starts_with("loop") || name.starts_with("ram")
                || name.starts_with("zram") || name.starts_with("dm-")
                || seen.contains(&name) {
                continue;
            }
            seen.insert(name.clone());

            let size = std::fs::read_to_string(format!("/sys/block/{name}/size"))
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
                .unwrap_or_default();

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

    disks
}
