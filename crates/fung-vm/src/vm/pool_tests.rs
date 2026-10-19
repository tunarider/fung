//! The thread pool for offload jobs (concurrency v2 R3, `pool.rs`).

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::pool::Pool;

/// A latch the jobs wait on until the test opens it.
#[derive(Clone, Default)]
struct Latch(Arc<(Mutex<bool>, Condvar)>);

impl Latch {
    fn wait(&self) {
        let (m, c) = &*self.0;
        let mut open = m.lock().unwrap();
        while !*open {
            open = c.wait(open).unwrap();
        }
    }

    fn open(&self) {
        let (m, c) = &*self.0;
        *m.lock().unwrap() = true;
        c.notify_all();
    }
}

/// Waits (at most 5 s) until `cond` holds.
fn until(what: &str, cond: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !cond() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::yield_now();
    }
}

const LONG: Duration = Duration::from_secs(10);

#[test]
fn at_most_max_threads_run_and_the_rest_wait_in_order() {
    let pool = Pool::new(2, LONG);
    let latch = Latch::default();
    let started = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(0));
    for i in 0..5 {
        let (latch, started, done) = (latch.clone(), started.clone(), done.clone());
        pool.submit(Box::new(move || {
            started.lock().unwrap().push(i);
            latch.wait();
            *done.lock().unwrap() += 1;
        }))
        .unwrap();
    }
    until("two running", || started.lock().unwrap().len() == 2);
    assert_eq!((pool.threads(), pool.queued()), (2, 3));
    latch.open();
    until("all done", || *done.lock().unwrap() == 5);
    let mut all = started.lock().unwrap().clone();
    all.sort();
    assert_eq!(all, vec![0, 1, 2, 3, 4]);
}

#[test]
fn queued_jobs_start_in_order() {
    let pool = Pool::new(1, LONG);
    let latch = Latch::default();
    let started = Arc::new(Mutex::new(Vec::new()));
    for i in 0..5 {
        let (latch, started) = (latch.clone(), started.clone());
        pool.submit(Box::new(move || {
            started.lock().unwrap().push(i);
            latch.wait();
        }))
        .unwrap();
    }
    until("one running", || pool.queued() == 4);
    latch.open();
    until("all started", || started.lock().unwrap().len() == 5);
    assert_eq!(*started.lock().unwrap(), vec![0, 1, 2, 3, 4]);
}

#[test]
fn an_idle_thread_takes_the_next_job() {
    let pool = Pool::new(4, LONG);
    let done = Arc::new(Mutex::new(0));
    for n in 1..=3 {
        let d = done.clone();
        pool.submit(Box::new(move || *d.lock().unwrap() += 1))
            .unwrap();
        until("the job", || *done.lock().unwrap() == n);
        until("the thread idle", || pool.idle() == 1);
    }
    assert_eq!(pool.threads(), 1);
}

#[test]
fn an_idle_thread_ends() {
    let pool = Pool::new(4, Duration::from_millis(50));
    pool.submit(Box::new(|| {})).unwrap();
    until("the thread to end", || pool.threads() == 0);
}

#[test]
fn an_abandoned_ticket_in_the_queue_is_skipped() {
    let pool = Pool::new(1, LONG);
    let latch = Latch::default();
    let ran = Arc::new(Mutex::new(false));
    let started = Arc::new(Mutex::new(false));
    let (l, st) = (latch.clone(), started.clone());
    let first = pool
        .submit(Box::new(move || {
            *st.lock().unwrap() = true;
            l.wait()
        }))
        .unwrap();
    let r = ran.clone();
    let second = pool
        .submit(Box::new(move || *r.lock().unwrap() = true))
        .unwrap();
    // running, not only out of the queue (a job taken but not started
    // yet can still be dropped)
    until("the first running", || *started.lock().unwrap());
    assert!(!first.abandon(), "a running job is not dropped");
    assert!(second.abandon(), "a queued job is dropped");
    latch.open();
    until("the queue drained", || {
        pool.queued() == 0 && pool.idle() == 1
    });
    assert!(!*ran.lock().unwrap());
}

#[test]
fn a_panicking_job_does_not_end_its_thread() {
    let pool = Pool::new(1, LONG);
    pool.submit(Box::new(|| panic!("in a job"))).unwrap();
    let ran = Arc::new(Mutex::new(false));
    let r = ran.clone();
    pool.submit(Box::new(move || *r.lock().unwrap() = true))
        .unwrap();
    until("the next job", || *ran.lock().unwrap());
    assert_eq!(pool.threads(), 1);
}

#[test]
fn no_thread_and_none_can_start_is_an_error() {
    let pool = Pool::new(2, LONG);
    pool.fail_spawns(true);
    assert!(pool.submit(Box::new(|| {})).is_err());
    pool.fail_spawns(false);
    let latch = Latch::default();
    let l = latch.clone();
    pool.submit(Box::new(move || l.wait())).unwrap();
    until("one running", || pool.threads() == 1 && pool.idle() == 0);
    pool.fail_spawns(true);
    let ran = Arc::new(Mutex::new(false));
    let r = ran.clone();
    pool.submit(Box::new(move || *r.lock().unwrap() = true))
        .unwrap();
    latch.open();
    until("the queued job", || *ran.lock().unwrap());
    assert_eq!(pool.threads(), 1);
}

/// Abandoned jobs behind a stuck pool do not pile up in its queue.
#[test]
fn abandoned_jobs_in_a_full_queue_do_not_pile_up() {
    let pool = Pool::new(1, LONG);
    let latch = Latch::default();
    let l = latch.clone();
    pool.submit(Box::new(move || l.wait())).unwrap();
    until("the first running", || {
        pool.idle() == 0 && pool.queued() == 0
    });
    for _ in 0..10_000 {
        let t = pool.submit(Box::new(|| {})).unwrap();
        assert!(t.abandon());
    }
    assert!(pool.queued() < 200, "{} queued", pool.queued());
    latch.open();
}

/// A pool shut down (its runtime went) ends its idle threads at once,
/// not after their idle time.
#[test]
fn dropping_the_pool_ends_its_idle_threads() {
    let pool = Pool::new(4, LONG);
    let latch = Latch::default();
    for _ in 0..3 {
        let l = latch.clone();
        pool.submit(Box::new(move || l.wait())).unwrap();
    }
    until("three running", || pool.threads() == 3 && pool.idle() == 0);
    latch.open();
    until("three idle", || pool.idle() == 3);
    pool.shutdown();
    let start = std::time::Instant::now();
    until("the threads to end", || pool.threads() == 0);
    assert!(start.elapsed() < Duration::from_secs(1));
}

/// A job running when the pool shuts down runs to its end.
#[test]
fn dropping_the_pool_lets_a_running_job_finish() {
    let pool = Pool::new(1, LONG);
    let latch = Latch::default();
    let ran = Arc::new(Mutex::new(false));
    let (l, r) = (latch.clone(), ran.clone());
    pool.submit(Box::new(move || {
        l.wait();
        *r.lock().unwrap() = true;
    }))
    .unwrap();
    until("the job running", || {
        pool.idle() == 0 && pool.threads() == 1
    });
    pool.shutdown();
    latch.open();
    until("the job's end", || *ran.lock().unwrap());
    until("the thread to end", || pool.threads() == 0);
}
