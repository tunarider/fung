//! The permissions of the files a program makes: every way of making one
//! (`fs.write`, `fs.append`, `fs.open` for `'write` and `'append`) creates
//! it with 0666 less the umask, as the shell's `>` and `>>` do. The child
//! runs under umask 000, so the mode is 0666 itself.

mod common;

use std::os::unix::fs::PermissionsExt;

#[test]
fn new_files_take_0666_less_the_umask() {
    let dir = std::env::temp_dir().join(format!("fung-modes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.to_string_lossy().into_owned();
    let program = format!(
        "fs.write(\"{d}/w\", \"x\")!\nfs.append(\"{d}/a\", \"x\")!\nfs.close(fs.open(\"{d}/ow\", 'write)!)!\nfs.close(fs.open(\"{d}/oa\", 'append)!)!"
    );
    let fung = common::fung_bin();
    let o = std::process::Command::new("sh")
        .arg("-c")
        .arg("umask 000 && exec \"$0\" -e \"$1\"")
        .arg(&fung)
        .arg(format!("import fs\n{program}"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{o:?}");
    for f in ["w", "a", "ow", "oa"] {
        let mode = std::fs::metadata(dir.join(f)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o666, "{f}: {mode:o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
