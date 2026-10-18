//! Operating-system errors in fung's words: the reason in an
//! `Err({kind, message})` is a short lower-case text, the same on every
//! platform for the common errors (`text_of`), and otherwise the system's
//! own text without Rust's " (os error N)".

use std::io;

/// Errnos the natives make or the tests name (the same numbers on Linux
/// and macOS, except `ENOTEMPTY`).
#[cfg(test)]
pub(crate) const ENOENT: i32 = 2;
pub(crate) const EPERM: i32 = 1;
#[cfg(test)]
pub(crate) const EACCES: i32 = 13;
pub(crate) const EEXIST: i32 = 17;
#[cfg(test)]
pub(crate) const ENOTDIR: i32 = 20;
pub(crate) const EINVAL: i32 = 22;
#[cfg(all(test, target_os = "macos"))]
pub(crate) const ENOTEMPTY: i32 = 66;
#[cfg(all(test, not(target_os = "macos")))]
pub(crate) const ENOTEMPTY: i32 = 39;

/// The reason text for an I/O error.
pub(crate) fn reason(e: &io::Error) -> String {
    // EPERM shares its kind with EACCES but means something else.
    if e.raw_os_error() == Some(EPERM) {
        return "operation not permitted".to_string();
    }
    if let Some(t) = text_of(e.kind()) {
        return t.to_string();
    }
    let text = e.to_string();
    let text = match text.find(" (os error ") {
        Some(at) if text.ends_with(')') => &text[..at],
        _ => &text,
    };
    let mut cs = text.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().chain(cs).collect(),
        None => String::new(),
    }
}

/// fung's text for the common kinds of error.
fn text_of(kind: io::ErrorKind) -> Option<&'static str> {
    use io::ErrorKind as K;
    Some(match kind {
        K::NotFound => "no such file or directory",
        K::PermissionDenied => "permission denied",
        K::AlreadyExists => "file exists",
        K::NotADirectory => "not a directory",
        K::IsADirectory => "is a directory",
        K::DirectoryNotEmpty => "directory not empty",
        K::InvalidInput => "invalid argument",
        K::ReadOnlyFilesystem => "read-only file system",
        K::StorageFull => "no space left on device",
        K::FileTooLarge => "file too large",
        K::CrossesDevices => "invalid cross-device link",
        K::InvalidFilename => "file name too long",
        K::ExecutableFileBusy => "text file busy",
        K::ArgumentListTooLong => "argument list too long",
        K::BrokenPipe => "broken pipe",
        K::ConnectionRefused => "connection refused",
        K::ConnectionReset => "connection reset by peer",
        K::TimedOut => "connection timed out",
        K::AddrInUse => "address already in use",
        K::AddrNotAvailable => "can't assign requested address",
        K::HostUnreachable => "no route to host",
        K::NetworkUnreachable => "network is unreachable",
        K::Interrupted => "interrupted system call",
        K::OutOfMemory => "cannot allocate memory",
        _ => return None,
    })
}

/// The kind of the `Err` a file native gives for an error: `not_found`,
/// `exists`, `permission` or `io`.
pub(crate) fn kind(e: &io::Error) -> &'static str {
    use io::ErrorKind as K;
    match e.kind() {
        K::NotFound => "not_found",
        K::AlreadyExists | K::DirectoryNotEmpty => "exists",
        K::PermissionDenied => "permission",
        _ => "io",
    }
}

/// An I/O error with errno `n`.
pub(crate) fn errno(n: i32) -> io::Error {
    io::Error::from_raw_os_error(n)
}

/// The reason text for errno `n`.
pub(crate) fn errno_text(n: i32) -> String {
    reason(&errno(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_texts() {
        for (n, want) in [
            (ENOENT, "no such file or directory"),
            (EACCES, "permission denied"),
            (EEXIST, "file exists"),
            (ENOTDIR, "not a directory"),
            (ENOTEMPTY, "directory not empty"),
            (EINVAL, "invalid argument"),
        ] {
            assert_eq!(errno_text(n), want, "errno {n}");
        }
    }

    /// An errno outside fung's table reads as the system's text, starting
    /// in lower case, without the "(os error N)" suffix.
    #[test]
    fn fallback_text_has_no_suffix() {
        #[cfg(target_os = "macos")]
        const ENOLCK: i32 = 77;
        #[cfg(not(target_os = "macos"))]
        const ENOLCK: i32 = 37;
        let t = errno_text(ENOLCK);
        assert!(!t.contains("os error"), "{t}");
        assert!(t.starts_with(|c: char| c.is_lowercase()), "{t}");
        assert_eq!(t, reason(&io::Error::from_raw_os_error(ENOLCK)));
    }

    #[test]
    fn no_errno_error_is_lowercase() {
        assert_eq!(reason(&io::Error::other("Bad thing")), "bad thing");
    }

    /// The common texts do not depend on the errno numbering: they come
    /// from the error's kind.
    #[test]
    fn texts_follow_the_kind() {
        let e = io::Error::from(io::ErrorKind::NotADirectory);
        assert_eq!(reason(&e), "not a directory");
        let e = io::Error::from(io::ErrorKind::StorageFull);
        assert_eq!(reason(&e), "no space left on device");
    }
}
