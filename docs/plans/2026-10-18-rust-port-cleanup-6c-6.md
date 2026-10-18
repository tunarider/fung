# Rust 이식 6c-6 (도구) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-6 결정표(T1~T4, P1~P3, C1·C2, S1, F1, R1~R6, W1)대로 `fung test`·`check`·`fmt`·REPL의 Go 흉내를 fung 규칙으로 바꾼다.

**Architecture:** 명령은 `crates/fung-cli/src/`(`test.rs`·`paths.rs`·`check.rs`·`fmt.rs`·`run.rs`·`main.rs`·`repl/`), 포매터는 `crates/fung-front/src/format/`. 표시 너비 표는 6c-1의 유니코드 생성기(`crates/fung-front/src/unicode/generate.rs`, `FUNG_UCD`)에 `EastAsianWidth.txt`를 더해 만든다.

**Tech Stack:** Rust 표준 라이브러리, `libc`(fung-vm `term.rs`), 유니코드 문자 데이터베이스 17.0.0.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` (4절 "6c-6 도구")

## Global Constraints

- 앞 단계들의 Global Constraints를 따른다. bless: `.superpowers/sdd/6c-tools/bless.sh`(dump·run·tools) + 해당 테스트 이름. 각 Task 끝에 `cargo test -q -p fung-vm --lib`와 `cargo test -q -p fung-cli`의 관련 테스트를 돌린다.
- 파일 에러 문구는 `fs` 모듈과 같은 꼴 ``cannot read `p`: <이유>``(이유는 6c-2의 `oserr::reason`).
- `EastAsianWidth.txt`는 받기 전에 사용자 승인을 받는다(Task 5).

## Review Focus

- `fung test`에서 읽지 못한 파일·종료 핸들러 에러가 요약의 실패 수에 들고, 종료 코드가 2(읽기)·1(핸들러)이다. → Task 1.
- `fmt`와 `test`가 같은 트리에서 같은 파일 목록을 고른다(`.`·`_` 디렉터리, 링크 규칙). → Task 2.
- REPL: 앞 입력에서 정의한 함수의 런타임 에러가 정의한 줄을 발췌한다. 전역 한도를 넘긴 입력 뒤에도 다음 입력이 돈다. → Task 3.
- 한글 주석·문자열이 든 줄의 끝 주석이 화면에서 한 열로 맞는다. → Task 5.

---

### Task 1: `fung test` (T1~T4)

**Files:** `crates/fung-cli/src/test.rs`, `crates/fung-cli/tests/expected/test/`(기대 출력), 골든 사례(`tests/lang` 또는 `crates/fung-cli/tests/data`).

- [ ] **Step 1: 실패하는 테스트:** 읽지 못하는 파일(권한 0 파일)이 `FAIL p: <load>`와 `0 passed, 1 failed`, 종료 코드 2. `at_exit(() => fail("x"))`가 든 테스트 파일이 `FAIL p: <exit handler>`와 실패 하나, 종료 코드 1.
- [ ] **Step 2~4:** 실패 확인, 구현(T1은 주석만), bless.
- [ ] **Step 5: 커밋** `feat(test): count unreadable files and failing exit handlers`.

### Task 2: 경로와 파일 에러 (P1~P3, C1, C2, S1)

**Files:** `crates/fung-cli/src/{paths,check,fmt,run,main}.rs`, `tools_expected.rs`의 사례.

- [ ] **Step 1: 실패하는 테스트:** `test`가 `.hidden/` 아래 파일을 건너뛴다. 인자로 준 디렉터리 링크를 `test`가 따라간다. 트리 안의 링크는 둘 다 따라가지 않는다. `fung run nope`·`check nope`·`test nope`·`fmt nope`가 ``error: cannot read `nope`: no such file or directory``. `check dir`이 ``cannot read `dir`: is a directory``. `check -e 1 x`가 사용 에러(종료 코드 2). 표준 입력 읽기 에러가 `<stdin>`.
- [ ] **Step 2~4:** 실패 확인, 구현, bless(`tools_expected`).
- [ ] **Step 5: 커밋** `refactor(cli): one walk rule and fung's file error texts`.

### Task 3: REPL 발췌와 전역 한도 (R1, R2)

**Files:** `crates/fung-cli/src/repl/mod.rs`, `crates/fung-vm/src/vm/runtime.rs`(`add_names` → 글까지 넘기는 `add_sources`, `MAX_REPL_GLOBALS` 공개).

- [ ] **Step 1: 실패하는 테스트**(REPL 기대 출력 또는 `repl` 테스트): 1번 입력 `fn f() = 1 / 0`, 2번 입력 `\n\nf()`의 에러 발췌가 1번 입력의 줄이다. 띄운 태스크 실패 보고에 발췌가 있다. 전역 65537개를 만드는 입력이 E0806이고, 다음 입력 `1 + 1`이 `2`를 찍는다.
- [ ] **Step 2~4:** 실패 확인, 구현(`excerpts` 삭제, 입력의 한도 검사를 전역 갱신 앞에서), bless.
- [ ] **Step 5: 커밋** `fix(repl): excerpt the input that defined the code; a refused input leaves no globals`.

### Task 4: REPL 입력 (R3, R4, R5)

**Files:** `crates/fung-vm/src/term.rs`(`is_terminal`: `tcgetattr`가 되는지), `crates/fung-cli/src/repl/{line,history}.rs`.

- [ ] **Step 1: 실패하는 테스트:** `/dev/null`을 표준 입력으로 준 REPL이 기록 파일을 만들지 않는다(`HOME`을 임시 디렉터리로). `plain_line`이 `"a\r\r\n"`을 `"a\r"`로 읽는다(`PlainReader`와 같음). 잘못된 바이트가 든 1,050줄 기록 파일을 줄인 뒤 남은 줄의 바이트가 그대로다.
- [ ] **Step 2~4:** 실패 확인, 구현(`History`가 원래 바이트도 쥔다), 테스트 통과.
- [ ] **Step 5: 커밋** `fix(repl): terminals only, one line end rule, history bytes kept`.

### Task 5: 포매터의 표시 너비 (F1)

**Files:** `crates/fung-front/src/unicode/{generate,mod}.rs`(너비 표), `spec/unicode_width.txt`(생성), `crates/fung-front/src/format.rs`·`format/align.rs`(`go_rune_width`·`rune_count` → `display_width`), 포매터 테스트·`fmt` 기대 출력.

- [ ] **Step 0:** `EastAsianWidth.txt`(유니코드 17.0.0) 다운로드 승인을 받고 `FUNG_UCD` 디렉터리에 받는다.
- [ ] **Step 1: 실패하는 테스트:** `display_width("한a")` = 3, 결합 문자 `e\u{301}` = 1, `\u{200B}` = 0, 잘못된 부분 `\xE2\x82` = 1. 포매터: `x = "한글" # c` / `yy = 1 # c` 가 끝 주석을 같은 화면 열에 둔다.
- [ ] **Step 2~4:** 실패 확인, 생성기와 표(`FUNG_UPDATE_SPEC=1 FUNG_UCD=<dir> cargo test -p fung-front unicode_spec_file`), 구현, bless(`fmt` 기대 출력).
- [ ] **Step 5: 커밋** `feat(fmt): align by display width`.

### Task 6: 문서와 정리 (W1)

- [ ] **Step 1:** 08 `fung test`(T2·T3, 경로 규칙)·`check`·`fmt`(표시 너비)·REPL(발췌, 터미널 판정, 줄 끝, 기록 파일)·W0102 순서. 매뉴얼 `internals.html`·`index.html`의 해당 절, CHANGELOG, open-questions "5단계" 항목의 Go 흉내 목록을 해결로.
- [ ] **Step 2:** 바꾼 파일(`crates/fung-cli/src/`, `format`)의 Go 주석과 이름.
- [ ] **Step 3:** 관련 테스트, 커밋 `docs: close the stage 6c-6 items`.
