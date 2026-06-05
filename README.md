# Fix-Automation

Bootable rescue USB tool — Rust + Ratatui TUI. Repairs GRUB, fixes fstab UUID mismatches, and reconfigures bootloaders after partition changes or Windows installs.

> **Target:** x86_64 UEFI/BIOS, Linux (Arch, Debian, Fedora, etc.)

## Screenshots / Demo

<!-- TODO: add screen recording or annotated screenshots -->
<!-- ![Welcome screen](media/welcome.png) -->
<!-- ![Action menu](media/action-menu.png) -->
<!-- ![BIOS boot test](media/bios-boot.gif) -->

Place images or a demo GIF in `media/` and link them above.

## Quick start

```bash
# Build native (for testing on your machine)
cargo run

# Build static musl binary (for Alpine/ramfs)
cargo build --release --target x86_64-unknown-linux-musl --features alpine

# Full image + QEMU boot test
./qemu-test-full.sh
```

### Distribution builds

| Variant | Output | Build command |
|---------|--------|---------------|
| **Buildroot** | `fix-automation-x86_64-buildroot.iso` + `grub-rescue-usb.zip` | CI or manual Buildroot build |
| **Alpine** | `fix-automation-x86_64-alpine.iso` | `docker build -f dist/alpine/Dockerfile .` |

## Screens flow

```
Welcome → SelectRoot → SelectEfi → Confirm → ActionMenu → ExecLog → Result → LogExport
```

## Project structure

```
src/
├── main.rs          ← terminal init, event loop, render dispatch
├── app.rs           ← App state, screens, log pipeline
├── init.rs          ← first-boot init (run before the TUI)
├── repair.rs        ← background repair thread
├── screens/         ← one file per screen
│   ├── welcome.rs   ├── confirm.rs    ├── exec_log.rs
│   ├── select_root.rs │ action_menu.rs  ├── log_export.rs
│   └── select_efi.rs  └── results.rs
├── sys/             ← system calls (mount, grub, fstab, blkdev, chroot, distro)
└── ui/              ← theme + shared widgets
    ├── theme.rs
    └── widgets.rs
```

## Testing

```bash
# Unit tests
cargo test

# Rust CI checks (same as CI)
cargo fmt --check
cargo clippy -- -D warnings

# QEMU boot test (requires Buildroot toolchain or prebuilt image)
./qemu-test-full.sh
```

Each workflow (`build-os.yaml`, `build-alpine.yaml`) boots the produced ISO under both UEFI (`-machine q35`) and BIOS (`-machine pc`) and asserts that init completes successfully.

## Contributing

PRs welcome. Keep commits focused, run `cargo fmt` and `cargo clippy` first.
