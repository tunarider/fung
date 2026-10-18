//! Input and output natives: `exit`, `dbg`, `eprint`, `read_line`,
//! `read_lines`. `f` is nil (standard input) or a file handle from
//! `fs.open`, read without the run token.

use fung_front::source::decode_lossy;

use crate::native::fs::fault_value;
use crate::native::nat::{self, opt, param};
use crate::value::{File, Lit, Native, Value, format};
use crate::vm::{Stop, Task, outside};

/// `exit(code = 0)` raises the exit signal (06): the VM unwinds through
/// every `with`'s release, then the process ends with the code.
pub(super) fn exit_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Err(Stop::Exit(nat::need_int("code", &a[0])?))
}

/// The `dbg` native: `dbg(x, text = nil, where = nil)`. It writes
/// `[where] text = value` (or `dbg = value` without the text) to standard
/// error and returns `x`. The compiler passes the source text and place
/// for a direct `dbg(e)` call (08).
pub fn dbg() -> &'static Native {
    static DBG: std::sync::OnceLock<&'static Native> = std::sync::OnceLock::new();
    DBG.get_or_init(|| {
        Native::leak(
            "dbg",
            vec![param("x"), opt("text", Lit::Nil), opt("where", Lit::Nil)],
            dbg_fn,
        )
    })
}

fn dbg_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    let line = match (&a[1], &a[2]) {
        (Value::Str(text), Value::Str(w)) => format!("[{w}] {text} = {}", format(&a[0])),
        (Value::Str(text), _) => format!("{text} = {}", format(&a[0])),
        _ => format!("dbg = {}", format(&a[0])),
    };
    t.stderr()
        .write_all((line + "\n").as_bytes())
        .map_err(nat::write_err)?;
    Ok(a.swap_remove(0))
}

/// `eprint(...)` is print to standard error.
pub(super) fn eprint_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let line = nat::join_for_print(super::rest_items(&a[0]).iter()) + "\n";
    t.stderr()
        .write_all(line.as_bytes())
        .map_err(nat::write_err)?;
    Ok(Value::Nil)
}

/// One line from standard input (`f` is nil) or a file handle, without
/// its line ending; `None` at the end. A handle that cannot be read gives
/// its `Err` value.
fn next_line(t: &mut Task<'_>, f: &Value) -> Result<Result<Option<String>, Value>, Stop> {
    match f {
        Value::Nil => Ok(Ok(t.read_line()?.map(|l| trim_line(&l).to_string()))),
        Value::File(h) => {
            let h: &File = h;
            Ok(match outside(t, || h.read_line()) {
                Ok(l) => Ok(l.map(|l| trim_line(&decode_lossy(&l)).to_string())),
                Err(e) => Err(fault_value(h, "read", e)),
            })
        }
        _ => Err(nat::wrong_arg("f", "file | nil", f)),
    }
}

/// Every remaining line of a handle, read without the token in one go
/// (the handle is locked for each line, as `next_line` locks it).
fn handle_lines(t: &mut Task<'_>, h: &File) -> Result<Vec<Value>, Value> {
    let r = outside(t, || {
        let mut lines = Vec::new();
        while let Some(l) = h.read_line()? {
            lines.push(l);
        }
        Ok(lines)
    });
    match r {
        Ok(lines) => Ok(lines
            .iter()
            .map(|l| Value::str(trim_line(&decode_lossy(l))))
            .collect()),
        Err(e) => Err(fault_value(h, "read", e)),
    }
}

fn trim_line(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// `read_line(f = nil)`: Ok(line), Ok(nil) at the end.
pub(super) fn read_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(match next_line(t, &a[0])? {
        Ok(Some(l)) => nat::ok(Value::str(&l)),
        Ok(None) => nat::ok(Value::Nil),
        Err(e) => e,
    })
}

/// `read_lines(f = nil)`: every remaining line in a list.
pub(super) fn read_lines_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    if let Value::File(h) = &a[0] {
        return Ok(match handle_lines(t, h) {
            Ok(lines) => nat::ok(nat::list(lines)),
            Err(e) => e,
        });
    }
    let mut lines = Vec::new();
    loop {
        match next_line(t, &a[0])? {
            Ok(Some(l)) => lines.push(Value::str(&l)),
            Ok(None) => break,
            Err(e) => return Ok(e),
        }
    }
    Ok(nat::ok(nat::list(lines)))
}
