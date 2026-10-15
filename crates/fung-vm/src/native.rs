//! What the core prelude and the native bridges share (Go `internal/nat`),
//! and the `_native.*` bridges themselves (Go `internal/native`).

pub(crate) mod file;
pub mod fs;
pub(crate) mod glob;
pub mod nat;
pub(crate) mod oserr;
pub mod path;
pub mod process;
// Not used outside its tests until time values arrive (stage 4b).
#[allow(dead_code)]
pub(crate) mod tz;
