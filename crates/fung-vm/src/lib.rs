//! The fung runtime: values and their operations, and (in later tasks) the VM.

// Diag carries a call path and exceeds clippy's 128-byte Err limit; errors are the
// cold path, so boxing every Result<_, Diag> is not worth the churn.
#![allow(clippy::result_large_err)]

pub mod value;
