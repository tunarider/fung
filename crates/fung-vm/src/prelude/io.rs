//! Input and output natives: `exit`, `dbg`, `eprint`, `read_line`,
//! `read_lines`. `f` is nil (standard input) or a file handle from
//! `fs.open`, read on a pool thread.

use std::sync::Arc;

use crate::native::file::{Fault, File};
use crate::native::fs::fault_value;
use crate::native::nat::{self, param};
use crate::native::pipe::{self, Step};
use crate::value::{Native, Value, format};
use crate::vm::{Flow, IoStep, Stop, Task, Wake, park_io, park_until_then};

/// `exit(code = 0)` raises the exit signal (06): the VM unwinds through
/// every `with`'s release, then the process ends with the code.
pub(super) fn exit_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    // The system keeps the low eight bits of a status, so a code outside
    // 0..=255 would end as another one: exit(256) as success (TL1).
    match nat::need_int("code", &a[0])? {
        n @ 0..=255 => Err(Stop::Exit(n)),
        n => Err(nat::fail(
            "E0403",
            vec![
                nat::s("code"),
                nat::s("an int from 0 to 255"),
                fung_front::diag::Arg::Str(n.to_string()),
            ],
        )),
    }
}

/// The prelude's `dbg(x)`, the function as a value (`map(xs, dbg)`,
/// `d = dbg`): it writes `dbg = value` to standard error and returns `x`.
pub fn dbg() -> &'static Native {
    static DBG: std::sync::OnceLock<&'static Native> = std::sync::OnceLock::new();
    DBG.get_or_init(|| Native::leak("dbg", vec![param("x")], dbg_fn))
}

/// The `dbg` a direct `dbg(e)` calls (`Const::Dbg`): the compiler passes
/// e's source text and place (08), and it writes `[where] text = value`.
/// No program can name it, so a program cannot pass those itself (SP11).
pub fn dbg_at() -> &'static Native {
    static DBG: std::sync::OnceLock<&'static Native> = std::sync::OnceLock::new();
    DBG.get_or_init(|| {
        Native::leak(
            "dbg",
            vec![param("x"), param("text"), param("where")],
            dbg_at_fn,
        )
    })
}

fn dbg_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    let line = format!("dbg = {}\n", format(&a[0]));
    t.stderr()
        .write_all(line.as_bytes())
        .map_err(nat::write_err)?;
    Ok(a.swap_remove(0))
}

fn dbg_at_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    let (Value::Str(text), Value::Str(at)) = (&a[1], &a[2]) else {
        unreachable!("the compiler passes dbg's text and place as strings")
    };
    let mut line = Vec::new();
    line.push(b'[');
    line.extend_from_slice(at);
    line.extend_from_slice(b"] ");
    line.extend_from_slice(text);
    line.extend_from_slice(format!(" = {}\n", format(&a[0])).as_bytes());
    t.stderr().write_all(&line).map_err(nat::write_err)?;
    Ok(a.swap_remove(0))
}

/// `eprint(...)` is print to standard error.
pub(super) fn eprint_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let mut line = nat::join_for_print(super::rest_items(&a[0]).iter());
    line.push(b'\n');
    t.stderr().write_all(&line).map_err(nat::write_err)?;
    Ok(Value::Nil)
}

fn trim_line(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// `read_line(f = nil)`: Ok(line), Ok(nil) at the end. A handle that
/// cannot be read gives its `Err` value. A wait for standard input parks
/// until its reader thread brings a line (`vm/stdin.rs`); a file's line
/// already in its buffer is read here, and any other read is handed off
/// (`nat::offload`).
pub(super) fn read_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    match &a[0] {
        Value::Nil => park_until_then(
            t,
            |t, _| match t.stdin_poll() {
                Ok(l) => l.map(|l| Ok(line_value(l))),
                Err(e) => Some(Err(no_reader(&e))),
            },
            STDIN_WAKE,
            |t| t.stdin_leave(),
        ),
        Value::File(h) if h.is_pipe() => pipe_lines(t, h.clone(), true),
        Value::File(h) => {
            if let Some(r) = h.read_buffered_line() {
                return file_line_value(h, r).map(Flow::Done);
            }
            let h = h.clone();
            let job = h.clone();
            nat::offload(
                t,
                move || job.read_line(),
                move |_, r| file_line_value(&h, r),
            )
        }
        f => Err(nat::wrong_arg("f", "file | nil", f)),
    }
}

/// `read_line` (`one`) or `read_lines` on a FIFO handle: read on the run
/// thread, parking in the event loop when nothing is there (`pipe.rs`,
/// L11). The lines read so far stay in the closure: a stop loses none of
/// the bytes, which stay in the handle's buffer.
fn pipe_lines(t: &mut Task<'_>, h: Arc<File>, one: bool) -> Result<Flow, Stop> {
    let mut got: Vec<Vec<u8>> = Vec::new();
    park_io(
        t,
        move |_, _| loop {
            match h.pipe_read_line() {
                Ok(Step::Ready(Some(l))) if one => {
                    return IoStep::Done(Ok(line_value(Some(l))));
                }
                Ok(Step::Ready(Some(l))) => got.push(l),
                Ok(Step::Ready(None)) if one => return IoStep::Done(Ok(line_value(None))),
                Ok(Step::Ready(None)) => {
                    return IoStep::Done(Ok(lines_value(std::mem::take(&mut got))));
                }
                Ok(Step::Wait(fd, interest)) => return pipe::io_step(fd, interest),
                Ok(Step::Fail(e)) => {
                    return IoStep::Done(fault_value(&h, "read", Fault::Io(e)));
                }
                Err(e) => return IoStep::Done(fault_value(&h, "read", e)),
            }
        },
        |_| {},
    )
}

/// `Ok([...])` of the lines `read_lines` read, each without its ending.
fn lines_value(ls: Vec<Vec<u8>>) -> Value {
    nat::ok(nat::list(
        ls.iter()
            .map(|l| Value::bytes(trim_line(l).to_vec()))
            .collect(),
    ))
}

/// E0801 when the standard input reader thread cannot start (spec 15,
/// review CC7): the input stays for a later read.
fn no_reader(e: &std::io::Error) -> Stop {
    nat::fail(
        "E0801",
        vec![fung_front::diag::Arg::Str(format!(
            "cannot start a job: {e}"
        ))],
    )
}

/// How a wait for standard input parks: the world outside wakes it (the
/// reader thread), so it is no deadlock while it waits.
const STDIN_WAKE: Wake = Wake {
    until: None,
    timed: true,
};

/// A handle's line read (or its fault) as `read_line` gives it.
fn file_line_value(h: &File, r: Result<Option<Vec<u8>>, Fault>) -> Result<Value, Stop> {
    match r {
        Ok(l) => Ok(line_value(l)),
        Err(e) => fault_value(h, "read", e),
    }
}

/// A line read (with its ending, or `None` at the end) as `read_line`
/// gives it.
fn line_value(l: Option<Vec<u8>>) -> Value {
    nat::ok(l.map_or(Value::Nil, |l| Value::bytes(trim_line(&l).to_vec())))
}

/// `read_lines(f = nil)`: every remaining line in a list. Standard input
/// parks whenever no line is queued; a handle is read on a pool thread
/// in one go (locked for each line).
pub(super) fn read_lines_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let lines = lines_value;
    match &a[0] {
        Value::Nil => {
            // A line at a time, parking whenever none is queued.
            let mut got = Vec::new();
            park_until_then(
                t,
                move |t, _| loop {
                    match t.stdin_poll() {
                        Ok(Some(Some(l))) => got.push(l),
                        Ok(Some(None)) => return Some(Ok(lines(std::mem::take(&mut got)))),
                        Ok(None) => return None,
                        Err(e) => return Some(Err(no_reader(&e))),
                    }
                },
                STDIN_WAKE,
                |t| t.stdin_leave(),
            )
        }
        Value::File(h) if h.is_pipe() => pipe_lines(t, h.clone(), false),
        Value::File(h) => {
            let h = h.clone();
            let job = h.clone();
            let read_all = move || {
                let mut ls = Vec::new();
                while let Some(l) = job.read_line()? {
                    ls.push(l);
                }
                Ok(ls)
            };
            nat::offload(t, read_all, move |_, r| match r {
                Ok(ls) => Ok(lines(ls)),
                Err(e) => fault_value(&h, "read", e),
            })
        }
        f => Err(nat::wrong_arg("f", "file | nil", f)),
    }
}
