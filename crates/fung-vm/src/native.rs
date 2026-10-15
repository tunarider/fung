//! What the core prelude and the native bridges share (Go `internal/nat`),
//! and the `_native.*` bridges themselves (Go `internal/native`).

pub mod fs;
pub(crate) mod glob;
pub mod nat;
pub(crate) mod oserr;
pub mod path;
pub mod process;
