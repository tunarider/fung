//! The terminal for the REPL's line editor (08): whether standard input
//! is a terminal, and raw mode.
//!
//! A terminal normally hands a program a whole line at a time and echoes
//! what is typed (canonical mode). The line editor needs every key as it
//! is pressed, so while it reads a line it turns off ECHO, ICANON, ISIG
//! and IEXTEN (and ICRNL and IXON on input).
//! Output processing stays on, so "\n" still moves to column 0. Raw mode
//! lasts only while one line is read: [`RawMode`] restores the saved
//! settings when it drops, which also happens when the reading code
//! unwinds from a panic. The code a line runs (an `exit(n)`, a crash)
//! therefore always finds the terminal as it was. A process that ends
//! without unwinding (SIGTERM or SIGHUP from outside, a closed output's
//! SIGPIPE) restores the saved settings first ([`restore`]): they are
//! also kept in [`SAVED`] while raw mode is on.
//!
//! `fung-cli` may depend only on workspace crates, so the `libc` calls
//! live here (`tests/deps.rs`).

use std::io;
use std::os::fd::RawFd;
use std::sync::Mutex;

/// The settings raw mode replaced, while it is on: what [`restore`] puts
/// back when the process ends without dropping the [`RawMode`].
static SAVED: Mutex<Option<(RawFd, libc::termios)>> = Mutex::new(None);

/// Puts back the settings raw mode replaced, if it is on: for a process
/// about to end at once (`signals::exit_now`, `sigpipe::die`).
pub fn restore() {
    let saved = SAVED.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some((fd, t)) = saved {
        // the process ends right after: a failure changes nothing
        let _ = set(fd, &t);
    }
}

/// Whether `fd` is a terminal: it has terminal settings (`tcgetattr`
/// succeeds). `/dev/null` and other devices are not.
pub fn is_terminal(fd: RawFd) -> bool {
    get(fd).is_ok()
}

/// The terminal settings of `fd` (`tcgetattr`).
fn get(fd: RawFd) -> io::Result<libc::termios> {
    // SAFETY: `tcgetattr` writes only into `t`, a local `termios` (a plain
    // C struct of integers, valid when zeroed); on failure it is unused.
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(t)
    }
}

/// Sets the terminal settings of `fd` at once (`TCSANOW`).
fn set(fd: RawFd, t: &libc::termios) -> io::Result<()> {
    // SAFETY: `tcsetattr` only reads `*t`, a valid `termios` borrowed for
    // the call; an `fd` that is not a terminal makes it fail (ENOTTY).
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, t) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Raw mode on a terminal until this drops, which restores the settings
/// it found.
pub struct RawMode {
    fd: RawFd,
    saved: libc::termios,
}

impl RawMode {
    /// Switches `fd` to raw input. Fails when `fd` is not a
    /// terminal; nothing is changed then.
    pub fn enable(fd: RawFd) -> io::Result<RawMode> {
        let saved = get(fd)?;
        let mut raw = saved;
        raw.c_iflag &= !(libc::ICRNL | libc::IXON);
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        *SAVED.lock().unwrap_or_else(|e| e.into_inner()) = Some((fd, saved));
        if let Err(e) = set(fd, &raw) {
            SAVED.lock().unwrap_or_else(|e| e.into_inner()).take();
            return Err(e);
        }
        Ok(RawMode { fd, saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        SAVED.lock().unwrap_or_else(|e| e.into_inner()).take();
        // a failure here has nothing left to do about it
        let _ = set(self.fd, &self.saved);
    }
}

/// Pseudo-terminals for tests (the REPL's raw mode is tested on one,
/// including from `fung-cli`'s tests, which cannot use `libc`).
///
/// Not behind a cargo feature: fung-cli's tests would have to enable it
/// through a dev-dependency with `features = [...]`, and `tests/deps.rs`
/// allows fung-cli's dependencies to be nothing but `path = "../…"`. The
/// command never calls it, so the linker drops it from the binary.
#[doc(hidden)]
pub mod pty {
    use std::io;
    use std::os::fd::{FromRawFd, OwnedFd, RawFd};

    // glibc before 2.34 has `openpty` in libutil, not libc; later glibc
    // keeps an empty libutil for programs that still link it. musl has
    // `openpty` in libc and Rust's self-contained musl ships no libutil,
    // so the static release build must not ask for it. The extern block
    // is linked into every binary that uses fung-vm, the command too.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[link(name = "util")]
    unsafe extern "C" {}

    /// A new pseudo-terminal: (controller, terminal).
    pub fn open() -> io::Result<(OwnedFd, OwnedFd)> {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: `openpty` writes two descriptors into the locals; the
        // name, settings and size pointers may be null (then unused).
        let r = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if r != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: on success both are fresh descriptors nothing else owns.
        let fds = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
        // `openpty` leaves them open across `exec`: a child another
        // thread starts would keep the terminal open after its test ends.
        for fd in [master, slave] {
            // SAFETY: `fd` is open (owned by `fds`); fcntl takes no pointers here.
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(fds)
    }

    /// Whether the terminal `fd` reads keys raw: canonical mode and echo
    /// both off.
    pub fn is_raw(fd: RawFd) -> io::Result<bool> {
        let t = super::get(fd)?;
        Ok(t.c_lflag & (libc::ICANON | libc::ECHO) == 0)
    }

    /// The local-mode flags of the terminal `fd`, to compare settings.
    /// PENDIN is left out: the kernel sets it (macOS does) when canonical
    /// mode comes back on, to reprocess input typed meanwhile.
    /// As `u64` on every system (`tcflag_t` is narrower on Linux).
    #[allow(clippy::useless_conversion)]
    pub fn lflag(fd: RawFd) -> io::Result<u64> {
        Ok((super::get(fd)?.c_lflag & !libc::PENDIN).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

    #[test]
    fn raw_mode_lasts_until_the_guard_drops() {
        let (_master, slave) = pty::open().unwrap();
        let fd = slave.as_raw_fd();
        let before = pty::lflag(fd).unwrap();
        assert!(!pty::is_raw(fd).unwrap());
        {
            let _raw = RawMode::enable(fd).unwrap();
            assert!(pty::is_raw(fd).unwrap());
            let t = get(fd).unwrap();
            assert_eq!(t.c_lflag & (libc::ISIG | libc::IEXTEN), 0);
            assert_eq!(t.c_iflag & (libc::ICRNL | libc::IXON), 0);
            assert_eq!(t.c_cc[libc::VMIN], 1);
            assert_ne!(t.c_oflag & libc::OPOST, 0, "output processing stays on");
        }
        assert_eq!(pty::lflag(fd).unwrap(), before);
    }

    #[test]
    fn a_panic_while_raw_restores_the_terminal() {
        let (_master, slave) = pty::open().unwrap();
        let fd = slave.as_raw_fd();
        let before = pty::lflag(fd).unwrap();
        let r = std::panic::catch_unwind(|| {
            let _raw = RawMode::enable(fd).unwrap();
            assert!(pty::is_raw(fd).unwrap());
            panic!("a bug while reading a line");
        });
        assert!(r.is_err());
        assert_eq!(pty::lflag(fd).unwrap(), before);
    }

    #[test]
    fn not_a_terminal() {
        let (r, _w) = crate::testutil::child::pipe().unwrap();
        assert!(!is_terminal(r.as_raw_fd()));
        assert!(RawMode::enable(r.as_raw_fd()).is_err());
        // a character device, but no terminal
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(!is_terminal(null.as_raw_fd()));
        assert!(RawMode::enable(null.as_raw_fd()).is_err());
        let (_master, slave) = pty::open().unwrap();
        assert!(is_terminal(slave.as_raw_fd()));
    }
}
