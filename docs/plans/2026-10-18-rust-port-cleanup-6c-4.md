# Rust 이식 6c-4 (정규식) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-4 결정표(R1~R6)대로 fung 정규식을 `regex` 크레이트의 문법과 규칙 위에 다시 세우고, 옮겨 온 Go 파서와 그 데이터를 지운다.

**Architecture:** `native/regex.rs`가 `regex::RegexBuilder`로 직접 컴파일한다(유니코드 기능 켬). 에러는 `regex_syntax::Parser`로 먼저 파싱해 에러 종류(`kind()`)를 한 줄 문구로 쓴다. `find_all`·`replace`는 크레이트의 반복과 `Captures::expand`를, `split`은 크레이트 매치 위에 R5 규칙을 쓴다. `regex_go.rs`·`regex_go_fixtures.rs`·`regex_go_tests.rs`·`spec/regex_unicode.txt`를 지운다.

**Tech Stack:** `regex` 1.13.1, `regex-syntax` 0.8.11(둘 다 이미 허용된 크레이트; `unicode` 기능을 더 켠다).

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` (4절 "6c-4 정규식")

## Global Constraints

- 6c-1~6c-3의 Global Constraints를 따른다. bless: `.superpowers/sdd/6c-tools/bless.sh regex_expected`, fung-vm은 `runs_match_the_expected_output`.
- 새 크레이트 없음. `Cargo.toml`의 기능만 바꾸고 `Cargo.lock`은 그대로(`--locked`). `deny.toml` 주석을 맞춘다.
- 실행 파일 크기가 늘어난다(크레이트 유니코드 표). `scripts/check-release.sh`에 크기 상한이 있으면 확인한다.

## Review Focus

- 한글·악센트·아랍 숫자가 든 주어에서 `\w`·`\d`·`\b`의 위치가 글자 인덱스로 맞는다. → Task 1 단위 테스트.
- 이름이 같은 그룹, 숫자로 시작하는 그룹 이름은 E0515. → Task 1.
- `split`의 끝 규칙: `"a,b,"`는 끝의 빈 조각을 남기고(`[a, b, ""]`), `"abc"`를 `""`로 자르면 `[a, b, c]`, `""`를 `x*`로 자르면 `[""]`. → Task 2.
- 아주 큰 패턴(`\pL{1000}{1000}`)은 E0515 ``pattern too large``이고 프로세스가 죽지 않는다. → Task 1.
- 캐시: 같은 패턴을 두 태스크가 동시에 컴파일해도 결과가 하나로 남는다(지금 테스트 유지). → Task 1.

---

### Task 1: 크레이트로 컴파일하기 (R1, R2, R3, R6)

**Files:** Modify `crates/fung-vm/Cargo.toml`(기능 `unicode`), `deny.toml`(주석), `crates/fung-vm/src/native/regex.rs`(`compile`, `Compiled`, 머리 주석), `crates/fung-front/src/diag/catalog.rs`(E0515 틀), `spec/errors.txt`(`FUNG_UPDATE_SPEC=1 cargo test spec_file`), `crates/fung-vm/src/native.rs`(모듈 삭제); Delete `native/regex_go.rs`, `regex_go_fixtures.rs`, `regex_go_tests.rs`, `spec/regex_unicode.txt`; Test `native/regex_tests.rs`.

**Interfaces:**
- `fn compile(pat: String) -> Result<Compiled, String>`: 먼저 `regex_syntax::Parser::new().parse(&pat)`; 에러면 `Err(format!("`{pat}`: {}", kind))`(`regex_syntax::Error::Parse(e) => e.kind()`, `Translate(e) => e.kind()`의 `Display`). 그다음 `RegexBuilder::new(&pat).build()`; `regex::Error::CompiledTooBig`이면 ``"`{pat}`: pattern too large"``, 그 밖의 에러는 ``"`{pat}`: invalid pattern"``.
- `Compiled { re: Regex, names: Vec<String> }`: `names[i]`는 `re.capture_names()`의 i번째(없으면 `""`).
- E0515 틀: `"invalid pattern %s"`(인자 하나: `` `pat`: reason ``). 모든 E0515 호출이 인자를 이 꼴로 준다.

- [ ] **Step 1: 실패하는 테스트**(`regex_tests.rs`): `unicode_classes`(`\d`가 `٣`, `\w`가 `é`·`가`에 맞고 `(?-u:\d)`는 `٣`에 안 맞음, `find("가a1", "\\w+")`의 `end`가 3), `crate_syntax`(`(?x) a b`가 `"ab"`에, `[a&&[a-c]]`가 `"a"`에 맞음), `pattern_errors`(`a(` → ``invalid pattern `a(`: unclosed group``, `(?P<n>a)(?P<n>b)` → `duplicate capture group name` 포함, `\pL{1000}{1000}` → `pattern too large` 포함).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현** — 기능을 켜고, `compile`을 다시 쓰고, Go 파일과 표를 지운다. 머리 주석은 "fung's regular expressions are the `regex` crate's syntax (06)".
- [ ] **Step 4: 통과 확인과 bless** — vm lib, fung-front(`spec_file`), `bless.sh regex_expected`, fung-vm bless. 골든 `tests/lang/regex*.fung`·`err_regex*`의 기대를 새 문구로.
- [ ] **Step 5: 커밋** — `feat(vm): use the regex crate's syntax for fung patterns`.

### Task 2: 반복·치환·나누기 (R4, R5)

**Files:** Modify `regex.rs`(`go_all`·`expand`·`extract` 삭제, `find_all_fn`·`replace_fn`·`split_fn`), `regex_tests.rs`.

**Interfaces:**
- `find_all`: `re.captures_iter(s)`; `match_value(s, names, &Captures, &mut RuneCursor)`.
- `replace`: `re.replace_all(s, repl)`(크레이트의 `$` 규칙).
- `split`: `re.find_iter(s)`의 매치 중 **빈 매치이면서 시작이 0이거나 `s.len()`인 것**을 건너뛰고, 나머지 매치마다 자른다.

- [ ] **Step 1: 실패하는 테스트**: `split_edges`(위 Review Focus의 세 사례와 `"a,,b"` → `["a","","b"]`), `replace_rules`(`$1a`는 빈 문자열, `${1}a`, `$$`, 없는 이름은 빈 문자열), `find_all_empty`(`"baaa"`에 `a*` → `(0,0)`·`(1,4)`).
- [ ] **Step 2~4**: 실패 확인, 구현, 통과·bless.
- [ ] **Step 5: 커밋** — `refactor(vm): iterate, replace and split with the regex crate`.

### Task 3: 문서와 정리

- [ ] **Step 1:** 06 "정규식" 절(문법은 `regex` 크레이트, 유니코드 클래스, `$` 규칙과 `${1}a` 안내, `split` 끝 규칙), 매뉴얼 `regex` 절, CHANGELOG, `CLAUDE.md`의 `spec/regex_unicode.txt` 언급, open-questions("정규식 문법" 결정 갱신, 4c 정규식 잔가지 세 항목 해결).
- [ ] **Step 2:** `extras/regex.fung` 머리 주석, `regex.rs`·`regex_tests.rs`의 Go 주석.
- [ ] **Step 3:** `cargo test -q -p fung-vm --lib`, `cargo test -q -p fung-cli --test manual`, 커밋 `docs: close the stage 6c-4 items`.
