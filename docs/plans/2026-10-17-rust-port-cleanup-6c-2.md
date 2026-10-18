# Rust 이식 6c-2 (파일 시스템·OS) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-2 결정표의 여덟 항목을 fung 규칙으로 바꾸고, 손댄 파일의 Go 이름·주석을 걷어낸다.

**Architecture:** `fung-vm/src/native/`의 `oserr.rs`(에러 문구), `fs.rs`(파일 시스템 함수), `glob.rs`, `path.rs`, `file.rs`(핸들 버퍼), `process.rs`(명령 실행 문구)와 앞단의 `diag::quote`(따옴표 형식)를 바꾼다. 동작마다 단위 테스트나 `tests/lang`을 먼저 쓰고, 그 영역의 기대 출력만 bless한다.

**Tech Stack:** Rust 표준 라이브러리, fung 기대 출력 파일.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` ("원칙", "2. 검증", "4. 6c-2 파일 시스템·OS")

## Global Constraints

- 6c-1 계획의 Global Constraints를 그대로 따른다(영어 코드·주석, Conventional Commits, 경로 명시 커밋, `Co-Authored-By`, 새 의존성 없음, 고친 테스트만 돌리고 bless는 영역별로, 사례 수 상수, CHANGELOG `[Unreleased]`, 매뉴얼, 새 코드에 "Go" 없음).
- bless 도우미: `.superpowers/sdd/6c-tools/bless.sh [추가 테스트 이름…]`(cli의 dump·run·tools와 인자로 준 테스트). fung-vm: `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`(prelude와 fs 둘 다).
- 각 Task의 마지막 확인은 `cargo test -q -p fung-vm --lib`(약 12초)까지 돌려 다른 단위 테스트가 옛 동작을 단언하지 않는지 본다(6c-1에서 배운 것).
- 기대 출력은 Linux·macOS에서 같아야 한다. 플랫폼마다 다른 문구가 생기면 안 된다.

## Review Focus

- errno가 표에 없는 에러(예: `ENOSPC`가 아닌 드문 것)의 문구가 `(os error N)` 없이 소문자로 시작하는지. → Task 1 `fallback_text_has_no_suffix`.
- `remove_all("x/.")`, `remove_all("..")`, `remove_all("")`이 각각 거부·거부·성공인지. → Task 2.
- `glob("a*[")`가 일치하는 파일이 하나도 없어도 에러인지(전에는 `a`로 시작하는 이름이 있어야 에러). → Task 3.
- `close` 없이 버린 쓰기 핸들의 마지막 줄이 파일에 남는지, 64 KiB를 넘는 한 줄이 그대로 쓰이는지. → Task 5.
- 따옴표 안의 `$`·`{`·제어 문자·U+00A0이 fung 리터럴로 되돌려 읽을 수 있는 형태인지. → Task 6.

---

### Task 1: OS 에러 문구

**Files:** Modify `crates/fung-vm/src/native/oserr.rs`(표 삭제, `reason`·`errno_text` 다시 쓰기), 테스트는 같은 파일의 `#[cfg(test)]` 또는 `fs_tests.rs`.

**Interfaces:**
- `pub(crate) fn reason(e: &io::Error) -> String`(시그니처 그대로): `e.kind()`로 아래 표를 먼저 찾고, 없으면 `e.to_string()`에서 ` (os error N)`을 떼고 첫 글자를 소문자로.
- 표(`io::ErrorKind` → 문구): NotFound `no such file or directory`, PermissionDenied `permission denied`, AlreadyExists `file exists`, NotADirectory `not a directory`, IsADirectory `is a directory`, DirectoryNotEmpty `directory not empty`, InvalidInput `invalid argument`, ReadOnlyFilesystem `read-only file system`, StorageFull `no space left on device`, FileTooLarge `file too large`, CrossesDevices `invalid cross-device link`, InvalidFilename `file name too long`, ExecutableFileBusy `text file busy`, ArgumentListTooLong `argument list too long`, BrokenPipe `broken pipe`, ConnectionRefused `connection refused`, ConnectionReset `connection reset by peer`, TimedOut `connection timed out`, AddrInUse `address already in use`, AddrNotAvailable `can't assign requested address`, HostUnreachable `no route to host`, NetworkUnreachable `network is unreachable`, Interrupted `interrupted system call`, OutOfMemory `cannot allocate memory`.
- `errno_text(n)`는 `reason(&io::Error::from_raw_os_error(n))`로. `ERRORS` 표들과 `extra`는 지운다. `kind`·`errno`·상수는 그대로.

- [ ] **Step 1: 실패하는 테스트** — `oserr::tests::common_texts`(ENOENT·EACCES·EEXIST·ENOTDIR·ENOTEMPTY·EINVAL의 문구), `fallback_text_has_no_suffix`(표에 없는 errno, 예: `ENOLCK`(Linux 37, macOS 77)의 문구가 `(os error`를 포함하지 않고 소문자로 시작), `no_errno_error_is_lowercase`(`io::Error::other("Bad thing")` → `bad thing`).
- [ ] **Step 2: 실패 확인** — `cargo test -q -p fung-vm --lib oserr`(표에 없는 이름 등으로 실패하거나 Go 문구가 나와 실패).
- [ ] **Step 3: 구현**.
- [ ] **Step 4: 통과 확인과 bless** — 단위 테스트, `cargo test -q -p fung-vm --lib`, fung-vm bless, `bless.sh sigpipe http tls`. diff는 드문 errno 문구에만 있어야 한다(흔한 것은 같은 문장).
- [ ] **Step 5: 커밋** — `refactor(vm): give OS errors fung's own texts`. 06에 "OS 에러 문구" 한 줄.

### Task 2: `fs`의 여러 단계 함수와 `walk` 에러

**Files:** Modify `crates/fung-vm/src/native/fs.rs`(`mkdir_all`, `remove`, `remove_all`, `rename` 주석, `walk`), `fs_tests.rs`.

**Interfaces:**
- `fn mkdir_all(p: &str, perm: u32) -> io::Result<()>`: `DirBuilder::new().recursive(true).mode(perm).create(p)`. 디렉터리가 이미 있으면 성공, 다른 것이 있으면 AlreadyExists.
- `fn remove(p: &str) -> io::Result<()>`: `symlink_metadata` 뒤 디렉터리면 `remove_dir`, 아니면 `remove_file`.
- `fn remove_all(p: &str) -> io::Result<()>`: `""`·없는 경로는 `Ok`, 마지막 성분이 `.`이나 `..`이면 EINVAL, 디렉터리는 `remove_dir_all`, 그 밖은 `remove_file`.
- `fn walk(root: &[u8]) -> Result<Vec<Vec<u8>>, (Vec<u8>, io::Error)>`: 실패한 경로를 함께 돌려주고, `walk_fn`이 그 경로로 `fs_err("walk", …)`.

- [ ] **Step 1: 실패하는 테스트**(`fs_tests.rs`): `mkdir_over_a_file_is_exists`(`kind == "exists"`), `remove_all_refuses_dot_paths`(`x/.`, `..`, `x/..` 거부, `""`·없는 경로 성공), `walk_names_the_directory_it_cannot_read`(권한 0인 하위 디렉터리; root로 돌면 권한이 무시되므로 `geteuid() == 0`이면 건너뛴다).
- [ ] **Step 2: 실패 확인** — `cargo test -q -p fung-vm --lib fs`.
- [ ] **Step 3: 구현**. `rename`의 주석은 "move never replaces an existing directory (only a case-only rename of the same directory goes through)"로.
- [ ] **Step 4: 통과 확인과 bless** — fung-vm bless(fs). diff 검토.
- [ ] **Step 5: 커밋** — `refactor(vm): simplify the fs primitives`. 06 `fs` 절에 `remove_all`의 `.`·`..` 거부, `move`가 디렉터리를 덮지 않는 것, `walk` 에러 경로.

### Task 3: `glob` 패턴 먼저 검사

**Files:** Modify `crates/fung-vm/src/native/glob.rs`, 테스트는 `fs_tests.rs`나 `glob.rs` 단위 테스트.

**Interfaces:** `pub(crate) fn validate(pattern: &[u8]) -> Result<(), BadPattern>`(패턴 전체의 문법만 검사; `[`의 짝, 범위, 끝의 `\`). `glob`이 맨 처음 부른다. `DEPTH_LIMIT`와 그 검사, 단계마다의 `matches(p, b"")?`는 지운다.

- [ ] **Step 1: 실패하는 테스트** — `bad_pattern_fails_without_matching_files`: 빈 임시 디렉터리에서 `glob("<dir>/a*[")`가 `Err(BadPattern)`; `validate`가 `b"a[b-"`, `b"\\"`, `b"[]"`를 거부하고 `b"a\\*b"`, `b"[!a]"`, `b"*.txt"`를 받는다.
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**. 머리 주석은 fung 규칙으로: `*` `?` `[...]` `\`, `!`는 글자, 읽지 못하는 디렉터리는 건너뜀, 결과는 바이트 순서.
- [ ] **Step 4: 통과 확인과 bless**.
- [ ] **Step 5: 커밋** — `fix(vm): reject a malformed glob pattern before matching`.

### Task 4: 점 파일의 확장자

**Files:** Modify `crates/fung-vm/src/native/path.rs`(`ext`·`stem`과 머리 주석), 06 `path` 줄, `docs/builtins.html`의 `path.ext`·`path.stem`·`path.join`.

- [ ] **Step 1: 실패하는 골든** `tests/lang/path_rules.fung`(`import path`):
  ```
  print(path.ext(".hidden"), path.stem(".hidden"), path.ext("a.tar.gz"), path.stem("a.tar.gz"), path.ext(".a.b"))
  # expect:  .hidden .gz a.tar .b
  print(path.join("a", "/c"), path.join("a/./b/", ".."), path.base(""))
  # expect: a/c a .
  ```
  (`print`의 빈 문자열은 빈 칸 하나로 보인다: 첫 줄 맨 앞의 공백.)
- [ ] **Step 2: 실패 확인** — 골든.
- [ ] **Step 3: 구현** — 마지막 성분의 마지막 `.`이 맨 앞(인덱스 0)이면 확장자 없음.
- [ ] **Step 4: 통과 확인과 bless** — 골든과 사례 수 상수, `bless.sh`, fung-vm bless.
- [ ] **Step 5: 커밋** — `fix(vm): give dot files no extension`. 06에 `join` 규칙(뒤의 절대 경로가 앞을 지우지 않는다).

### Task 5: 파일 핸들 버퍼

**Files:** Modify `crates/fung-vm/src/native/file.rs`(`GoWriter` 삭제, `BufWriter`, 머리 주석), `fs_tests.rs`.

**Interfaces:** `const BUF_SIZE: usize = 64 * 1024;`, `Io::Write(BufWriter<std::fs::File>)`, `Io::Read(BufReader<…>)`(크기만). `write_line`은 `write_all`. `close`는 `flush` 뒤 닫기, 첫 에러를 돌려준다. 마지막 사본이 사라질 때는 `BufWriter`의 `Drop`이 비운다(에러는 무시; 주석으로 적는다).

- [ ] **Step 1: 실패하는 테스트**(`fs_tests.rs`의 실행 사례): 핸들을 열고 `write_line` 두 번, `close` 없이 함수에서 빠져나가 핸들이 버려진 뒤 `fs.read`가 두 줄을 돌려준다. 그리고 70,000바이트 한 줄을 쓰고 닫으면 크기가 70,001바이트.
- [ ] **Step 2: 실패 확인** — 첫 사례가 빈 내용으로 실패.
- [ ] **Step 3: 구현**. `GoWriter` 단위 테스트(4095/4096 경계)는 지운다.
- [ ] **Step 4: 통과 확인과 bless**.
- [ ] **Step 5: 커밋** — `fix(vm): flush a dropped file handle and buffer 64 KiB`. CHANGELOG 한 줄(버린 핸들도 쓴 내용을 남긴다), open-questions 4a "닫지 않은 핸들"의 버퍼 줄을 해결로.

### Task 6: fung 리터럴 따옴표

**Files:** Modify `crates/fung-front/src/diag.rs`(`quote`), `crates/fung-front/src/consts.rs`(`quote_str`를 공용으로), 사용처는 그대로 `diag::quote`를 부른다.

**Interfaces:** `pub fn quote(s: &str) -> String`이 `consts::quote_str(s)`와 같은 결과를 낸다: `\n` `\t` `\r` `\\` `\"`, `${` 앞의 `\$`, 보이지 않는 글자는 `\u{X}`(대문자 16진). 테스트 `quote_matches_go`는 `quote_uses_fung_literals`로.

- [ ] **Step 1: 실패하는 테스트** — `diag::tests::quote_uses_fung_literals`: `quote("a\tb\"c ") == "\"a\\tb\\\"c \""`, `quote("\u{0}") == "\"\\u{0}\""`, `quote("\u{A0}") == "\"\\u{A0}\""`, `quote("${x}") == "\"\\${x}\""`, `quote("한") == "\"한\""`.
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현** — `quote`가 `crate::consts::quote_str`를 부른다.
- [ ] **Step 4: 통과 확인과 bless** — `cargo test -q -p fung-front`, fung-vm lib, fung-vm bless, `bless.sh json_expected time_expected regex_expected http tls`. diff는 `\x..`·`\u00..`·`\a`·`\b`·`\f`·`\v`가 `\u{..}`로 바뀐 것뿐.
- [ ] **Step 5: 커밋** — `feat: quote strings in messages with fung literals`. CHANGELOG 한 줄.

### Task 7: 명령 실행 문구

**Files:** Modify `crates/fung-vm/src/native/process.rs`(문구, 머리 주석, `prepare`·`look_path` 주석), 테스트는 기존 process 테스트 모듈.

**Interfaces:** 문구: `command not found in PATH`, `command found only in a relative PATH entry`, `empty command name`, `argument contains a NUL byte`, `environment variable contains a NUL byte`, `working directory contains a NUL byte`, `not an executable file (run scripts with sh)`. 검사 순서: 명령 이름(빈 이름, NUL) → `$PATH` 찾기 → 인자 NUL → `env` NUL → `cwd`(NUL, 없음) → 실행 형식.

- [ ] **Step 1: 실패하는 테스트** — `process` 단위 테스트에 각 문구 한 사례씩(`exec([""])`, `exec(["no-such-cmd-xyz"])`, `exec(["ls", "a\u{0}"])`, `exec(["ls"], env: {"A": "\u{0}"})`, 실행 비트만 있는 텍스트 파일).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**.
- [ ] **Step 4: 통과 확인과 bless**.
- [ ] **Step 5: 커밋** — `refactor(vm): word command errors in fung's terms`. 06 명령 실행 절과 매뉴얼에 문구가 있으면 함께.

### Task 8: 6c-2 정리

**Files:** Task 1~7에서 손댄 소스 파일의 남은 Go 주석(테스트 파일의 출처 표시는 6c-7), `docs/design/open-questions.md`의 6c-2 항목(운영체제 에러, `filepath`·순서, `glob`, `GoWriter`·핸들 표시, 실행 형식 문구) 해결 표시.

- [ ] **Step 1:** `grep -nw Go`로 손댄 소스 파일을 보고 주석을 다시 쓴다.
- [ ] **Step 2:** `cargo test -q -p fung-front && cargo test -q -p fung-vm --lib`.
- [ ] **Step 3: 커밋** — `docs: close the stage 6c-2 items`.
