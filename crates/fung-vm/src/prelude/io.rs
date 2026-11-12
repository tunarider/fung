//! Input and output natives: `exit`, `dbg`, `eprint`, `write`,
//! `ewrite`, `read_line`, `read_lines`. `file` is nil (standard input) or a file handle from
//! `fs.open`, read on a pool thread.

use std::sync::Arc;

use crate::native::file::{Fault, File, PipeTurn};
use crate::native::fs::fault_value;
use crate::native::nat::{self, param};
use crate::native::oserr;
use crate::native::pipe::{self, Step};
use crate::value::{Args, Native, Value, format_text};
use crate::vm::{Flow, Input, IoStep, Std, Stop, Task, Wake, park_io, park_until_then, write_std};

/// `exit(code = 0)` raises the exit signal (06): the VM unwinds through
/// every `with`'s release, then the process ends with the code.
pub(super) fn exit_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
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
    DBG.get_or_init(|| Native::leak_step("dbg", vec![param("x")], dbg_fn))
}

/// The `dbg` a direct `dbg(e)` calls (`Const::Dbg`): the compiler passes
/// e's source text and place (08), and it writes `[where] text = value`.
/// No program can name it, so a program cannot pass those itself (SP11).
pub fn dbg_at() -> &'static Native {
    static DBG: std::sync::OnceLock<&'static Native> = std::sync::OnceLock::new();
    DBG.get_or_init(|| {
        Native::leak_step(
            "dbg",
            vec![param("x"), param("text"), param("where")],
            dbg_at_fn,
        )
    })
}

fn dbg_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
    let line = format!("dbg = {}\n", format_text(&a[0])?);
    let x = a.swap_remove(0);
    write_std(t, Std::Err, line.into_bytes(), x)
}

fn dbg_at_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
    let (Value::Str(text), Value::Str(at)) = (&a[1], &a[2]) else {
        unreachable!("the compiler passes dbg's text and place as strings")
    };
    let mut line = Vec::new();
    line.push(b'[');
    line.extend_from_slice(at);
    line.extend_from_slice(b"] ");
    line.extend_from_slice(text);
    line.extend_from_slice(format!(" = {}\n", format_text(&a[0])?).as_bytes());
    let x = a.swap_remove(0);
    write_std(t, Std::Err, line, x)
}

/// `eprint(...)` is print to standard error.
pub(super) fn eprint_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let mut line = nat::join_for_print(super::rest_items(&a[0]).iter())?;
    line.push(b'\n');
    write_std(t, Std::Err, line, Value::Nil)
}

/// `write(s)`: the string as it is, no newline added (D73), through the
/// output thread as `print` (so in order with it). A string only: `print`
/// is the one that shows values.
pub(super) fn write_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let s = nat::need_bytes("s", &a[0])?.to_vec();
    write_std(t, Std::Out, s, Value::Nil)
}

/// `ewrite(s)` is write to standard error.
pub(super) fn ewrite_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let s = nat::need_bytes("s", &a[0])?.to_vec();
    write_std(t, Std::Err, s, Value::Nil)
}

fn trim_line(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// `read_line(file = nil)`: Ok(line), Ok(nil) at the end. A handle that
/// cannot be read gives its `Err` value. A wait for standard input parks
/// until its reader thread brings a line (`vm/stdin.rs`); a file's line
/// already in its buffer is read here, and any other read is handed off
/// (`nat::offload`).
pub(super) fn read_line_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    match &a[0] {
        Value::Nil => park_until_then(
            t,
            |t, _| match t.stdin_poll() {
                Ok(None) => None,
                Ok(Some(Input::Line(l))) => Some(Ok(line_value(STDIN, Some(l)))),
                Ok(Some(Input::End)) => Some(Ok(line_value(STDIN, None))),
                Ok(Some(Input::Failed(e))) => Some(Ok(stdin_err(&e))),
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
        f => Err(nat::wrong_arg("file", "file | nil", f)),
    }
}

/// `read_line` (`one`) or `read_lines` on a FIFO handle: read on the run
/// thread, parking in the event loop when nothing is there (`pipe.rs`,
/// L11). The lines read so far stay in the closure: a stop loses none of
/// the bytes, which stay in the handle's buffer. One task at a time reads
/// the handle (its read turn, CC14); the others wait in line.
fn pipe_lines(t: &mut Task<'_>, h: Arc<File>, one: bool) -> Result<Flow, Stop> {
    let mut got: Vec<Vec<u8>> = Vec::new();
    let turn = PipeTurn::new(h.clone(), false);
    let stopped = turn.clone();
    park_io(
        t,
        move |_, me| {
            if !turn.take(me) {
                return IoStep::Park;
            }
            let step = pipe_lines_step(&h, one, &mut got);
            if let IoStep::Done(_) = step {
                turn.end();
            }
            step
        },
        move |_| stopped.end(),
    )
}

/// One look of `pipe_lines`, holding the read turn.
fn pipe_lines_step(h: &File, one: bool, got: &mut Vec<Vec<u8>>) -> IoStep {
    loop {
        match h.pipe_read_line() {
            Ok(Step::Ready(Some(l))) if one => {
                return IoStep::Done(Ok(line_value(&named(h), Some(l))));
            }
            Ok(Step::Ready(Some(l))) => got.push(l),
            Ok(Step::Ready(None)) if one => return IoStep::Done(Ok(line_value("", None))),
            Ok(Step::Ready(None)) => {
                return IoStep::Done(Ok(lines_value(&named(h), std::mem::take(got))));
            }
            Ok(Step::Wait(fd, interest, at)) => return pipe::io_step(fd, interest, at),
            Ok(Step::Fail(e)) => {
                return IoStep::Done(fault_value(h, "read", Fault::Io(e)));
            }
            Err(e) => return IoStep::Done(fault_value(h, "read", e)),
        }
    }
}

/// `Ok([...])` of the lines `read_lines` read from `who`, each without
/// its ending, or the `Err` of a line past the string limit.
fn lines_value(who: &str, ls: Vec<Vec<u8>>) -> Value {
    if ls.iter().any(|l| !nat::fits_limit(trim_line(l))) {
        return line_too_long(who);
    }
    // More lines than a list holds: the read's Err too (D63, as D22).
    if ls.len() as u64 > nat::MAX_ELEMENTS {
        return nat::err_value(
            "io",
            &format!(
                "cannot read {who}: more lines than the list limit ({} elements)",
                nat::MAX_ELEMENTS
            ),
            Vec::new(),
        );
    }
    nat::ok(nat::list(
        ls.iter()
            .map(|l| Value::bytes(trim_line(l).to_vec()))
            .collect(),
    ))
}

/// What a read says it reads from: standard input, or a handle's path.
const STDIN: &str = "standard input";

fn named(h: &File) -> String {
    format!("`{}`", fung_front::text::lossy(&h.path))
}

/// A line read from outside past the string limit is the read's `Err`
/// (LB29, D22), as other failures of the input are.
fn line_too_long(who: &str) -> Value {
    nat::err_value(
        "io",
        &format!("cannot read {who}: {}", nat::too_long("line")),
        Vec::new(),
    )
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

/// A read error of standard input as `read_line` and `read_lines` give
/// it (CC10): the `Err` the other reads give (`fs`).
fn stdin_err(e: &std::io::Error) -> Value {
    nat::err_value(
        oserr::kind(e),
        &format!("cannot read standard input: {}", oserr::reason(e)),
        Vec::new(),
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
        Ok(l) => Ok(line_value(&named(h), l)),
        Err(e) => fault_value(h, "read", e),
    }
}

/// A line read (with its ending, or `None` at the end) as `read_line`
/// gives it.
fn line_value(who: &str, l: Option<Vec<u8>>) -> Value {
    match l {
        Some(l) if !nat::fits_limit(trim_line(&l)) => line_too_long(who),
        l => nat::ok(l.map_or(Value::Nil, |l| Value::bytes(trim_line(&l).to_vec()))),
    }
}

/// `read_lines(f = nil)`: every remaining line in a list. Standard input
/// parks whenever no line is queued; a handle is read on a pool thread
/// in one go (locked for each line).
pub(super) fn read_lines_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    match &a[0] {
        Value::Nil => {
            // A line at a time, parking whenever none is queued.
            let mut got = Vec::new();
            park_until_then(
                t,
                move |t, _| loop {
                    match t.stdin_poll() {
                        Ok(Some(Input::Line(l))) => got.push(l),
                        Ok(Some(Input::End)) => {
                            return Some(Ok(lines_value(STDIN, std::mem::take(&mut got))));
                        }
                        // The lines read before it went with the call.
                        Ok(Some(Input::Failed(e))) => return Some(Ok(stdin_err(&e))),
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
                Ok(ls) => Ok(lines_value(&named(&h), ls)),
                Err(e) => fault_value(&h, "read", e),
            })
        }
        f => Err(nat::wrong_arg("file", "file | nil", f)),
    }
}
