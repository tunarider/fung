//! The terminal for the REPL's line editor (08, Go `internal/repl`
//! `line_unix.go`): whether standard input is a terminal, and raw mode.
//!
//! A terminal normally hands a program a whole line at a time and echoes
//! what is typed (canonical mode). The line editor needs every key as it
//! is pressed, so while it reads a line it turns off ECHO, ICANON, ISIG
//! and IEXTEN (and ICRNL and IXON on input), as Go's `rawMode` does.
//! Output processing stays on, so "\n" still moves to column 0. Raw mode
//! lasts only while one line is read: [`RawMode`] restores the saved
//! settings when it drops, which also happens when the reading code
//! unwinds from a panic. The code a line runs (an `exit(n)`, a crash)
//! therefore always finds the terminal as it was.
//!
//! `fung-cli` may depend only on workspace crates, so the `libc` calls
//! live here (`tests/deps.rs`).

use std::io;
use std::os::fd::RawFd;

/// Whether `fd` is a character device (Go `ModeCharDevice`, the test
/// `TermReader` makes). A terminal is one; so is `/dev/null`, where
/// [`RawMode::enable`] then fails and the editor reads plain lines.
pub fn is_char_device(fd: RawFd) -> bool {
    // SAFETY: `fstat` writes only into `st`, a local `stat` it fully
    // initialises on success (zeroed first, so it is a valid value either
    // way); an invalid `fd` makes it fail with EBADF, which is a `false`.
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        libc::fstat(fd, &mut st) == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFCHR
    }
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

/// Sets the terminal settings of `fd` at once (`TCSANOW`, Go's
/// `TIOCSETA`/`TCSETS`).
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
    /// Switches `fd` to raw input (Go `rawMode`). Fails when `fd` is not a
    /// terminal; nothing is changed then.
    pub fn enable(fd: RawFd) -> io::Result<RawMode> {
        let saved = get(fd)?;
        let mut raw = saved;
        raw.c_iflag &= !(libc::ICRNL | libc::IXON);
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        set(fd, &raw)?;
        Ok(RawMode { fd, saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // Go ignores the result too: there is nothing left to do about it.
        let _ = set(self.fd, &self.saved);
    }
}

/// Pseudo-terminals for tests (the REPL's raw mode is tested on one,
/// including from `fung-cli`'s tests, which cannot use `libc`).
#[doc(hidden)]
pub mod pty {
    use std::io;
    use std::os::fd::{FromRawFd, OwnedFd, RawFd};

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
        Ok(unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) })
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
    pub fn lflag(fd: RawFd) -> io::Result<libc::tcflag_t> {
        Ok(super::get(fd)?.c_lflag & !libc::PENDIN)
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
        let (r, _w) = std::io::pipe().unwrap();
        assert!(!is_char_device(r.as_raw_fd()));
        assert!(RawMode::enable(r.as_raw_fd()).is_err());
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(
            is_char_device(null.as_raw_fd()),
            "Go's test lets /dev/null through"
        );
        assert!(RawMode::enable(null.as_raw_fd()).is_err());
        let (_master, slave) = pty::open().unwrap();
        assert!(is_char_device(slave.as_raw_fd()));
    }
}
