//! The M6 instructions: ASSERT and TEST and the test block registry. The
//! `dbg` native lives in the prelude (`prelude::dbg`).

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::{Runtime, Stop, Task, at, err, internal};
use crate::value::{Value, compare, equal, format, type_name};

/// One registered test block: its name and the function to run.
#[derive(Clone, Debug)]
pub(crate) struct TestCase {
    pub name: String,
    pub func: Value,
}

impl Runtime {
    /// The names of the test blocks registered so far, in order. A body
    /// registers them only when it was compiled with test blocks kept
    /// (`fung test`). The functions stay in the runtime: `call_test`
    /// runs one by its index here.
    pub fn tests(&self) -> Vec<String> {
        self.world().tests.iter().map(|t| t.name.clone()).collect()
    }

    /// Runs test block `i` on a fresh task. An `exit(n)` inside it comes
    /// back as `Stop::Exit` so `fung test` can stop.
    pub fn call_test(&self, i: usize) -> Result<(), Stop> {
        let mut t = self.child_task();
        let Some(f) = t.world.tests.get(i).map(|c| c.func.clone()) else {
            return Err(internal(vec![Arg::Str(format!("no test block {i}"))]));
        };
        let r = t.call_root(&f, Vec::new()).map(|_| ());
        // The block runs on a fresh node below the root, cancelled when it
        // returns: what it spawned and left running ends with it.
        t.cancel.cancel();
        r
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
                self.world.tests.push(TestCase {
                    name: name.to_string(),
                    func,
                });
            }
            _ => unreachable!("exec_m6 called with {}", op.name()),
        }
        Ok(())
    }
}
