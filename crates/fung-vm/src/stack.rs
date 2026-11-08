//! Starting a thread with a big stack, and with smaller ones when the
//! operating system refuses the reservation (docs/notes/R3a-runtime.md
//! 2절 "스택 크기", 6a 마무리). A stack is reserved address space, but a
//! host with little memory and swap, strict overcommit or `ulimit -v`
//! may refuse a reservation of gigabytes. Such a host then runs with a
//! smaller stack: only the front end's recursion on a very long flat
//! chain in the source (about 1 KiB a link in a release build) hits its
//! lower limit; values and calls use no Rust stack for their depth (L9).

use std::io;
use std::sync::{Arc, Mutex};
use std::thread::{Builder, JoinHandle};

/// The stack sizes the `fung` command thread tries, in order. 4 GiB is
/// only reserved address space, for the front end's recursion on long
/// flat chains (R3a 2절, L9). Since R1b every task runs on this thread
/// (`vm/sched.rs`); fung calls use no Rust stack (frames and
/// continuations live on the heap), nor do the walks of values (L9).
pub const ENTRY_STACKS: &[usize] = &[4 << 30, 1 << 30, 256 << 20, 64 << 20];

/// Makes glibc's malloc keep one arena (L15 P14). glibc gives a thread
/// other than the main one an arena of its own, and fung runs all its
/// work on such a thread (the run thread): in a Docker arm64 VM an
/// allocation-heavy loop (`bench/loop.fung`) took 0.447 s that way and
/// 0.284 s with one arena, as the musl build does (0.281 s). Set once,
/// before the first thread starts. Other C libraries have no arenas.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn one_malloc_arena() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: mallopt only changes the allocator's settings; it is
        // called before this process starts its other threads.
        unsafe {
            libc::mallopt(libc::M_ARENA_MAX, 1);
        }
    });
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn one_malloc_arena() {}

/// Starts `body(payload)` on a thread made by `builder` with the first
/// stack size in `sizes` the operating system accepts. `Builder::spawn`
/// drops its closure when it fails, so the payload waits in a shared
/// slot that each attempt's closure only points to; if every size is
/// refused, the payload comes back with the last error, to be dropped
/// by the caller on its own thread.
pub fn spawn_sized<P: Send + 'static, T: Send + 'static>(
    builder: impl Fn() -> Builder,
    sizes: &[usize],
    payload: P,
    body: fn(P) -> T,
) -> Result<JoinHandle<T>, (io::Error, P)> {
    one_malloc_arena();
    let slot = Arc::new(Mutex::new(Some(payload)));
    let mut last = io::Error::other("no stack size to try");
    for &size in sizes {
        let s = Arc::clone(&slot);
        let started = builder().stack_size(size).spawn(move || {
            let p = s
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .expect("a thread payload taken twice");
            drop(s);
            body(p)
        });
        match started {
            Ok(h) => return Ok(h),
            Err(e) => last = e,
        }
    }
    let p = slot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .expect("a payload kept after every refused spawn");
    Err((last, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// More than any 64-bit host maps: every system refuses it.
    pub(crate) const REFUSED: usize = 1 << 47;

    #[test]
    fn a_refused_size_falls_back_to_the_next() {
        let h = spawn_sized(Builder::new, &[REFUSED, REFUSED, 64 << 20], 20, |n: i32| {
            n + 1
        })
        .map_err(|(e, _)| e)
        .expect("the 64 MiB attempt starts");
        assert_eq!(h.join().unwrap(), 21);
    }

    #[test]
    fn every_size_refused_returns_the_payload_and_the_error() {
        let (e, p) = spawn_sized(
            Builder::new,
            &[REFUSED, REFUSED],
            String::from("kept"),
            |s| s,
        )
        .expect_err("no attempt starts");
        assert_eq!(p, "kept");
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn the_sizes_shrink() {
        for w in ENTRY_STACKS.windows(2) {
            assert!(w[0] > w[1]);
        }
        assert_eq!(*ENTRY_STACKS.last().unwrap(), 64 << 20);
    }
}
