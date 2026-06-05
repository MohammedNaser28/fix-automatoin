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
    load_modules()?;
    setup_console()?;
    set_controlling_tty()?;
    setup_signals()?;

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
        Err(Errno::EBUSY) => {} // kernel auto-mounted devtmpfs — fine
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }

    Ok(())
}

fn load_module_via_fd(data: &[u8], flags: i32) -> Result<(), std::io::Error> {
    use std::os::fd::{AsRawFd, FromRawFd};

    let fd = unsafe { libc::syscall(libc::SYS_memfd_create, c"mod".as_ptr(), 0_usize) as i32 };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut fd = unsafe { std::fs::File::from_raw_fd(fd) };
    use std::io::Write;
    fd.write_all(data)?;
    fd.flush()?;
    let raw_fd = fd.as_raw_fd();
    let ret = unsafe {
        libc::syscall(
            libc::SYS_finit_module,
            raw_fd,
            std::ptr::null::<libc::c_char>(),
            flags as libc::c_int,
        )
    };
    drop(fd);
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn load_modules() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new("/modules");
    if !dir.is_dir() {
        return Ok(());
    }

    const IGNORE_MODVERSIONS: i32 = 1;

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
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => {
                continue;
            }
        };

        match load_module_via_fd(&data, 0) {
            Ok(()) => continue,
            Err(e) if e.raw_os_error() == Some(libc::EEXIST) => continue,
            Err(_) => {}
        }

        match load_module_via_fd(&data, IGNORE_MODVERSIONS) {
            Ok(()) => eprintln!("init: loaded {name} (ignored modversions)"),
            Err(e) => eprintln!("init: load {name}: {e}"),
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
