//! What the core prelude and the native bridges share (Go `internal/nat`),
//! and the `_native.*` bridges themselves (Go `internal/native`).

pub(crate) mod file;
pub mod fs;
pub(crate) mod glob;
pub mod json;
pub mod nat;
pub mod net;
pub(crate) mod oserr;
pub mod path;
pub mod process;
pub mod regex;
pub(crate) mod regex_go;
pub(crate) mod sock;
pub mod time;
pub(crate) mod timefmt;
pub(crate) mod tz;

/// Go's text for an operating-system error (`syscall.Errno.Error()`), for
/// the command's own messages (`fung check`/`test`/`fmt` path errors).
pub fn os_error_text(e: &std::io::Error) -> String {
    oserr::reason(e)
}
