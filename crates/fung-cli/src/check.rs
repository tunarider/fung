//! `fung check <file>` and `fung check -e CODE` (08): compile the whole
//! import graph with the entry's tests, run nothing. Errors exit 2;
//! warnings are printed and exit 0; a clean file prints nothing.
//! Everything goes to standard error.

use fung_front::diag::{self, EXIT_COMPILE, EXIT_OK};
use fung_front::loader::{Loader, Stage};
use fung_front::source::decode_lossy;

use crate::{Out, read_error, read_file, restore_name, usage_error};

/// `args` are the arguments after `check`, decoded; `raw` their bytes.
/// `-e` takes the code and nothing after it (nothing runs, so there is
/// no use for arguments), and a path must be the only argument (one
/// file: `check` does not walk directories).
pub fn run_check(args: &[String], raw: &[Vec<u8>]) -> Out {
    match args {
        [e, code] if e == "-e" => check(b"-e", code),
        [e, _, _, ..] if e == "-e" => usage_error(b"check -e takes the code and nothing after it"),
        [p] if p != "-e" => match read_file(&raw[0]) {
            Ok(src) => check(&raw[0], &src),
            Err(msg) => read_error(&raw[0], &msg),
        },
        _ => usage_error(b"check needs a script path or -e code"),
    }
}

/// Compiles `src` as the entry file `file` (bytes as given; positions
/// use its decoded form) and reports errors or warnings.
fn check(file: &[u8], src: &str) -> Out {
    let name = decode_lossy(file);
    let loader = Loader {
        tests: true,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(&name, src, Stage::All);
    let (shown, code) = if errs.is_empty() {
        (&b.warnings, EXIT_OK)
    } else {
        (&errs, EXIT_COMPILE)
    };
    let text: String = shown.iter().map(|e| diag::format(e, &b.sources)).collect();
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
    fn warnings_exit_zero_and_errors_two() {
        let out = check_e("fn f():\n  x = 1\n  2\n;;");
        assert_eq!(out.code, EXIT_OK);
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.starts_with("warning[W0102]: unused variable `x`\n  --> -e:2:3\n"),
            "{err}"
        );
        let out = check_e("fn f():\n  x = 1\n  y\n;;");
        assert_eq!(out.code, EXIT_COMPILE);
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.starts_with("error[E0204]") && !err.contains("W0102"),
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
            out.stderr
                .starts_with(b"error: check -e takes the code and nothing after it\n\nusage:"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
