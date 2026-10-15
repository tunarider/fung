//! The M6 instructions (Go `m6.go`): ASSERT and TEST and the test block
//! registry. The `dbg` native lives in the prelude (`prelude::dbg`).

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::{Runtime, Stop, Task, at, err};
use crate::value::{Value, compare, equal, format, type_name};

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
