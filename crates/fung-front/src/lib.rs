// Diag carries a call path now and exceeds clippy's 128-byte Err limit; errors are
// the cold path, so boxing every Result<_, Diag> is not worth the churn.
#![allow(clippy::result_large_err)]

pub mod builtins;
pub mod bytecode;
pub mod compiler;
pub mod consts;
pub mod desugar;
pub mod diag;
pub mod extras;
pub mod format;
pub mod lexer;
pub mod loader;
pub mod parser;
pub mod resolve;
pub mod source;
pub mod tree;
pub mod unicode;
