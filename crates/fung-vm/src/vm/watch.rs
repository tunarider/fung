//! Waiting on the operating system with cancellation (stage 4d): what a
//! task blocked in a socket call (`accept`, a read, a write, `connect`,
//! a name lookup) waits with, so that a cancellation wakes it at once.
//!
//! **How.** The task makes the call on a non-blocking descriptor outside
//! the run token (`outside_watched`). When the call would block, it
//! `poll`s that descriptor together with the read end of a pipe (the
//! *wake pipe*). The pipe's write end is moved into a hook registered on
//! the task's cancellation node (`CancelNode::on_cancel`); cancelling the
//! node (`cancel(t)`, `exit` from any task, a reported deadlock, the end
//! of a spawning task) runs the hook, which writes one byte: the poll
//! returns and the wait ends as cancelled. The caller then unwinds with
//! E0701 (E0702 after a deadlock report) once the token is back.
//!
//! Other ways considered: `shutdown` on a `try_clone` of the socket
//! wakes a blocked read, but not an `accept` on macOS nor a `connect` in
//! progress, and it ends the connection for every task holding it; a
//! short poll in a loop wakes late and costs wakeups for every idle
//! connection. The pipe wakes any wait the same way and touches no
//! socket.
//!
//! **Races.**
//! - A cancel before the hook is registered: `on_cancel` checks the flag
//!   under the node's lock, after `cancel` set it and before `cancel`
//!   takes that lock, so the registration either fails (the wait returns
//!   `Cancelled` without polling) or comes first and the hook runs.
//! - A cancel after the hook is registered but before `poll` starts: the
//!   byte stays in the pipe, so the poll returns at once.
//! - The hook runs on the cancelling thread while this task uses the
//!   socket: the hook only writes to its own pipe and never touches the
//!   socket, so there is nothing to race with.
//! - The wait ends (ready, timed out) while a cancel runs the hook: the
//!   hook owns the write end, so the write goes to a live pipe; the read
//!   end lives until `Watch` drops, which unregisters the hook under the
//!   node's lock first (`remove_hook`: afterwards the hook is not running
//!   and never will).
//!
//! **Leaks.** No thread is made. The two pipe descriptors live while the
//! `Watch` does (one per blocking call, made only when the call would
//! block): the read end in the `Watch`, the write end in the hook, which
//! is dropped when it runs or when the `Watch` unregisters it. A
//! cancelled wait therefore leaves nothing behind once the native
//! returns; the socket itself belongs to the fung value, which closes it
//! when its last copy goes.

use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use super::block::outside;
use super::task::CancelNode;
use super::{Stop, Task};

/// Hook ids, unique in the process.
static NEXT_HOOK: AtomicU64 = AtomicU64::new(1);

/// Wake pipes alive now (a leak check for the tests).
static LIVE_WAKES: AtomicUsize = AtomicUsize::new(0);

/// How many wake pipes are open now (test probe).
#[cfg(test)]
pub(crate) fn live_wakes() -> usize {
    LIVE_WAKES.load(Ordering::SeqCst)
}

/// What a descriptor is waited for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Interest {
    Read,
    Write,
}

/// How a wait ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Waited {
    /// One of the descriptors is ready (or failed: the call says how).
    Ready,
    /// The deadline passed.
    TimedOut,
    /// The task was cancelled: give up and return; the caller unwinds.
    Cancelled,
}

/// A task's wait on the operating system, cancellable (see the module
/// comment). Made with the token held (`outside_watched`), used without
/// it: it holds no fung value.
pub(crate) struct Watch {
    node: Arc<CancelNode>,
    /// The hook's id and the pipe's read end, made at the first wait.
    wake: Option<(u64, io::PipeReader)>,
    /// A wait ended as cancelled.
    stopped: bool,
}

impl Watch {
    pub(crate) fn new(node: Arc<CancelNode>) -> Watch {
        Watch {
            node,
            wake: None,
            stopped: false,
        }
    }

    /// Whether the task is cancelled; true once a wait said so.
    pub(crate) fn cancelled(&mut self) -> bool {
        if self.node.is_cancelled() {
            self.stopped = true;
        }
        self.stopped
    }

    /// Waits until one of `fds` is ready for its interest (or reports an
    /// error or hang-up), `deadline` passes, or the task is cancelled.
    /// Cancellation wins over a descriptor that became ready at the same
    /// time. The error is only a failed `pipe` or `poll`.
    pub(crate) fn wait(
        &mut self,
        fds: &[(RawFd, Interest)],
        deadline: Option<Instant>,
    ) -> io::Result<Waited> {
        if self.cancelled() {
            return Ok(Waited::Cancelled);
        }
        let wake_fd = match &self.wake {
            Some((_, r)) => r.as_raw_fd(),
            None => {
                let (r, mut w) = io::pipe()?;
                LIVE_WAKES.fetch_add(1, Ordering::SeqCst);
                let id = NEXT_HOOK.fetch_add(1, Ordering::Relaxed);
                // One byte into an empty pipe never blocks; an error
                // cannot happen while the read end lives (it does until
                // the hook is unregistered or has run).
                let hook = Box::new(move || {
                    let _ = w.write(&[1]);
                });
                if !self.node.on_cancel(id, hook) {
                    drop(r);
                    LIVE_WAKES.fetch_sub(1, Ordering::SeqCst);
                    self.stopped = true;
                    return Ok(Waited::Cancelled);
                }
                let fd = r.as_raw_fd();
                self.wake = Some((id, r));
                fd
            }
        };
        let mut pfds: Vec<libc::pollfd> = fds
            .iter()
            .map(|&(fd, interest)| libc::pollfd {
                fd,
                events: match interest {
                    Interest::Read => libc::POLLIN,
                    Interest::Write => libc::POLLOUT,
                },
                revents: 0,
            })
            .collect();
        pfds.push(libc::pollfd {
            fd: wake_fd,
            events: libc::POLLIN,
            revents: 0,
        });
        loop {
            let timeout = match deadline {
                None => -1,
                Some(at) => {
                    let now = Instant::now();
                    if now >= at {
                        return Ok(Waited::TimedOut);
                    }
                    // rounded up, so the wait never ends early and spins
                    let ms = (at - now).as_nanos().div_ceil(1_000_000);
                    ms.min(i32::MAX as u128) as i32
                }
            };
            // SAFETY: `pfds` is a live, initialised array of `len`
            // pollfd structs, and every descriptor in it stays open for
            // the call (the caller holds its socket; `self` holds the
            // pipe's read end).
            let n = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            let (wake, rest) = pfds.split_last().expect("the wake pipe");
            // Only the hook writes to the pipe, and only after the flag
            // is set: a readable pipe is a cancellation. (So is a closed
            // write end, which happens only once the hook has run: an
            // unrun hook is dropped by `Drop` alone, after the waits.)
            if wake.revents != 0 {
                self.stopped = true;
                return Ok(Waited::Cancelled);
            }
            if rest.iter().any(|p| p.revents != 0) {
                return Ok(Waited::Ready);
            }
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some((id, r)) = self.wake.take() {
            self.node.remove_hook(id);
            drop(r);
            LIVE_WAKES.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// `outside` for a call that waits on the operating system through a
/// `Watch` on the task's cancellation node: `f` runs without the token;
/// if one of its waits ended as cancelled (or it saw the task
/// cancelled), its result is dropped and the task unwinds with E0701
/// (E0702 once a deadlock was reported), like `block`.
pub(crate) fn outside_watched<T: Send>(
    t: &mut Task<'_>,
    f: impl FnOnce(&mut Watch) -> T + Send,
) -> Result<T, Stop> {
    let mut w = Watch::new(t.cancel.clone());
    let r = outside(t, || f(&mut w));
    if w.stopped {
        return Err(t.unwound());
    }
    Ok(r)
}

/// The tests that count wake pipes run one at a time (the count is
/// process-wide): this module's and `native/net_tests.rs`.
#[cfg(test)]
pub(crate) fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::time::Duration;

    use super::*;

    /// A pipe nothing is written to: waiting for it to be readable blocks.
    fn quiet() -> (io::PipeReader, io::PipeWriter) {
        io::pipe().unwrap()
    }

    #[test]
    fn cancel_wakes_a_socket_wait_from_another_thread() {
        let _g = serial();
        let node = CancelNode::root();
        let (r, _w) = quiet();
        let mut watch = Watch::new(node.clone());
        let fd = r.as_raw_fd();
        let canceller = {
            let node = node.clone();
            std::thread::spawn(move || {
                // by state: the hook is registered once the wait started
                while live_wakes() == 0 {
                    std::thread::yield_now();
                }
                node.cancel();
            })
        };
        let start = Instant::now();
        let got = watch.wait(&[(fd, Interest::Read)], None).unwrap();
        canceller.join().unwrap();
        assert_eq!(got, Waited::Cancelled);
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(watch.cancelled());
        drop(watch);
        assert_eq!(live_wakes(), 0);
    }

    #[test]
    fn a_cancelled_task_does_not_wait_or_make_a_pipe() {
        let _g = serial();
        let node = CancelNode::root();
        node.cancel();
        let (r, _w) = quiet();
        let mut watch = Watch::new(node);
        let got = watch.wait(&[(r.as_raw_fd(), Interest::Read)], None);
        assert_eq!(got.unwrap(), Waited::Cancelled);
        assert_eq!(live_wakes(), 0);
    }

    /// A cancel between the registration and the poll leaves its byte in
    /// the pipe: the next wait returns at once.
    #[test]
    fn a_cancel_between_waits_is_not_lost() {
        let _g = serial();
        let node = CancelNode::root();
        let (r, _w) = quiet();
        let mut watch = Watch::new(node.clone());
        let fd = r.as_raw_fd();
        let soon = Some(Instant::now() + Duration::from_millis(1));
        assert_eq!(
            watch.wait(&[(fd, Interest::Read)], soon).unwrap(),
            Waited::TimedOut
        );
        node.cancel();
        // the flag check is bypassed to show the pipe itself wakes
        assert_eq!(
            watch.poll_only(&[(fd, Interest::Read)]).unwrap(),
            Waited::Cancelled
        );
        drop(watch);
        assert_eq!(live_wakes(), 0);
    }

    #[test]
    fn ready_and_deadline() {
        let _g = serial();
        let node = CancelNode::root();
        let (mut r, mut w) = quiet();
        let mut watch = Watch::new(node);
        let fd = r.as_raw_fd();
        w.write_all(b"x").unwrap();
        assert_eq!(
            watch.wait(&[(fd, Interest::Read)], None).unwrap(),
            Waited::Ready
        );
        let mut b = [0u8; 1];
        r.read_exact(&mut b).unwrap();
        let start = Instant::now();
        let at = Instant::now() + Duration::from_millis(20);
        assert_eq!(
            watch.wait(&[(fd, Interest::Read)], Some(at)).unwrap(),
            Waited::TimedOut
        );
        assert!(Instant::now() >= at && start.elapsed() < Duration::from_secs(5));
        assert_eq!(
            watch
                .wait(&[(w.as_raw_fd(), Interest::Write)], None)
                .unwrap(),
            Waited::Ready
        );
        drop(watch);
        assert_eq!(live_wakes(), 0);
    }

    /// The hook is gone once the watch is: a later cancel runs nothing.
    #[test]
    fn dropping_the_watch_unregisters_its_hook() {
        let _g = serial();
        let node = CancelNode::root();
        let (r, _w) = quiet();
        let mut watch = Watch::new(node.clone());
        let soon = Some(Instant::now() + Duration::from_millis(1));
        watch
            .wait(&[(r.as_raw_fd(), Interest::Read)], soon)
            .unwrap();
        assert_eq!(node.hook_count(), 1);
        drop(watch);
        assert_eq!(node.hook_count(), 0);
        node.cancel();
        assert_eq!(live_wakes(), 0);
    }

    impl Watch {
        /// `wait` without the flag check first (tests).
        fn poll_only(&mut self, fds: &[(RawFd, Interest)]) -> io::Result<Waited> {
            let stopped = std::mem::replace(&mut self.stopped, false);
            let node = std::mem::replace(&mut self.node, CancelNode::root());
            let r = self.wait(fds, None);
            self.node = node;
            self.stopped |= stopped;
            r
        }
    }
}
