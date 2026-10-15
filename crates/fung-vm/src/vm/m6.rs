//! The M6 instructions (Go `m6.go`): ASSERT and TEST, the test block
//! registry, and the `dbg` native (Go `prelude/io.go` `dbgFn`), whose
//! source text and place the compiler passes for a direct `dbg(e)` (08).

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::{Runtime, Stop, Task, at, err, internal};
use crate::value::{Native, NativeParam, Value, compare, equal, format, type_name};

/// One registered test block: its name and the function to run.
#[derive(Clone, Debug)]
pub struct TestCase {
    pub name: String,
    pub func: Value,
}

impl Runtime {
    /// The test blocks registered so far, in order. A body registers them
    /// only when it was compiled with test blocks kept (`fung test`).
    pub fn tests(&self) -> &[TestCase] {
        &self.tests
    }

    /// Runs a test function on a fresh task. An `exit(n)` inside it comes
    /// back as `Stop::Exit` so `fung test` can stop.
    pub fn call_test(&mut self, f: &Value) -> Result<Value, Stop> {
        Task::new(self).call(f, Vec::new())
    }
}

/// Applies an assert's comparison operator (08).
fn compare_by(op: &str, l: &Value, r: &Value) -> Result<bool, Diag> {
    match op {
        "==" => return Ok(equal(l, r)),
        "!=" => return Ok(!equal(l, r)),
        _ => {}
    }
    let c = compare(l, r)?;
    Ok(match op {
        "<" => c.is_lt(),
        ">" => c.is_gt(),
        "<=" => c.is_le(),
        _ => c.is_ge(),
    })
}

impl Task<'_> {
    /// Runs ASSERT or TEST of frame `fi`.
    pub(super) fn exec_m6(&mut self, fi: usize, op: Op, arg: usize, pos: Pos) -> Result<(), Stop> {
        let proto = self.frames[fi].proto.clone();
        match op {
            Op::Assert => {
                let a = &proto.asserts[arg];
                if a.op.is_empty() {
                    return match self.pop() {
                        Value::Bool(true) => Ok(()),
                        Value::Bool(false) => {
                            Err(err("E0807", pos, vec![Arg::Str(a.text.clone())]))
                        }
                        v => Err(err("E0505", pos, vec![Arg::Str(type_name(&v))])),
                    };
                }
                let r = self.pop();
                let l = self.pop();
                if !compare_by(&a.op, &l, &r).map_err(|d| at(d, pos))? {
                    let mut d = error("E0807", pos, vec![Arg::Str(a.text.clone())]);
                    if !a.left.is_empty() {
                        d.notes.push(format!("{} = {}", a.left, format(&l)));
                    }
                    if !a.right.is_empty() {
                        d.notes.push(format!("{} = {}", a.right, format(&r)));
                    }
                    return Err(Stop::Error(Box::new(d)));
                }
            }
            Op::Test => {
                let func = self.pop();
                let Value::Str(name) = &proto.consts[arg] else {
                    panic!("TEST with a name that is not a string");
                };
                self.rt.tests.push(TestCase {
                    name: name.to_string(),
                    func,
                });
            }
            _ => unreachable!("exec_m6 called with {}", op.name()),
        }
        Ok(())
    }
}

/// The `dbg` native: `dbg(x, text = nil, where = nil)`. It writes
/// `[where] text = value` (or `dbg = value` without the text) to standard
/// error and returns `x`.
pub fn dbg() -> &'static Native {
    let p = |name, default: Option<Value>| NativeParam {
        name,
        default,
        rest: false,
    };
    Native::leak(
        "dbg",
        vec![
            p("x", None),
            p("text", Some(Value::Nil)),
            p("where", Some(Value::Nil)),
        ],
        dbg_fn,
    )
}

fn dbg_fn(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let line = match (&args[1], &args[2]) {
        (Value::Str(text), Value::Str(w)) => format!("[{w}] {text} = {}", format(&args[0])),
        (Value::Str(text), _) => format!("{text} = {}", format(&args[0])),
        _ => format!("dbg = {}", format(&args[0])),
    };
    t.stderr()
        .write_all((line + "\n").as_bytes())
        .map_err(|e| internal(vec![Arg::Str(e.to_string())]))?;
    Ok(args.swap_remove(0))
}
