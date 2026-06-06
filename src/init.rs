use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static IS_INIT: AtomicBool = AtomicBool::new(false);

pub fn is_pid1() -> bool {
    std::process::id() == 1
}

pub fn init_system() -> Result<(), Box<dyn std::error::Error>> {
    if !is_pid1() {
        return Ok(());
    }

    IS_INIT.store(true, Ordering::SeqCst);

    mount_fs()?;

    let s = serial_init().unwrap_or(-1);
    serial_fd(s, "init: start");

    if let Err(e) = load_modules(s) {
        serial_fd(s, &format!("init: load_modules: {e}"));
    }
    serial_fd(s, "init: modules done");

    // Always set up console (tty0 works with vgacon in text mode)
    if let Err(e) = setup_console() {
        serial_fd(s, &format!("init: setup_console: {e}"));
        return Err(e);
    }
    if Path::new("/dev/fb0").exists() {
        serial_fd(s, "init: /dev/fb0 OK");
        if let Err(e) = set_controlling_tty() {
            serial_fd(s, &format!("init: set_controlling_tty: {e}"));
            return Err(e);
        }
    } else {
        serial_fd(s, "init: /dev/fb0 missing — VT visible, no fbcon");
    }

    if let Err(e) = setup_signals() {
        serial_fd(s, &format!("init: setup_signals: {e}"));
        return Err(e);
    }

    serial_fd(s, "init: done");

    Ok(())
}

fn mount_fs() -> Result<(), Box<dyn std::error::Error>> {
    use nix::errno::Errno;
    use nix::mount::{MsFlags, mount};

    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV;

    let _ = mount::<str, str, str, str>(Some("proc"), "/proc", Some("proc"), flags, None::<&str>);

    let _ = mount::<str, str, str, str>(Some("sysfs"), "/sys", Some("sysfs"), flags, None::<&str>);

    match mount::<str, str, str, str>(
        Some("devtmpfs"),
        "/dev",
        Some("devtmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
        None::<&str>,
    ) {
        Err(Errno::EBUSY) => {} // kernel auto-mounted devtmpfs - fine
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }

    Ok(())
}

fn serial_init() -> Option<i32> {
    use std::os::fd::AsRawFd;
    if let Ok(fd) = nix::fcntl::open(
        "/dev/ttyS0",
        nix::fcntl::OFlag::O_WRONLY,
        nix::sys::stat::Mode::empty(),
    ) {
        Some(fd.as_raw_fd())
    } else {
        None
    }
}

fn serial_fd(fd: i32, msg: &str) {
    unsafe {
        let _ = libc::write(fd, msg.as_ptr() as *const libc::c_void, msg.len());
        let _ = libc::write(fd, c"\n".as_ptr() as *const libc::c_void, 2usize);
    }
}

fn load_modules(s: i32) -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new("/modules");
    if !dir.is_dir() {
        serial_fd(s, "init: no /modules dir");
        return Ok(());
    }

    serial_fd(s, "init: loading modules...");

    for name in &[
        "agpgart",
        "drm",
        "drm_kms_helper",
        "ttm",
        "drm_ttm_helper",
        "drm_vram_helper",
        "bochs",
    ] {
        let path = dir.join(format!("{name}.ko"));
        if !path.exists() {
            serial_fd(s, &format!("init: {name}: file not found"));
            continue;
        }

        #[allow(clippy::collapsible_if)]
        if let Ok(status) = std::process::Command::new("/busybox")
            .args(["insmod", &path.to_string_lossy()])
            .status()
        {
            if status.success() {
                serial_fd(s, &format!("init: {name}: OK"));
            } else {
                serial_fd(
                    s,
                    &format!("init: {name}: busybox exit={:?}", status.code()),
                );
            }
        } else {
            serial_fd(s, &format!("init: {name}: failed to run busybox"));
        }
    }

    Ok(())
}

fn setup_console() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::fd::AsRawFd;

    for dev in &["/dev/tty0", "/dev/console", "/dev/tty1"] {
        if let Ok(fd) = nix::fcntl::open(
            Path::new(dev),
            nix::fcntl::OFlag::O_RDWR,
            nix::sys::stat::Mode::empty(),
        ) {
            unsafe {
                libc::dup2(fd.as_raw_fd(), 0);
                libc::dup2(fd.as_raw_fd(), 1);
                libc::dup2(fd.as_raw_fd(), 2);
            }
            if fd.as_raw_fd() > 2 {
                let _ = nix::unistd::close(fd);
            }
            return Ok(());
        }
    }

    Err("could not open any console device".into())
}

fn set_controlling_tty() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::fd::AsRawFd;

    for dev in &["/dev/tty0", "/dev/console"] {
        if let Ok(fd) = nix::fcntl::open(
            Path::new(dev),
            nix::fcntl::OFlag::O_RDWR,
            nix::sys::stat::Mode::empty(),
        ) {
            unsafe {
                libc::ioctl(fd.as_raw_fd(), libc::TIOCSCTTY, 0);
            }
            if fd.as_raw_fd() > 2 {
                let _ = nix::unistd::close(fd);
            }
            return Ok(());
        }
    }

    Err("could not set controlling tty".into())
}

fn setup_signals() -> Result<(), Box<dyn std::error::Error>> {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

    extern "C" fn sigchld_handler(_sig: i32) {
        unsafe { while libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) > 0 {} }
    }

    let action = SigAction::new(
        SigHandler::Handler(sigchld_handler),
        SaFlags::SA_NOCLDSTOP | SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    unsafe { sigaction(Signal::SIGCHLD, &action)? };

    Ok(())
}

pub fn poweroff() {
    if !IS_INIT.load(Ordering::SeqCst) {
        let _ = std::process::Command::new("poweroff").status();
        return;
    }
    unsafe {
        libc::sync();
        libc::reboot(libc::RB_POWER_OFF);
    }
}

pub fn reboot() {
    if !IS_INIT.load(Ordering::SeqCst) {
        let _ = std::process::Command::new("reboot").status();
        return;
    }
    unsafe {
        libc::sync();
        libc::reboot(libc::RB_AUTOBOOT);
    }
}

pub fn panic_reboot() {
    if is_pid1() {
        unsafe {
            libc::sync();
            libc::reboot(libc::RB_AUTOBOOT);
        }
    }
}
