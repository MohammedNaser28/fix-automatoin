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
