//! The debug mark of the run token (stage 3a): whether this thread holds
//! the lock that makes it the one thread allowed to touch fung values.
//!
//! The run token is `World`'s mutex (see `world.rs`). fung values hold
//! `Rc`s, whose counts are not atomic, so creating, reading, cloning or
//! dropping one is only sound on the thread that holds the token. The
//! compiler cannot check that rule for us (the wrappers that let values
//! cross threads are `unsafe impl Send`), so debug builds keep a
//! thread-local flag, set exactly when the guard is acquired and cleared
//! just before it is released, and assert it at the boundaries: a task's
//! start and end, the points where a task blocks, and the runtime's drop.
//! Release builds compile all of this away.

#[cfg(debug_assertions)]
use std::cell::Cell;

#[cfg(debug_assertions)]
thread_local! {
    static HELD: Cell<bool> = const { Cell::new(false) };
}

/// Panics (debug builds only) unless this thread holds the run token.
#[inline]
pub(crate) fn assert_holds_token() {
    #[cfg(debug_assertions)]
    assert!(
        HELD.with(Cell::get),
        "fung values touched without the run token"
    );
}

/// Panics (debug builds only) if this thread already holds the token:
/// taking it again would deadlock on the mutex, so the check, made before
/// locking, turns that bug into a panic that names the cause.
#[inline]
pub(super) fn assert_not_held() {
    #[cfg(debug_assertions)]
    assert!(!HELD.with(Cell::get), "run token taken twice on one thread");
}

/// Marks the token taken by this thread, right after the mutex is locked.
#[inline]
pub(super) fn mark_taken() {
    #[cfg(debug_assertions)]
    HELD.with(|h| h.set(true));
}

/// Marks the token given up by this thread. Called before the guard is
/// released, never after: once the mutex is free another thread may hold
/// the token, and this thread must already count as not holding it.
#[inline]
pub(super) fn mark_released() {
    #[cfg(debug_assertions)]
    HELD.with(|h| h.set(false));
}
