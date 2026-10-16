//! The fung runtime: values and their operations, and the VM that runs
//! the front end's bytecode.

// Diag carries a call path and exceeds clippy's 128-byte Err limit; errors are the
// cold path, so boxing every Result<_, Diag> is not worth the churn.
#![allow(clippy::result_large_err)]

pub mod boot;
pub mod conc;
pub mod native;
pub mod prelude;
pub mod sigpipe;
pub mod value;
pub mod vm;

#[cfg(test)]
mod testutil;
