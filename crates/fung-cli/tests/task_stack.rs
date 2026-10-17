//! A spawned task gets as much stack as a Go goroutine may grow to
//! (1 GiB, the first of `fung_vm::stack::TASK_STACKS`): a program that nests Rust frames deeply (a
//! native calling back into fung) gives the same result in a task as on
//! the entry thread instead of aborting the process. Formatting a value
//! nested 1,000,000 deep also needs more than 256 MiB, but `str` on it
//! takes over a minute (in Go as well), so it is not tested here.
//! Release builds only: a debug build uses far more stack per frame and
//! overflows much earlier, on the entry thread too.
#![cfg(not(debug_assertions))]

mod common;

use std::ffi::OsString;

use common::{run, rust_fung};

/// Runs `code` with `fung -e` and returns its standard output, failing
/// on any other exit than 0.
fn eval(code: &str) -> String {
    let (out, err, status) = run(&rust_fung(), &[OsString::from("-e"), OsString::from(code)]);
    assert!(
        status.contains("unix_wait_status(0)") || status.contains("exit status: 0"),
        "status {status}\nstderr: {}",
        String::from_utf8_lossy(&err)
    );
    String::from_utf8(out).unwrap()
}

/// `map` calling back into fung, 99,000 levels deep.
const DEEP_CALLBACK: &str = "fn f(n) = n == 0 then 0 else map([n], x => f(x - 1))[0] + 1\n";

#[test]
fn deep_callbacks_in_a_task_match_the_entry() {
    let entry = eval(&format!("{DEEP_CALLBACK}print(f(99000) |> type_of)"));
    assert_eq!(entry, "'int\n");
    let task = eval(&format!(
        "{DEEP_CALLBACK}print(await(spawn(() => f(99000))) |> type_of)"
    ));
    assert_eq!(task, entry);
}
