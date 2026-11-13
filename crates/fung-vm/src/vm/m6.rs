//! The M6 instructions: ASSERT and TEST and the test block registry. The
//! `dbg` native lives in the prelude (`prelude::dbg`).

use fung_front::bytecode::{Cmp, Op};
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::{Runtime, Stop, Task, at, discarded, err, internal};
use crate::value::{Value, eq, index, type_name};

/// One registered test block: its name, the function to run and where
/// the block stands.
#[derive(Clone, Debug)]
pub(crate) struct TestCase {
    pub name: String,
    pub func: Value,
    pub pos: Pos,
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
        let Some((f, pos)) = t.world.tests.get(i).map(|c| (c.func.clone(), c.pos)) else {
            return Err(internal(vec![Arg::Str(format!("no test block {i}"))]));
        };
        // What the block returns is discarded: an Err in it fails it (02).
        let r = match discarded(t.call_root(&f, Vec::new())) {
            Err(Stop::Error(d)) => Err(at(*d, pos)),
            r => r.map(|_| ()),
        };
        // The block runs on a fresh node below the root, cancelled when it
        // returns: what it spawned and left running ends with it.
        t.fiber.cancel.cancel();
        r
    }
}

/// Applies an assert's comparison as the operator itself would (08,
/// VM45): the same function answers, with the same help.
fn compare_by(op: Cmp, l: &Value, r: &Value) -> Result<bool, Diag> {
    let ordered = |test: fn(std::cmp::Ordering) -> bool| super::ordered(l, r).map(test);
    match op {
        Cmp::Eq => eq(l, r),
        Cmp::Ne => eq(l, r).map(|e| !e),
        Cmp::In => index::contains(r, l),
        Cmp::NotIn => index::contains(r, l).map(|found| !found),
        Cmp::Lt => ordered(std::cmp::Ordering::is_lt),
        Cmp::Gt => ordered(std::cmp::Ordering::is_gt),
        Cmp::Le => ordered(std::cmp::Ordering::is_le),
        Cmp::Ge => ordered(std::cmp::Ordering::is_ge),
    }
}

impl Task<'_> {
    /// Runs ASSERT or TEST of frame `fi`.
    pub(super) fn exec_m6(&mut self, fi: usize, op: Op, arg: usize, pos: Pos) -> Result<(), Stop> {
        let proto = self.fiber.frames[fi].proto().clone();
        match op {
            Op::Assert => {
                let a = &proto.asserts[arg];
                let Some((cmp, op_at)) = a.cmp else {
                    return match self.pop() {
                        Value::Bool(true) => Ok(()),
                        Value::Bool(false) => {
                            Err(err("E0807", pos, vec![Arg::Str(a.text.clone())]))
                        }
                        v => Err(err("E0505", pos, vec![Arg::Str(type_name(&v))])),
                    };
                };
                let r = self.pop();
                let l = self.pop();
                // A comparison that cannot be made fails where the
                // operator is, as it does outside an assert.
                if !compare_by(cmp, &l, &r).map_err(|d| at(d, op_at))? {
                    let mut d = error("E0807", pos, vec![Arg::Str(a.text.clone())]);
                    if !a.left.is_empty() {
                        d.notes
                            .push(format!("{} = {}", a.left, super::m4::shown_value(&l)));
                    }
                    if !a.right.is_empty() {
                        d.notes
                            .push(format!("{} = {}", a.right, super::m4::shown_value(&r)));
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
                    name: name.lossy().into_owned(),
                    func,
                    pos,
                });
            }
            _ => unreachable!("exec_m6 called with {}", op.name()),
        }
        Ok(())
    }
}
