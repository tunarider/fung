//! Operating-system errors in fung's words: the reason in an
//! `Err({kind, message})` is a short lower-case text, the same on every
//! platform for the common errors, and otherwise the system's own text
//! without Rust's " (os error N)" (`fung_front::oserr`, which the loader
//! shares).

use std::io;

/// Errnos the natives make or the tests name (the same numbers on Linux
/// and macOS, except `ENOTEMPTY`).
#[cfg(test)]
pub(crate) const ENOENT: i32 = 2;
#[cfg(test)]
pub(crate) const EPERM: i32 = fung_front::oserr::EPERM;
#[cfg(test)]
pub(crate) const EACCES: i32 = 13;
pub(crate) const EEXIST: i32 = 17;
pub(crate) const ENOTDIR: i32 = 20;
pub(crate) const EINVAL: i32 = 22;
#[cfg(all(test, target_os = "macos"))]
pub(crate) const ENOTEMPTY: i32 = 66;
#[cfg(all(test, not(target_os = "macos")))]
pub(crate) const ENOTEMPTY: i32 = 39;

/// The reason text for an I/O error (shared with the loader's "cannot
/// read module", `fung_front::oserr`).
pub(crate) fn reason(e: &io::Error) -> String {
    fung_front::oserr::reason(e)
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
