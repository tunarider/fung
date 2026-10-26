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
    // an example that states a failing exit may be one `check` refuses
    // (a compile error); its run below shows that error for real
    let shows_failure = code.exit != 0;
    if (checked.code != 0 || checked.output.contains("error[")) && !shows_failure {
        return Err(format!("check: exit {}\n{}", checked.code, checked.output));
    }
    // `expect check`: what `fung check` printed, as the page shows it
    if code.expect_checked
        && let Some(want) = &code.expect
        && checked.output.trim_end() != want.trim_end()
    {
        return Err(format!(
            "expect check: `fung check` printed\n{}\nnot\n{want}",
            checked.output
        ));
    }
    if matches!(code.kind, Kind::Run | Kind::Check) {
        let f = exec(bin, &dir, code, &["fmt", "--check", "example.fung"], false)?;
        // the formatter cannot parse a syntax error: that is no format fault
        let unparsable = shows_failure && f.output.contains("error[");
        if f.code != 0 && !unparsable {
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
    // a failing example names the error it shows, so a wrong
    // reproduction (a missing fixture, a typo) cannot pass for it
    let shown = first
        .output
        .split("error[")
        .nth(1)
        .map(|r| r.chars().take(5).collect::<String>());
    match (&code.error, &shown) {
        (Some(want), Some(got)) if want != got => {
            return Err(format!("shows {got}, not error={want}\n{}", first.output));
        }
        (Some(want), None) => {
            return Err(format!(
                "shows no error, not error={want}\n{}",
                first.output
            ));
        }
        (None, Some(got)) if code.exit != 0 => {
            return Err(format!(
                "shows {got}: say which error it is about with `error={got}`\n{}",
                first.output
            ));
        }
        _ => {}
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
        let rel = Path::new(name);
        let inside = rel
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !inside {
            return Err(format!(
                "file {name}: a fixture stays inside the example's directory (a relative path without `..`)"
            ));
        }
        let path = dir.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, contents).map_err(|e| format!("file {name}: {e}"))?;
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
    // The pipe close-on-exec only in a second step: made, and the child
    // started, under the lock every start of the binary takes.
    let held = crate::common::starting();
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
    drop(held);
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
