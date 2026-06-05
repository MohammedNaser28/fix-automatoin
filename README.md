# fix-automation

Bootable rescue USB tool — repairs broken GRUB, fixes fstab UUID mismatches, and recovers boot entries after partition changes or Windows installs.

![CI build-os](https://github.com/MohammedNaser28/fix-automatoin-alpine/actions/workflows/build-os.yaml/badge.svg)
![CI alpine](https://github.com/MohammedNaser28/fix-automatoin-alpine/actions/workflows/build-alpine.yaml/badge.svg)
![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust)
![Release](https://img.shields.io/github/v/release/MohammedNaser28/fix-automatoin-alpine)

<p align="center">
  <!-- TODO: add screenshot here -->
  <img src="docs/screenshots/tui-main.png" alt="fix-automation TUI" width="700"/>
</p>

## What is fix-automation

fix-automation solves the problem of a Linux system that won't boot. Whether GRUB was wiped by a Windows install, fstab UUIDs became stale after repartitioning, or an EFI boot entry went missing — booting from this USB lets you repair in place without needing a live desktop environment.

The tool runs entirely offline. After booting from the USB, a Ratatui TUI walks you through scanning block devices, selecting the root and EFI partitions, confirming targets, and choosing a repair action. All repair operations run via chroot into your installed system, so the fixes persist after reboot. No internet connection is required.

## Features

- GRUB reinstall and grub.cfg regeneration (UEFI + BIOS)
- fstab repair — auto-regenerate or edit manually
- Device scan and firmware detection (UEFI/BIOS)
- Chroot shell access for advanced recovery
- Windows EFI entry recovery (recover from NTFS backup)
- Partition manager — list, create, delete, resize
- Log export via QR code and paste URL
- Diagnose with AI — send logs to a language model
- Works fully offline

## Distributions

Two bootable ISO variants are built from this repo. The Rust binary is the same in both; only the surrounding OS differs.

| | Buildroot | Alpine |
|---|---|---|
| **ISO size** | ~TBD | ~TBD |
| **Base OS** | Custom minimal Linux (Buildroot) | Alpine Linux |
| **Boot method** | UEFI + BIOS, Ventoy, FAT32 USB | UEFI + BIOS, Ventoy |
| **Rust feature flag** | `default` | `--features alpine` |
| **Use case** | Smallest possible image, embedded-friendly | Familiar Alpine environment, easier to extend |
| **Package manager** | None (static binary only) | apk available in chroot |

Use **Buildroot** when you want the smallest image or are flashing to a tiny FAT32 partition. It boots fast and contains only what is needed for repair. This is the recommended variant for everyday rescue.

Use **Alpine** when you want a more familiar Linux environment, need to install extra packages during a rescue session (via apk), or prefer Ventoy compatibility with the full Alpine userland available alongside the TUI.

## Download

Grab the latest release from the [releases page](https://github.com/MohammedNaser28/fix-automatoin-alpine/releases).

| Artifact | Description |
|---|---|
| `fix-automation-x86_64-buildroot.iso` | Buildroot ISO — boot with Ventoy or burn directly |
| `grub-rescue-usb.zip` | Buildroot ZIP — extract to a FAT32 USB partition manually |
| `fix-automation-x86_64-alpine.iso` | Alpine ISO — boot with Ventoy or burn directly |
| `fix-automation` | Raw static binary (musl) — for advanced use |

## Usage

### Flash to USB

**Method 1: dd (Linux/macOS)**
```bash
# replace /dev/sdX with your USB drive — double-check with lsblk
sudo dd if=fix-automation-x86_64-buildroot.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

**Method 2: Ventoy**
```
1. Install Ventoy on your USB drive (https://ventoy.net)
2. Copy either .iso file to the Ventoy partition
3. Boot from USB and select fix-automation from the Ventoy menu
```

**Method 3: Manual FAT32 (Buildroot ZIP only)**
```bash
# Format a small partition (512MB is enough) as FAT32
sudo mkfs.vfat -F32 /dev/sdX1
sudo mount /dev/sdX1 /mnt/usb
unzip grub-rescue-usb.zip -d /mnt/usb
sudo umount /mnt/usb
```

### Boot and repair

1. Boot from the USB (set boot order in BIOS/UEFI or use boot menu key)
2. fix-automation starts automatically — no login required
3. Follow the TUI: scan detects your disks, select root and EFI partitions
4. Choose your repair action from the action menu
5. Review the execution log, export logs if needed
6. Reboot when done

## Test locally with QEMU

### Buildroot ISO

```bash
# install deps
sudo apt-get install -y qemu-system-x86 ovmf

# UEFI boot
qemu-system-x86_64 \
  -cdrom fix-automation-x86_64-buildroot.iso \
  -m 512M -machine q35 -nographic \
  -bios /usr/share/ovmf/OVMF.fd \
  -no-reboot -accel tcg

# BIOS boot
qemu-system-x86_64 \
  -cdrom fix-automation-x86_64-buildroot.iso \
  -m 512M -machine pc -nographic \
  -no-reboot -accel tcg
```

### Alpine ISO

```bash
# UEFI boot
qemu-system-x86_64 \
  -cdrom fix-automation-x86_64-alpine.iso \
  -m 512M -machine q35 -nographic \
  -bios /usr/share/ovmf/OVMF.fd \
  -no-reboot -accel tcg
```

## Build from source

### Prerequisites

```
- Rust stable (rustup)
- x86_64-unknown-linux-musl target: rustup target add x86_64-unknown-linux-musl
- For Buildroot: Docker, or native Linux with make gcc g++ python3 bc wget cpio rsync
- For Alpine: Docker
```

### Build

```bash
# clone
git clone https://github.com/MohammedNaser28/fix-automatoin-alpine
cd fix-automatoin-alpine

# build default (Buildroot) binary
cargo build --release --target x86_64-unknown-linux-musl

# build Alpine binary
cargo build --release --target x86_64-unknown-linux-musl --features alpine

# full Alpine ISO (requires Docker)
docker build -t fix-automation-builder -f dist/alpine/Dockerfile .
docker run --rm --privileged \
  -v $PWD:/build \
  -e BINARY_PATH=/build/target/x86_64-unknown-linux-musl/release/fix-automation \
  -e OUTPUT_DIR=/build/dist/alpine/output \
  fix-automation-builder \
  sh /build/dist/alpine/build.sh --arch x86_64
```

## Project structure

```
fix-automation/
├── src/                    # Rust TUI source
├── os-config/              # Buildroot external tree
│   ├── configs/            # Buildroot defconfig
│   └── rootfs_overlay/     # Files injected into rootfs
├── dist/
│   └── alpine/             # Alpine ISO Dockerfile + build script
├── .github/
│   └── workflows/          # CI pipelines
└── cliff.toml              # Changelog config (git-cliff)
```

## CI / CD

Every push triggers the Rust CI (lint, clippy, test, build for both feature flags). Build and boot-test pipelines run in parallel for the Buildroot and Alpine variants. On version tags (`v*`), a GitHub Release is created with a changelog generated by git-cliff and the ISO/ZIP artifacts attached.

```
push / tag
    │
    ▼
rust-ci.yml  (lint · clippy · test · build x2 features)
    │
    ├──► build-os.yaml   →  qemu-boot (UEFI + BIOS)  →  release
    │
    └──► build-alpine.yaml  →  qemu-boot (UEFI + BIOS)  →  release
```

## License

No license specified.
