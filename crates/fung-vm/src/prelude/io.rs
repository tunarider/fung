//! Input and output natives (Go `prelude/io.go`): `exit`, `dbg`,
//! `eprint`, `read_line`, `read_lines`. File handles come with the `fs`
//! module; until then `f` must be nil (standard input).

use crate::native::nat::{self, opt, param};
use crate::value::{Lit, Native, Value, format};
use crate::vm::{Stop, Task};

/// `exit(code = 0)` raises the exit signal (06): the VM unwinds, the
/// command runs the at_exit handlers, then the process ends with the code.
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

/// One line from standard input (`f` is nil) without its line ending;
/// `None` at the end.
fn next_line(t: &mut Task<'_>, f: &Value) -> Result<Option<String>, Stop> {
    match f {
        Value::Nil => Ok(t.read_line()?.map(|l| trim_line(&l).to_string())),
        _ => Err(nat::wrong_arg("f", "file | nil", f)),
    }
}

fn trim_line(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// `read_line(f = nil)`: Ok(line), Ok(nil) at the end.
pub(super) fn read_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(nat::ok(match next_line(t, &a[0])? {
        Some(l) => Value::str(&l),
        None => Value::Nil,
    }))
}

/// `read_lines(f = nil)`: every remaining line in a list.
pub(super) fn read_lines_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let mut lines = Vec::new();
    while let Some(l) = next_line(t, &a[0])? {
        lines.push(Value::str(&l));
    }
    Ok(nat::ok(nat::list(lines)))
}
