//! A spawned task (here in a scope) runs on the command thread, as the
//! entry does (since R1b): a program that nests calls deeply through a
//! native that calls back into fung (`map`) gives the same result in a
//! task as at the top level. Such calls keep their frames on the heap
//! (R2), and values are shown and compared without the Rust stack (L9;
//! `value/tests.rs` walks a million levels on a 1 MiB stack).
//! Release builds only, as the depth makes a debug build slow.
#![cfg(not(debug_assertions))]

mod common;

use std::ffi::OsString;

use common::{fung_bin, run};

/// Runs `code` with `fung -e` and returns its standard output, failing
/// on any other exit than 0.
fn eval(code: &str) -> String {
    let (out, err, status) = run(&fung_bin(), &[OsString::from("-e"), OsString::from(code)]);
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
        "{DEEP_CALLBACK}print(scope(s => await(spawn(s, () => f(99000)))) |> type_of)"
    ));
    assert_eq!(task, entry);
}
