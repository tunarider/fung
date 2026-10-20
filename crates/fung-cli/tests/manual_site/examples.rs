//! Running the manual's examples: each in its own scratch directory,
//! checked (`fung check`, `fmt --check`) and run, its output kept for the
//! page.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::markup::{Code, Kind};
use crate::common::Reaped;

/// What an example printed (standard output and error in the order they
/// came) and its exit code.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub output: String,
    pub code: i32,
}

/// How long one step of an example may take.
const LIMIT: Duration = Duration::from_secs(20);

/// Checks and runs every example; all problems at once.
pub fn run_all(
    bin: &Path,
    examples: &[(String, &Code)],
) -> Result<HashMap<String, Outcome>, Vec<String>> {
    let results = crate::common::par_map(examples, |(label, code)| {
        one(bin, code).map_err(|e| format!("{label}: {e}"))
    });
    let mut ok = HashMap::new();
    let mut bad = Vec::new();
    for ((label, _), r) in examples.iter().zip(results) {
        match r {
            Ok(o) => {
                ok.insert(label.clone(), o);
            }
            Err(e) => bad.push(e),
        }
    }
    if bad.is_empty() { Ok(ok) } else { Err(bad) }
}

/// One example: check, format check, then its run (twice, to catch
/// output that changes from run to run).
fn one(bin: &Path, code: &Code) -> Result<Outcome, String> {
    let dir = laid_out(code)?;
    let checked = exec(bin, &dir, code, &["check", "example.fung"], false)?;
    if checked.code != 0 || checked.output.contains("error[") {
        return Err(format!("check: exit {}\n{}", checked.code, checked.output));
    }
    if matches!(code.kind, Kind::Run | Kind::Check) {
        let f = exec(bin, &dir, code, &["fmt", "--check", "example.fung"], false)?;
        if f.code != 0 {
            return Err(format!(
                "fmt --check: the example is not formatted (exit {})\n{}",
                f.code, f.output
            ));
        }
    }
    let mut args: Vec<&str> = match &code.kind {
        Kind::Run => vec!["run", "example.fung"],
        Kind::Dump(stage) => vec!["dump", stage, "example.fung"],
        _ => {
            return Ok(Outcome {
                output: String::new(),
                code: 0,
            });
        }
    };
    args.extend(code.args.iter().map(String::as_str));
    // each run starts from the fixtures alone
    let first = exec(bin, &laid_out(code)?, code, &args, true)?;
    if first.code != code.exit {
        return Err(format!(
            "exit {}, expected {} (`exit=N` on the fence)\n{}",
            first.code, code.exit, first.output
        ));
    }
    if !code.hide_output {
        let again = exec(bin, &laid_out(code)?, code, &args, true)?;
        if again != first {
            return Err(format!(
                "output differs between runs; mark the fence hide-output\n{}\n----\n{}",
                first.output, again.output
            ));
        }
    }
    Ok(first)
}

/// A fresh scratch directory with the example and its fixtures.
fn laid_out(code: &Code) -> Result<Scratch, String> {
    let dir = Scratch::new()?;
    std::fs::write(dir.0.join("example.fung"), format!("{}\n", code.src))
        .map_err(|e| e.to_string())?;
    for (name, contents) in &code.files {
        std::fs::write(dir.0.join(name), contents).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

/// Runs `fung args` in the scratch directory, standard output and error
/// through one pipe so their order is kept, the directory's path
/// replaced by `.`. The environment is fixed, so the output is the same
/// on every host.
fn exec(
    bin: &Path,
    dir: &Scratch,
    code: &Code,
    args: &[&str],
    with_input: bool,
) -> Result<Outcome, String> {
    let (mut reader, writer) = std::io::pipe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(bin);
    // nothing of the host's environment but `PATH` reaches a page: home
    // and temporary directory are the scratch directory (shown as `.`)
    cmd.args(args)
        .current_dir(&dir.0)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("TZ", crate::common::TZ)
        .env("HOME", &dir.0)
        .env("TMPDIR", &dir.0)
        .stdout(writer.try_clone().map_err(|e| e.to_string())?)
        .stderr(writer);
    for (k, v) in &code.env {
        cmd.env(k, v);
    }
    match (&code.stdin, with_input) {
        (Some(name), true) => {
            let f =
                std::fs::File::open(dir.0.join(name)).map_err(|e| format!("stdin={name}: {e}"))?;
            cmd.stdin(f);
        }
        _ => {
            cmd.stdin(Stdio::null());
        }
    }
    let child = Reaped::new(cmd.spawn().map_err(|e| e.to_string())?);
    // the parent's copies of the pipe's writing end go with `cmd`
    drop(cmd);
    let read = std::thread::spawn(move || {
        let mut b = Vec::new();
        reader.read_to_end(&mut b).map(|_| b)
    });
    let (out, killed) = child.output_within(LIMIT).map_err(|e| e.to_string())?;
    let bytes = read.join().unwrap().map_err(|e| e.to_string())?;
    if killed {
        return Err(format!(
            "`fung {}` took longer than {LIMIT:?}",
            args.join(" ")
        ));
    }
    let mut output = String::from_utf8_lossy(&bytes).into_owned();
    for p in dir.paths() {
        output = output.replace(&p, ".");
    }
    Ok(Outcome {
        output,
        code: out.status.code().unwrap_or(-1),
    })
}

/// A fresh empty directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fung-manual-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Scratch(dir))
    }

    /// The directory's path as written and as resolved (macOS's temporary
    /// directory is behind a symbolic link), longest first.
    fn paths(&self) -> Vec<String> {
        let mut v = vec![self.0.to_string_lossy().into_owned()];
        if let Ok(c) = self.0.canonicalize() {
            v.push(c.to_string_lossy().into_owned());
        }
        v.sort_by_key(|p| std::cmp::Reverse(p.len()));
        v
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
