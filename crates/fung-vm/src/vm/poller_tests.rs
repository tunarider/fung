//! The event loop core (concurrency v2 R4a, `poller.rs`), on real pipes
//! and sockets.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::poller::{Interest, Poller};

fn wait(p: &Poller, timeout: Duration) -> Vec<u64> {
    let mut out = Vec::new();
    p.wait(&mut out, Some(timeout)).unwrap();
    out
}

#[test]
fn a_registered_read_fires_once() {
    let p = Poller::new().unwrap();
    let (r, mut w) = std::io::pipe().unwrap();
    p.register(r.as_raw_fd(), Interest::Read, 7).unwrap();
    assert_eq!(wait(&p, Duration::ZERO), Vec::<u64>::new());
    w.write_all(b"x").unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![7]);
    // One-shot: still readable, but not registered again.
    assert_eq!(wait(&p, Duration::from_millis(20)), Vec::<u64>::new());
    p.register(r.as_raw_fd(), Interest::Read, 8).unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![8]);
}

#[test]
fn write_interest_fires_on_a_writable_socket() {
    let p = Poller::new().unwrap();
    let (a, _b) = UnixStream::pair().unwrap();
    p.register(a.as_raw_fd(), Interest::Write, 3).unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![3]);
}

#[test]
fn read_and_write_on_one_socket_are_apart() {
    let p = Poller::new().unwrap();
    let (a, mut b) = UnixStream::pair().unwrap();
    p.register(a.as_raw_fd(), Interest::Read, 1).unwrap();
    p.register(a.as_raw_fd(), Interest::Write, 2).unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![2]);
    b.write_all(b"x").unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![1]);
}

#[test]
fn the_timeout_ends_an_empty_wait() {
    let p = Poller::new().unwrap();
    let start = Instant::now();
    assert_eq!(wait(&p, Duration::from_millis(20)), Vec::<u64>::new());
    assert!(start.elapsed() >= Duration::from_millis(20));
}

#[test]
fn a_waker_from_another_thread_ends_the_wait() {
    let p = Poller::new().unwrap();
    let w = p.waker();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        w.wake();
    });
    let mut out = Vec::new();
    let start = Instant::now();
    p.wait(&mut out, None).unwrap();
    assert!(out.is_empty());
    assert!(start.elapsed() < Duration::from_secs(5));
    t.join().unwrap();
    // A wake before the wait ends the next wait at once, once.
    p.waker().wake();
    p.waker().wake();
    let start = Instant::now();
    p.wait(&mut out, Some(Duration::from_secs(5))).unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    let start = Instant::now();
    p.wait(&mut out, Some(Duration::from_millis(20))).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(20));
}

#[test]
fn a_deregistered_fd_does_not_fire() {
    let p = Poller::new().unwrap();
    let (r, mut w) = std::io::pipe().unwrap();
    p.register(r.as_raw_fd(), Interest::Read, 5).unwrap();
    p.deregister(r.as_raw_fd(), Interest::Read);
    w.write_all(b"x").unwrap();
    assert_eq!(wait(&p, Duration::from_millis(20)), Vec::<u64>::new());
    // Deregistering what is not registered is quiet.
    p.deregister(r.as_raw_fd(), Interest::Read);
    p.deregister(r.as_raw_fd(), Interest::Write);
}

#[test]
fn a_hang_up_fires_the_read_token() {
    let p = Poller::new().unwrap();
    let (r, w) = std::io::pipe().unwrap();
    p.register(r.as_raw_fd(), Interest::Read, 9).unwrap();
    drop(w);
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![9]);
}

#[test]
fn registering_again_replaces_the_token() {
    let p = Poller::new().unwrap();
    let (r, mut w) = std::io::pipe().unwrap();
    p.register(r.as_raw_fd(), Interest::Read, 1).unwrap();
    p.register(r.as_raw_fd(), Interest::Read, 2).unwrap();
    w.write_all(b"x").unwrap();
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![2]);
}

/// A write wait on a socket whose peer closed its writing half (and
/// whose own send buffer is full) does not fire: the peer's end of
/// stream is a read event, and a write wait must not spin on it.
#[test]
fn a_peer_half_close_does_not_fire_a_write_wait() {
    use std::io::ErrorKind;
    let p = Poller::new().unwrap();
    let (mut a, b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap();
    let chunk = [0u8; 65536];
    loop {
        match a.write(&chunk) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("{e}"),
        }
    }
    b.shutdown(std::net::Shutdown::Write).unwrap();
    p.register(a.as_raw_fd(), Interest::Write, 4).unwrap();
    let start = Instant::now();
    let mut fired = 0;
    while start.elapsed() < Duration::from_millis(50) {
        fired += wait(&p, Duration::from_millis(50)).len();
    }
    assert_eq!(fired, 0);
    drop(b);
}

/// The waits a loop look ends early: a write wait on such a socket does
/// not make `wait` return at once again and again.
#[test]
fn a_half_closed_peer_does_not_wake_the_loop() {
    use std::io::ErrorKind;
    let p = Poller::new().unwrap();
    let (mut a, b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap();
    let chunk = [0u8; 65536];
    while !matches!(a.write(&chunk), Err(e) if e.kind() == ErrorKind::WouldBlock) {}
    b.shutdown(std::net::Shutdown::Write).unwrap();
    p.register(a.as_raw_fd(), Interest::Write, 4).unwrap();
    let start = Instant::now();
    let mut returns = 0;
    while start.elapsed() < Duration::from_millis(50) {
        wait(&p, Duration::from_millis(50));
        returns += 1;
    }
    assert!(returns <= 2, "the loop woke {returns} times");
    drop(b);
}

/// The end of a child comes as its exit token.
#[test]
fn an_exited_child_fires_its_exit_token() {
    let p = Poller::new().unwrap();
    let mut c = std::process::Command::new("sleep")
        .arg("0.05")
        .spawn()
        .unwrap();
    let w = p.exit_watch(c.id() as libc::pid_t).unwrap();
    assert!(p.register_exit(&w, 9).unwrap());
    assert_eq!(wait(&p, Duration::from_secs(5)), vec![9]);
    c.wait().unwrap();
}

/// A child that ended (not reaped yet) before its end is registered is
/// seen at once: the registration says so, or its token comes at once.
#[test]
fn a_child_that_already_exited_fires_at_once() {
    let p = Poller::new().unwrap();
    let mut c = std::process::Command::new("true").spawn().unwrap();
    let pid = c.id() as libc::pid_t;
    // SAFETY: an all-zero `siginfo_t` is a valid value to be filled in;
    // `WNOWAIT` leaves the child to reap below.
    unsafe {
        let mut info: libc::siginfo_t = std::mem::zeroed();
        assert_eq!(
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOWAIT
            ),
            0
        );
    }
    let w = p.exit_watch(pid).unwrap();
    if p.register_exit(&w, 9).unwrap() {
        assert_eq!(wait(&p, Duration::from_secs(1)), vec![9]);
    }
    c.wait().unwrap();
}
