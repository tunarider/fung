//! Operating-system errors as Go reports them (Go `nat.Reason` over an
//! `*os.PathError`): the reason is Go's own text for the errno
//! (`syscall.Errno.Error()`, from Go's `zerrors_<os>_<arch>.go` tables),
//! not the C library's `strerror` and never Rust's `io::Error` display
//! ("No such file or directory (os error 2)").
//!
//! The tables are copied from Go 1.27's `syscall` package. Errno numbers
//! differ between macOS and Linux, so each has its own table, indexed by
//! the platform's own numbering (`io::Error::raw_os_error`). An errno
//! without text reads `errno <n>`, as in Go. Transitional (open-questions
//! "Go 동등을 위한 과도기 잔가지").

use std::io;

/// Go's `fs.ErrNotExist`, `fs.ErrExist` and `fs.ErrPermission` as errnos
/// (`syscall.Errno.Is`).
pub(crate) const ENOENT: i32 = 2;
pub(crate) const EPERM: i32 = 1;
pub(crate) const EACCES: i32 = 13;
pub(crate) const EEXIST: i32 = 17;
pub(crate) const ENOTDIR: i32 = 20;
pub(crate) const EINVAL: i32 = 22;
#[cfg(target_os = "macos")]
pub(crate) const ENOTEMPTY: i32 = 66;
#[cfg(not(target_os = "macos"))]
pub(crate) const ENOTEMPTY: i32 = 39;

/// Go `nat.Reason` for an I/O error: the errno's Go text. An error that
/// carries no errno (made by the standard library itself) gives its own
/// text in lower case, without Rust's " (os error N)".
pub(crate) fn reason(e: &io::Error) -> String {
    if let Some(n) = e.raw_os_error() {
        return errno_text(n);
    }
    let text = e.to_string();
    let mut cs = text.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().chain(cs).collect(),
        None => String::new(),
    }
}

/// The kind of `Err` Go's `fsErr` gives an error: `not_found`, `exists`,
/// `permission` or `io` (`errors.Is` with `fs.ErrNotExist`, `fs.ErrExist`,
/// `fs.ErrPermission`).
pub(crate) fn kind(e: &io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(ENOENT) => "not_found",
        Some(EEXIST | ENOTEMPTY) => "exists",
        Some(EACCES | EPERM) => "permission",
        _ => "io",
    }
}

/// An I/O error with errno `n`.
pub(crate) fn errno(n: i32) -> io::Error {
    io::Error::from_raw_os_error(n)
}

/// Go `syscall.Errno(n).Error()`.
pub(crate) fn errno_text(n: i32) -> String {
    let text = usize::try_from(n)
        .ok()
        .and_then(|i| ERRORS.get(i).copied())
        .or_else(|| extra(n))
        .unwrap_or("");
    if text.is_empty() {
        format!("errno {n}")
    } else {
        text.to_string()
    }
}

/// Go's arm64 tables have one entry more than its amd64 ones.
fn extra(n: i32) -> Option<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if n == 106 {
        return Some("interface output queue is full");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    if n == 133 {
        return Some("memory page has hardware error");
    }
    let _ = n;
    None
}

/// Go `zerrors_darwin_amd64.go` `errors` (index = errno; `""` = none).
#[cfg(target_os = "macos")]
const ERRORS: &[&str] = &[
    "",
    "operation not permitted",
    "no such file or directory",
    "no such process",
    "interrupted system call",
    "input/output error",
    "device not configured",
    "argument list too long",
    "exec format error",
    "bad file descriptor",
    "no child processes",
    "resource deadlock avoided",
    "cannot allocate memory",
    "permission denied",
    "bad address",
    "block device required",
    "resource busy",
    "file exists",
    "cross-device link",
    "operation not supported by device",
    "not a directory",
    "is a directory",
    "invalid argument",
    "too many open files in system",
    "too many open files",
    "inappropriate ioctl for device",
    "text file busy",
    "file too large",
    "no space left on device",
    "illegal seek",
    "read-only file system",
    "too many links",
    "broken pipe",
    "numerical argument out of domain",
    "result too large",
    "resource temporarily unavailable",
    "operation now in progress",
    "operation already in progress",
    "socket operation on non-socket",
    "destination address required",
    "message too long",
    "protocol wrong type for socket",
    "protocol not available",
    "protocol not supported",
    "socket type not supported",
    "operation not supported",
    "protocol family not supported",
    "address family not supported by protocol family",
    "address already in use",
    "can't assign requested address",
    "network is down",
    "network is unreachable",
    "network dropped connection on reset",
    "software caused connection abort",
    "connection reset by peer",
    "no buffer space available",
    "socket is already connected",
    "socket is not connected",
    "can't send after socket shutdown",
    "too many references: can't splice",
    "operation timed out",
    "connection refused",
    "too many levels of symbolic links",
    "file name too long",
    "host is down",
    "no route to host",
    "directory not empty",
    "too many processes",
    "too many users",
    "disc quota exceeded",
    "stale NFS file handle",
    "too many levels of remote in path",
    "RPC struct is bad",
    "RPC version wrong",
    "RPC prog. not avail",
    "program version wrong",
    "bad procedure for program",
    "no locks available",
    "function not implemented",
    "inappropriate file type or format",
    "authentication error",
    "need authenticator",
    "device power is off",
    "device error",
    "value too large to be stored in data type",
    "bad executable (or shared library)",
    "bad CPU type in executable",
    "shared library version mismatch",
    "malformed Mach-o file",
    "operation canceled",
    "identifier removed",
    "no message of desired type",
    "illegal byte sequence",
    "attribute not found",
    "bad message",
    "EMULTIHOP (Reserved)",
    "no message available on STREAM",
    "ENOLINK (Reserved)",
    "no STREAM resources",
    "not a STREAM",
    "protocol error",
    "STREAM ioctl timeout",
    "operation not supported on socket",
    "policy not found",
    "state not recoverable",
    "previous owner died",
];

/// Go `zerrors_linux_amd64.go` `errors` (index = errno; `""` = none).
/// MIPS, SPARC and POWER number some errnos differently.
#[cfg(not(target_os = "macos"))]
const ERRORS: &[&str] = &[
    "",
    "operation not permitted",
    "no such file or directory",
    "no such process",
    "interrupted system call",
    "input/output error",
    "no such device or address",
    "argument list too long",
    "exec format error",
    "bad file descriptor",
    "no child processes",
    "resource temporarily unavailable",
    "cannot allocate memory",
    "permission denied",
    "bad address",
    "block device required",
    "device or resource busy",
    "file exists",
    "invalid cross-device link",
    "no such device",
    "not a directory",
    "is a directory",
    "invalid argument",
    "too many open files in system",
    "too many open files",
    "inappropriate ioctl for device",
    "text file busy",
    "file too large",
    "no space left on device",
    "illegal seek",
    "read-only file system",
    "too many links",
    "broken pipe",
    "numerical argument out of domain",
    "numerical result out of range",
    "resource deadlock avoided",
    "file name too long",
    "no locks available",
    "function not implemented",
    "directory not empty",
    "too many levels of symbolic links",
    "",
    "no message of desired type",
    "identifier removed",
    "channel number out of range",
    "level 2 not synchronized",
    "level 3 halted",
    "level 3 reset",
    "link number out of range",
    "protocol driver not attached",
    "no CSI structure available",
    "level 2 halted",
    "invalid exchange",
    "invalid request descriptor",
    "exchange full",
    "no anode",
    "invalid request code",
    "invalid slot",
    "",
    "bad font file format",
    "device not a stream",
    "no data available",
    "timer expired",
    "out of streams resources",
    "machine is not on the network",
    "package not installed",
    "object is remote",
    "link has been severed",
    "advertise error",
    "srmount error",
    "communication error on send",
    "protocol error",
    "multihop attempted",
    "RFS specific error",
    "bad message",
    "value too large for defined data type",
    "name not unique on network",
    "file descriptor in bad state",
    "remote address changed",
    "can not access a needed shared library",
    "accessing a corrupted shared library",
    ".lib section in a.out corrupted",
    "attempting to link in too many shared libraries",
    "cannot exec a shared library directly",
    "invalid or incomplete multibyte or wide character",
    "interrupted system call should be restarted",
    "streams pipe error",
    "too many users",
    "socket operation on non-socket",
    "destination address required",
    "message too long",
    "protocol wrong type for socket",
    "protocol not available",
    "protocol not supported",
    "socket type not supported",
    "operation not supported",
    "protocol family not supported",
    "address family not supported by protocol",
    "address already in use",
    "cannot assign requested address",
    "network is down",
    "network is unreachable",
    "network dropped connection on reset",
    "software caused connection abort",
    "connection reset by peer",
    "no buffer space available",
    "transport endpoint is already connected",
    "transport endpoint is not connected",
    "cannot send after transport endpoint shutdown",
    "too many references: cannot splice",
    "connection timed out",
    "connection refused",
    "host is down",
    "no route to host",
    "operation already in progress",
    "operation now in progress",
    "stale file handle",
    "structure needs cleaning",
    "not a XENIX named type file",
    "no XENIX semaphores available",
    "is a named type file",
    "remote I/O error",
    "disk quota exceeded",
    "no medium found",
    "wrong medium type",
    "operation canceled",
    "required key not available",
    "key has expired",
    "key has been revoked",
    "key was rejected by service",
    "owner died",
    "state not recoverable",
    "operation not possible due to RF-kill",
];
