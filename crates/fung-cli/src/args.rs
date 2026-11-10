//! The option parser every command shares (TL7): `-h` and `--help` print
//! the usage, `--` ends the options, an argument that starts with `-` and
//! is not an option the command knows is ``unknown option `X` `` (3), and
//! what is left is the command's positional arguments, which each command
//! checks for count and meaning.

use fung_front::diag::EXIT_OK;

use crate::{Out, USAGE, usage_error};

/// What a command accepts besides positional arguments.
pub(crate) struct Spec {
    /// Flags that stand alone (`--check`).
    pub flags: &'static [&'static str],
    /// `-e CODE` is accepted: the code is the argument after `-e`, taken
    /// as it is even if it starts with `-`.
    pub code: bool,
    /// Options end at the first positional argument: the rest belongs to
    /// the script (`fung run [--] script args...`).
    pub to_script: bool,
}

/// The arguments of a command after its options are taken out.
pub(crate) struct Parsed {
    /// The flags given, as written, in order.
    pub flags: Vec<String>,
    /// The other arguments, decoded and as bytes; `-e` and its code are
    /// not among them.
    pub args: Vec<String>,
    pub raw: Vec<Vec<u8>>,
    /// `-e` was given (only where the spec accepts it, before `--`).
    pub code_flag: bool,
    /// The code after `-e`, if there was an argument for it.
    pub code: Option<Vec<u8>>,
}

impl Parsed {
    pub fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

/// Sorts `args` (decoded) and `raw` (bytes) into flags and positional
/// arguments. `Err` is the finished command: the usage for `-h` and
/// `--help` (exit 0), a usage error for an unknown option.
pub(crate) fn parse(args: &[String], raw: &[Vec<u8>], spec: &Spec) -> Result<Parsed, Out> {
    let mut p = Parsed {
        flags: Vec::new(),
        args: Vec::new(),
        raw: Vec::new(),
        code_flag: false,
        code: None,
    };
    let mut i = 0;
    let mut options = true;
    while i < args.len() {
        let a = args[i].as_str();
        let take = |p: &mut Parsed, i: usize| {
            p.args.push(args[i].clone());
            p.raw.push(raw[i].clone());
        };
        if !options {
            take(&mut p, i);
        } else if a == "--" {
            options = false;
        } else if a == "-h" || a == "--help" {
            return Err(Out {
                stdout: USAGE.as_bytes().to_vec(),
                code: EXIT_OK,
                ..Out::default()
            });
        } else if spec.flags.contains(&a) {
            p.flags.push(a.to_string());
        } else if a == "-e" && spec.code {
            p.code_flag = true;
            if i + 1 < args.len() {
                i += 1;
                p.code = Some(raw[i].clone());
            }
        } else if a.starts_with('-') && a != "-" {
            return Err(unknown_option(&raw[i]));
        } else {
            take(&mut p, i);
            if spec.to_script {
                options = false;
            }
        }
        i += 1;
    }
    Ok(p)
}

/// ``unknown option `X` ``, the same words in every command.
pub(crate) fn unknown_option(raw: &[u8]) -> Out {
    usage_error(&[b"unknown option `", raw, b"`"].concat())
}
