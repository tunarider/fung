//! What the core prelude and the native bridges share, and the
//! `_native.*` bridges themselves.

pub(crate) mod file;
pub mod fs;
pub(crate) mod glob;
pub mod json;
pub mod nat;
pub mod net;
pub(crate) mod oserr;
pub mod path;
pub mod process;
pub mod range;
pub mod regex;
pub(crate) mod sock;
pub mod time;
pub(crate) mod timefmt;
pub(crate) mod tz;
pub(crate) mod walk;

/// fung's text for an operating-system error (the short lower-case reason
/// of `oserr`), for the command's own messages (`fung check`/`test`/`fmt`
/// path errors).
pub fn os_error_text(e: &std::io::Error) -> String {
    oserr::reason(e)
}
