//! The open file limit (concurrency v2 R4a, spec 15).
//!
//! Every connection is a file descriptor, and a program launched from a
//! shell often starts with a soft limit of 256, far below the thousands of
//! connections the runtime can hold now that a waiting socket costs no
//! thread. So `fung` raises its soft limit to the hard limit at start
//! (`raise_open_files`), as other language runtimes do; no privilege is needed
//! below the hard limit. macOS refuses a soft limit above
//! `kern.maxfilesperproc` and Linux one above `fs.nr_open`, so the raise
//! stops there.
//!
//! The commands `fung` runs get the original soft limit back
//! (`restore_in_child`): an old program that uses `select()` breaks on a
//! descriptor numbered 1024 or more, and a limit it did not ask for could
//! let one through.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// The soft limit `fung` started with, once `raise_open_files` saved it.
static SAVED: AtomicBool = AtomicBool::new(false);
static ORIGINAL: AtomicU64 = AtomicU64::new(0);

fn get() -> Option<libc::rlimit> {
    // SAFETY: `getrlimit` fills the zeroed local struct.
    unsafe {
        let mut r: libc::rlimit = std::mem::zeroed();
        (libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) == 0).then_some(r)
    }
}

/// The most a soft limit may be on this system, below the hard limit.
#[cfg(target_os = "macos")]
fn ceiling() -> u64 {
    let mut n: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is a NUL-terminated string; `n` and `len` are a
    // valid output buffer of the size given.
    let ok = unsafe {
        libc::sysctlbyname(
            c"kern.maxfilesperproc".as_ptr(),
            (&mut n as *mut libc::c_int).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } == 0;
    // `OPEN_MAX` of <sys/syslimits.h> when the name cannot be read.
    if ok && n > 0 { n as u64 } else { 10_240 }
}

/// Linux refuses a soft limit above `fs.nr_open` (a hard limit of
/// "unlimited" is above it).
#[cfg(target_os = "linux")]
fn ceiling() -> u64 {
    std::fs::read_to_string("/proc/sys/fs/nr_open")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(u64::MAX)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn ceiling() -> u64 {
    u64::MAX
}

/// Raises the soft open file limit to the hard one (at most `ceiling`),
/// keeping the original for the commands `fung` runs. A failure leaves
/// the limit as it was.
pub fn raise_open_files() {
    let Some(mut r) = get() else { return };
    if !SAVED.swap(true, Ordering::SeqCst) {
        ORIGINAL.store(r.rlim_cur, Ordering::SeqCst);
    }
    let want = r.rlim_max.min(ceiling());
    if want <= r.rlim_cur {
        return;
    }
    r.rlim_cur = want;
    // SAFETY: `setrlimit` reads the initialised local struct.
    unsafe {
        libc::setrlimit(libc::RLIMIT_NOFILE, &r);
    }
}

/// The soft limit `fung` started with, if `raise_open_files` ran.
pub fn original() -> Option<u64> {
    SAVED
        .load(Ordering::SeqCst)
        .then(|| ORIGINAL.load(Ordering::SeqCst))
}

/// Gives a forked child the original soft limit back before it `exec`s
/// (a `pre_exec` hook): only atomic loads, `getrlimit` and `setrlimit`.
/// The two calls are not on POSIX's list of async-signal-safe functions,
/// but glibc, musl and macOS make each a plain system call, which is safe
/// between `fork` and `exec`.
pub(crate) fn restore_in_child() -> std::io::Result<()> {
    let Some(soft) = original() else {
        return Ok(());
    };
    let Some(mut r) = get() else { return Ok(()) };
    r.rlim_cur = soft;
    // SAFETY: `setrlimit` reads the initialised local struct; the child
    // has a single thread here.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &r) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn soft_and_hard() -> (u64, u64) {
        // SAFETY: `getrlimit` fills the zeroed local struct.
        unsafe {
            let mut r: libc::rlimit = std::mem::zeroed();
            assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut r), 0);
            (r.rlim_cur, r.rlim_max)
        }
    }

    #[test]
    fn raising_keeps_within_the_hard_limit() {
        let (before, _) = soft_and_hard();
        super::raise_open_files();
        let (soft, hard) = soft_and_hard();
        assert!(soft >= before, "{soft} < {before}");
        assert!(soft <= hard, "{soft} > {hard}");
        assert!(super::original().is_some());
    }

    /// Linux refuses a soft limit above `fs.nr_open` (a hard limit of
    /// "unlimited" is above it), so the raise stops there.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_linux_ceiling_is_nr_open() {
        let n: u64 = std::fs::read_to_string("/proc/sys/fs/nr_open")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(super::ceiling(), n);
    }
}
