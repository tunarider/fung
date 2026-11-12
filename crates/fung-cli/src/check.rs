//! `fung check <file>` and `fung check -e CODE` (08): compile the whole
//! import graph with the entry's tests, run nothing. Errors exit 2; a
//! clean file prints nothing.
//! Everything goes to standard error.

use fung_front::diag::{self, EXIT_COMPILE, EXIT_OK};
use fung_front::loader::{Loader, Stage};
use fung_front::source::decode_lossy;

use crate::args::{Spec, parse};
use crate::{Out, read_error, read_file, restore_name, usage_error};

/// `args` are the arguments after `check`, decoded; `raw` their bytes.
/// `-e` takes the code and nothing after it (nothing runs, so there is
/// no use for arguments), and a path must be the only argument (one
/// file: `check` does not walk directories).
pub fn run_check(args: &[String], raw: &[Vec<u8>]) -> Out {
    const SPEC: Spec = Spec {
        flags: &[],
        code: true,
        to_script: false,
    };
    let p = match parse(args, raw, &SPEC) {
        Ok(p) => p,
        Err(out) => return out,
    };
    let raw = &p.raw;
    if p.code_flag {
        return match (&p.code, raw.is_empty()) {
            (None, _) => usage_error(b"-e needs code"),
            // Nothing runs: arguments after the code have no use (TL4).
            (Some(_), false) => {
                usage_error(b"check takes one script path or -e code, nothing more")
            }
            (Some(code), true) => match crate::code_arg(code) {
                Ok(code) => check(b"-e", &code),
                Err(out) => out,
            },
        };
    }
    match &raw[..] {
        [] => usage_error(b"check needs a script path or -e code"),
        [path] => match read_file(path) {
            Ok(src) => check(path, &src),
            Err(fail) => read_error(path, &fail),
        },
        _ => usage_error(b"check takes one script path or -e code, nothing more"),
    }
}

/// Compiles `src` as the entry file `file` (bytes as given; positions
/// use its decoded form) and reports its errors.
fn check(file: &[u8], src: &str) -> Out {
    let name = decode_lossy(file);
    let loader = Loader {
        tests: true,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(&name, src, Stage::All);
    let code = if errs.is_empty() {
        EXIT_OK
    } else {
        EXIT_COMPILE
    };
    let text: String = errs.iter().map(|e| diag::format(e, &b.sources)).collect();
    Out {
        stderr: restore_name(&text, &name, file),
        code,
        ..Out::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fung_front::diag::EXIT_USAGE;

    fn check_e(src: &str) -> Out {
        run_check(
            &["-e".into(), src.into()],
            &[b"-e".to_vec(), src.as_bytes().to_vec()],
        )
    }

    #[test]
    fn errors_exit_two_and_a_clean_file_zero() {
        let out = check_e("fn f():\n  x = 1\n  2\n;;");
        assert_eq!(out.code, EXIT_COMPILE);
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.starts_with("error[E0239]: unused variable `x`\n  --> -e:2:3\n"),
            "{err}"
        );
        let out = check_e("print(1)");
        assert_eq!((out.code, out.stderr.len()), (EXIT_OK, 0));
    }

    #[test]
    fn arguments() {
        let out = run_check(&[], &[]);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(
            out.stderr
                .starts_with(b"error: check needs a script path or -e code\n\nusage:")
        );
        let two = ["a".to_string(), "b".to_string()];
        assert_eq!(
            run_check(&two, &[b"a".to_vec(), b"b".to_vec()]).code,
            EXIT_USAGE
        );
        // nothing runs, so arguments after the code have no use: an error,
        // not silently dropped
        let extra = ["-e".to_string(), "1".to_string(), "x".to_string()];
        let raw: Vec<Vec<u8>> = extra.iter().map(|a| a.as_bytes().to_vec()).collect();
        let out = run_check(&extra, &raw);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(
            out.stderr.starts_with(
                b"error: check takes one script path or -e code, nothing more\n\nusage:"
            ),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
