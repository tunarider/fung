//! Operating-system errors in fung's words: a short lower-case text, the
//! same on every platform for the common errors (`text_of`), and otherwise
//! the system's own text without Rust's " (os error N)". The `Err` values
//! of the file natives and the loader's "cannot read module" both say it.

use std::io;

/// `EPERM` (the same number on Linux and macOS).
pub const EPERM: i32 = 1;

/// The reason text for an I/O error.
pub fn reason(e: &io::Error) -> String {
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
