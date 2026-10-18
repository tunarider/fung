# Rust 이식 6c-1 (기반) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-1 결정표의 아홉 항목(+ 정렬, 아래 Task 8)을 fung 규칙으로 바꾸고, 손댄 파일의 Go 이름·주석을 걷어낸다.

**Architecture:** 앞단(`fung-front`)의 유니코드 표를 Rust 생성기로 다시 만들고 식별자 판정을 UAX #31로 바꾼다. VM(`fung-vm`)의 순회 도우미(`native/nat.rs`)를 스트리밍으로 바꾸고, 한도 에러를 E0806 하나로 모은다. 동작마다 설계 문서와 `tests/lang`을 먼저 고친 뒤 구현하고, 그 영역의 기대 출력만 bless한다.

**Tech Stack:** Rust 표준 라이브러리만(앞단은 크레이트 0개), fung 골든 테스트, 기대 출력 파일.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` (특히 "원칙", "2. 검증", "4. 6c-1 기반")

## Global Constraints

- 코드·주석·에러 메시지·커밋 메시지는 영어, 설계 문서는 한국어. 커밋은 Conventional Commits, 경로를 명시해 커밋(`git commit -am` 금지).
- 커밋 작성자 `tunarider <tunarider@tunaground.net>`. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- `fung-front`는 크레이트 0개. 새 의존성 없음.
- 단계 중 검사: TDD로 고친 테스트만, bless는 그 영역의 테스트만. 전체 커밋 전 검사는 6c 맨 끝에 한 번(스펙 2절). 중간 커밋이 다른 테스트를 깨도 된다. 다만 고친 테스트는 통과해야 한다.
- bless 명령: `FUNG_EXPECTED=bless cargo test -p fung-cli --test <이름>`, fung-vm은 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`. bless 뒤 `git diff --stat`과 diff를 읽고, 의도한 변화만 있는지 커밋 메시지 본문에 한 줄로 적는다.
- 사례를 더하거나 빼면 사례 수 상수(`RUN_CASE_COUNT` 등)를 함께 바꾼다. 새 `tests/lang/*.fung`은 `fung fmt --check`를 통과해야 하고, dump·run·fmt 기대 출력과 `DUMP_CASES`·`FMT_INPUTS` 등이 함께 바뀐다(CLAUDE.md "구조와 테스트").
- 사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`에 한 줄씩. 내장 함수 동작을 바꾸면 `docs/builtins.html`도.
- 새로 쓰거나 고친 코드·주석에 "Go"를 쓰지 않는다. 손댄 함수의 Go 주석은 같은 커밋에서 걷어낸다.
- 한도 숫자: 원소 100,000,000(`MAX_ELEMENTS`), 글자 100,000,000(`MAX_CHARS`, 있음), 표준 입력 버퍼 65,536바이트.

## Review Focus

- 채널 `each`/`for` 안에서 함수가 실패할 때: 그 자리에서 에러가 나고, 받지 않은 값은 채널에 남으며, 보내는 태스크는 막힌 채 남는다(교착 보고 규칙은 그대로). → Task 7 테스트 `each_failure_leaves_rest`.
- 같은 채널을 순회하는 함수가 그 채널에 다시 보낼 때(`each(ch, x => send(ch, x))`, 버퍼 있음): 스트리밍은 끝없이 돌 수 있다. 교착이나 무한 루프가 아니라 사용자의 프로그램이 정한 동작이다. 문서에 한 줄 적고, 버퍼가 꽉 차면 막히는지 테스트로 확인한다. → Task 7 `resend_into_iterated_channel`.
- 식별자의 결합 문자: `नमस्ते`(Mn U+094D 포함)와 `é`(분해형 `e\u{301}`)가 이름이 되고, 태그 판정은 첫 글자만 본다. `_`만 있는 이름, 숫자로 시작하는 이름은 여전히 거부. → Task 1 골든.
- `upper`가 글자 수를 바꾸는 경우(`ß` → `SS`, `ŉ` → `ʼN`)와 끝 시그마(`lower("ΟΔΟΣ")` → `"οδος"`). → Task 2.
- `float("1_000.5")`는 받고, `float("1__0")`·`float("_1")`·`float("1e")`·`float("+-1")`·`float(" 1")`은 거부. → Task 6.

---

### Task 1: 유니코드 생성기와 UAX #31 식별자

**Files:**
- Create: `crates/fung-front/src/unicode/generate.rs`(테스트 전용 생성기, `#[cfg(test)]`)
- Modify: `crates/fung-front/src/unicode.rs` → `crates/fung-front/src/unicode/mod.rs`(디렉터리로)
- Modify: `spec/unicode.txt`(다시 생성)
- Modify: `crates/fung-front/src/lexer.rs:297-301`, `crates/fung-front/src/loader.rs:410-413`, `crates/fung-vm/src/value/format.rs:75`, `crates/fung-vm/src/prelude/convert.rs:178-187`
- Modify: `docs/design/01-syntax.md`(식별자 절), `docs/design/open-questions.md`(유니코드 항목: 해결 표시와 다시 만드는 법 교체), `CHANGELOG.md`
- Test: `tests/lang/ident_unicode.fung`(새 골든)

**Interfaces:**
- Produces (`fung_front::unicode`):
  - `pub fn is_name_start(c: char) -> bool` — `XID_Start`(`_`는 호출하는 쪽이 따로 본다)
  - `pub fn is_name_continue(c: char) -> bool` — `XID_Continue`(`_` 포함, 표 그대로)
  - `pub fn is_upper(c: char) -> bool` — 일반 범주 Lu
  - `pub fn is_print(c: char) -> bool` — 일반 범주 L·M·N·P·S 또는 U+0020
  - 지우는 것: `is_letter`, `is_digit`, 표의 `space` 구간(공백은 `char::is_whitespace`, 즉 유니코드 `White_Space`를 그대로 쓴다)
- `spec/unicode.txt` 형식은 지금과 같은 `<class> <first> <last>` 줄이고, class는 `name_start`·`name_continue`·`upper`·`print`. 머리 주석:
  `# Unicode character classes for fung (UCD 17.0.0). Generated: FUNG_UPDATE_SPEC=1 FUNG_UCD=<ucd dir> cargo test -p fung-front unicode_spec_file; do not edit.`

- [ ] **Step 1: 유니코드 데이터 받기 (사용자 승인 필요)**
  `https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt`와 `DerivedCoreProperties.txt`를 스크래치 디렉터리에 받는다. 파일 이름·출처·크기를 말하고 사용자 승인을 받은 뒤 받는다. 저장소에는 넣지 않는다.
- [ ] **Step 2: 실패하는 테스트 쓰기**
  - `unicode::tests::name_classes`: `is_name_start('a'|'é'|'파'|'न')`이 참, `is_name_continue('\u{94D}'|'\u{301}'|'5'|'_')`이 참, `is_name_start('5'|'_'|'\u{301}')`이 거짓.
  - `unicode::tests::print_classes`: 지금 `is_print_cases`의 값을 그대로 옮긴다(L·M·N·P·S + U+0020이라 같은 결과).
  - `generate::unicode_spec_file`: `FUNG_UCD`가 없으면 아무것도 하지 않고 통과한다. 있으면 두 파일에서 표를 만들어 `spec/unicode.txt`와 비교하고, `FUNG_UPDATE_SPEC=1`이면 파일을 다시 쓴다. `spec_file`의 기존 관례(`crates/fung-vm/src/boot/spec_file.rs`)를 따른다.
  - 골든 `tests/lang/ident_unicode.fung`:
    ```
    नमस्ते = 1
    café = 2
    print(नमस्ते + café)
    # expect: 3
    ```
    와 `type_of(Été)`가 태그로 판정되는 줄(`# expect:`는 실제 태그 출력 형식대로).
- [ ] **Step 3: 실패 확인** — `cargo test -p fung-front unicode`가 없는 함수로 컴파일 실패, `cargo test -p fung-cli --test golden`이 `ident_unicode`에서 실패.
- [ ] **Step 4: 생성기 구현** — `UnicodeData.txt`의 범위 줄(`<..., First>`/`<..., Last>`)을 펼쳐서 일반 범주를 읽고, `DerivedCoreProperties.txt`의 `XID_Start`·`XID_Continue`를 읽어 구간으로 합친다. `FUNG_UCD=<dir> FUNG_UPDATE_SPEC=1 cargo test -p fung-front unicode_spec_file`로 `spec/unicode.txt`를 다시 쓴다.
- [ ] **Step 5: 판정 함수 구현과 호출하는 곳 바꾸기** — 위 네 곳에서 `is_letter`는 `is_name_start`, (`is_letter || is_digit`)는 `is_name_continue`로 바꾼다. `convert.rs`의 `is_name`과 `loader.rs`의 모듈 이름 검사에는 "첫 글자가 Lu가 아님"이 그대로 남는다. 모듈 머리 주석은 "fung's character classes (UAX #31 identifiers, Lu tags, printable)"로.
- [ ] **Step 6: 01-syntax 갱신** — "이어지는 글자는 글자·숫자·`_`"를 "이름은 UAX #31(`XID_Start` 또는 `_`로 시작해 `XID_Continue`)을 따르고, 결합 문자가 이어질 수 있다"로 바꾼다. 유니코드 버전 17.0.0과 표를 다시 만드는 명령을 적는다.
- [ ] **Step 7: 통과 확인** — `cargo test -p fung-front unicode`, `cargo test -p fung-cli --test golden`. 골든 수를 세는 상수가 있으면 함께 바꾼다.
- [ ] **Step 8: bless** — dump·fmt 기대 출력에 새 골든이 들어가므로 `FUNG_EXPECTED=bless cargo test -p fung-cli --test dump_expected`, `--test run_expected`, `--test fmt_repo`(있으면). diff는 새 파일 사례만이어야 한다.
- [ ] **Step 9: 커밋** — `feat(front): follow UAX #31 for names and generate the Unicode tables in Rust`. open-questions 유니코드 항목에 "→ 해결 (6c-1)"과 새 명령을 적는다. CHANGELOG: "Names may contain combining marks (UAX #31 identifiers)."

### Task 2: 전체 대소문자 매핑

**Files:**
- Modify: `crates/fung-vm/src/prelude/strings.rs:95-135`
- Modify: `docs/design/06-native-and-stdlib.md`(문자열 함수 절), `docs/builtins.html`(`upper`·`lower`), `CHANGELOG.md`
- Test: `tests/lang/strings_case.fung`(새 골든)

**Interfaces:** `upper_rune`·`lower_rune`을 지우고 `upper_fn`·`lower_fn`이 `str::to_uppercase`·`to_lowercase`를 부른다.

- [ ] **Step 1: 실패하는 골든 쓰기**
  ```
  print(upper("straße"), upper("ŉ"), lower("ΟΔΟΣ"), lower("İ"), len(upper("ß")))
  # expect: STRASSE ʼN οδος i̇ 2
  ```
- [ ] **Step 2: 실패 확인** — `cargo test -p fung-cli --test golden`이 `strings_case`에서 실패.
- [ ] **Step 3: 구현** — 두 함수를 `string(s.to_uppercase())`, `string(s.to_lowercase())`로.
- [ ] **Step 4: 통과 확인과 bless** — 골든 통과. 이어서 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`과 cli의 run·dump·fmt 기대 출력을 bless하고, diff가 대소문자 사례뿐인지 본다.
- [ ] **Step 5: 문서와 커밋** — 06과 매뉴얼에 "유니코드 기본 대소문자 변환(전체 매핑): 글자 수가 바뀔 수 있다"를 적는다. 커밋 `feat(prelude): use full Unicode case mapping in upper and lower`.

### Task 3: 한도를 E0806 하나로

**Files:**
- Modify: `crates/fung-vm/src/native/nat.rs:120-131`(`MAX_SLICE`·`too_long` → `MAX_ELEMENTS`·`too_many_elements`)
- Modify: `crates/fung-vm/src/prelude.rs:118-130`(`map`), `crates/fung-vm/src/prelude/collections.rs`(`sort`·`sort_by`), `crates/fung-vm/src/prelude/strings.rs:155-176`(`repeat`·`join`), `crates/fung-vm/src/conc.rs:220-240`(`channel`)
- Modify: `crates/fung-vm/src/prelude/tests.rs`의 `huge_range_cases`, `crates/fung-vm/src/prelude/run_cases.rs`
- Modify: `docs/design/02-values-and-types.md` 또는 06(한도 절이 있는 곳), `docs/design/05-concurrency.md`(`channel(n)`), `CHANGELOG.md`

**Interfaces:**
- Produces (`nat`): `pub const MAX_ELEMENTS: u64 = 100_000_000;`, `pub fn too_many_elements() -> Stop`(`E0806`, 인자 `"elements"`, `MAX_ELEMENTS`).
- `strings::MAX_CHARS`는 그대로 쓴다. `pub(crate) fn too_many_chars() -> Stop`(`E0806`, `"characters"`, `MAX_CHARS`)를 더한다.

- [ ] **Step 1: 기대 사례 정하기** — `huge_range_cases`와 `run_cases.rs`에 다음을 둔다(있는 것은 그대로 두고 기대만 바뀜).
  - `print(sort(0..100000001))`, `print(map(0..100000001, x => x))`는 `E0806 too many elements (limit 100000000)`.
  - `print(sort(0..100000000) |> len)`는 이 테스트에서 너무 느리니 넣지 않는다. 경계는 단위 테스트 `nat::tests::element_limit_boundary`로 확인한다. `MAX_ELEMENTS`짜리 범위는 통과하고 하나 더 많으면 거부하는지, 리스트를 만들지 않고 길이만으로 판정한다.
  - `print(join(0..100000000000000000, 1))`는 `sep` 인자 에러(E0403)이고, 크기 에러가 아니다.
  - `print(sort_by(0..100000000000000000, 1))`는 `f` 인자 에러.
  - `print(len(repeat("ab", 9223372036854775807)))`와 `repeat("a", 100000001)`은 `E0806 too many characters (limit 100000000)`.
  - `print(join(0..100000000000000000, ""))`는 글자 한도 E0806(조인 중 `out`이 `MAX_CHARS` 글자를 넘는 순간).
  - `ch = channel(17592186044410); print(send(ch, 1), recv(ch))`는 `Ok(nil) Ok(1)`(형식은 실제 `send` 결과대로).
- [ ] **Step 2: 실패 확인** — `cargo test -p fung-vm runs_match_the_expected_output`이 위 사례에서 실패(기대 파일을 아직 bless하지 않았으므로 실패하는 것이 맞다). 대신 `nat::tests::element_limit_boundary`로 실패를 본다.
- [ ] **Step 3: 구현**
  - `map`: `need_fn` 검사 → 순회할 수 있는지 검사 → 범위면 `r.len() > MAX_ELEMENTS`일 때 `too_many_elements()`. 미리 잡는 용량은 지금처럼 `min(1 << 16)`.
  - `items`(정렬용): 범위 길이 검사를 `MAX_ELEMENTS`로.
  - `sort_by`: `f` 검사를 `items`보다 먼저.
  - `join`: `sep` 검사를 먼저. 쌓는 동안 글자 수가 `MAX_CHARS`를 넘으면 `too_many_chars()`.
  - `repeat`: `checked_mul`로 글자 수를 계산하고, 넘치거나 `MAX_CHARS`를 넘으면 `too_many_chars()`. `strings: Repeat` 분기를 지운다.
  - `channel`: `MAX_CHANNEL_BUFFER`와 그 분기를 지운다.
- [ ] **Step 4: 통과 확인과 bless** — 단위 테스트 통과. 이어서 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`, `FUNG_EXPECTED=bless cargo test -p fung-cli --test json_expected`(큰 들여쓰기 사례는 6c-3 몫이니 diff에 나오면 안 된다). diff에 `makeslice`·`makechan`·`Repeat`이 사라지고 E0806이 생겼는지 본다.
- [ ] **Step 5: 문서와 커밋** — 설계 문서에 한도 표(원소 1억, 글자 1억)와 "범위를 펼치는 함수만, 인자 검사가 먼저"를 적는다. 05의 `channel(n)`에서 상한 서술을 지운다. 커밋 `feat(prelude): report element and character limits as E0806`.

### Task 4: 표준 입력 64 KiB

**Files:**
- Modify: `crates/fung-vm/src/vm/stdin.rs:1-41`(`BUFFER = 65536`, 머리 주석), `crates/fung-cli/src/run.rs:148`(주석)
- Modify: `crates/fung-cli/tests/stdin.rs`, `docs/design/06-native-and-stdlib.md`(표준 입력 절)

- [ ] **Step 1: 실패하는 테스트로 바꾸기** — `one_read_line_takes_one_buffer_of_a_file`의 입력을 64 KiB보다 크게(약 20,000줄) 하고, 기대를 `&content[65536..]`로. 머리 주석은 "a buffer of 64 KiB at a time (the pipe buffer size on Linux)".
- [ ] **Step 2: 실패 확인** — `cargo test -p fung-cli --test stdin`이 오프셋 4096으로 실패.
- [ ] **Step 3: 구현** — `BUFFER: usize = 64 * 1024`. 주석에서 Go를 걷어낸다.
- [ ] **Step 4: 통과 확인** — `cargo test -p fung-cli --test stdin`.
- [ ] **Step 5: 문서와 커밋** — 06에 "필요할 때만, 한 번에 최대 64 KiB를 읽는다. 같은 입력을 이어 읽는 다른 프로세스는 그 뒤부터 받는다"를 적는다. 커밋 `perf(vm): read standard input 64 KiB at a time`.

### Task 5: 실행 형식 검사 주석

**Files:**
- Modify: `crates/fung-vm/src/native/process.rs:236-245`, `:278-300`의 주석만

- [ ] **Step 1:** `kernel_can_run`과 그것을 부르는 곳의 주석을 fung 규칙으로 다시 쓴다. 내용: "A file the kernel cannot execute is refused rather than handed to `/bin/sh` (which `execvp` would do); shell scripts go through `sh()`." 문구 `exec format error`는 6c-2에서 정하므로 그대로 둔다.
- [ ] **Step 2: 커밋** — `docs(vm): explain why exec refuses files the kernel cannot run`.

### Task 6: 실수 변환은 리터럴 문법

**Files:**
- Modify: `crates/fung-vm/src/prelude/convert.rs:30-150`(`parse_float`, `parse_hex_float` 삭제, `parse_int` 주석)
- Modify: `docs/design/06-native-and-stdlib.md`(변환 함수 절), `docs/builtins.html`(`float`·`parse_float`), `CHANGELOG.md`
- Test: `convert` 단위 테스트, `run_cases.rs`의 해당 사례

**Interfaces:** `fn parse_float(s: &str) -> Option<f64>`(시그니처 그대로). 받는 문법: `[+-]? D ('.' D)? ([eE] [+-]? D)?`, `D = 숫자 ('_'? 숫자)*`. 결과가 유한해야 한다.

- [ ] **Step 1: 실패하는 테스트** — `convert::tests::float_literal_syntax`:
  - 받음: `"3.14"`→3.14, `"-2.5e-3"`, `"+1e9"`, `"42"`→42.0, `"1_000.5"`→1000.5.
  - 거부(`None`): `"0x1p4"`, `".5"`, `"1."`, `"1e"`, `"1__0"`, `"_1"`, `"1_"`, `"inf"`, `"nan"`, `"+-1"`, `" 1"`, `"1e400"`(무한).
- [ ] **Step 2: 실패 확인** — `cargo test -p fung-vm float_literal_syntax`가 `0x1p4`·`.5`·`1.`에서 실패.
- [ ] **Step 3: 구현** — 위 문법을 손으로 검사한 뒤 `_`를 지우고 `str::parse::<f64>`에 넘긴다. `parse_hex_float`을 지운다.
- [ ] **Step 4: 통과 확인과 bless** — 단위 테스트, `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`(16진 실수 사례가 있으면 기대가 거부로 바뀐다).
- [ ] **Step 5: 문서와 커밋** — 06과 매뉴얼에 "`float`·`parse_float`는 실수 리터럴 문법(앞의 부호, 정수 꼴 허용)만 받는다"를 적는다. 커밋 `feat(prelude): parse floats with the literal syntax`.

### Task 7: 채널 순회를 스트리밍으로

**Files:**
- Modify: `crates/fung-vm/src/native/nat.rs:133-230`(`check_items`, `each_until`, `items`)
- Modify: `crates/fung-vm/src/prelude/collections.rs`(`need_fn_after`와 `check_items`를 부르는 함수들), `crates/fung-vm/src/prelude.rs`(`map`), `crates/fung-vm/src/prelude/strings.rs`(`join`)
- Modify: `extras/prelude.fung`(`_list`, `first`, `take`; 머리 주석의 Go 세 줄)
- Modify: `crates/fung-vm/src/vm/channel.rs:456-467`(`drain` 주석)
- Modify: `docs/design/05-concurrency.md:25-26`, `docs/design/02-values-and-types.md`(순회 대상), `CHANGELOG.md`
- Test: `tests/lang/channel_stream.fung`(새 골든), `crates/fung-vm/src/vm/task_tests.rs` 또는 채널 테스트 모듈

**Interfaces:**
- `nat::check_items(t, v, verb) -> Result<(), Stop>`: 이제 순회할 수 있는지만 본다(리스트·범위·채널, 아니면 E0506). 채널을 비우지 않고 `&mut Value`도 필요 없으므로 `check_items(v: &Value, verb: &str) -> Result<(), Stop>`로 바꾸고, 부르는 곳을 모두 고친다.
- `nat::each_until`: 채널이면 `channel::recv(t, ch, None)`을 값마다 부르고, `Received::Value(v)`마다 `f`를 부른다. `f`가 참을 돌려주면 거기서 멈춘다(남은 값은 채널에 남는다). `Closed`면 끝.
- `nat::items`: 전체가 필요한 함수용이라 지금처럼 `drain`.
- `need_fn_after`: 먼저 함수 검사, 그다음 `check_items`(채널을 비우지 않음).
- `extras/prelude.fung`: `fn _take_ch(ch, n, acc)`(recv를 n번, 닫히면 멈춤, 꼬리 호출)를 더한다. `take(xs, n)`과 `first(xs)`는 채널이면 이것을 쓴다(`first`는 `n = 1`에서 첫 원소 또는 `nil`). 나머지(`last`·`drop`·`reverse`·`zip`·`min`·`max`)는 `_list`로 전체를 받는다.

- [ ] **Step 1: 실패하는 골든** — `tests/lang/channel_stream.fung`. 보내는 쪽과 받는 쪽이 번갈아 찍는지는 상태로 순서를 정한다(sleep 없음). 랑데부 채널이면 보내기가 받기를 기다리므로 순서가 정해진다.
  ```
  ch = channel()
  spawn(fn(): each(1..=3, x => do: print("send ${x}"); send(ch, x) ;;); close(ch) ;;)
  for x in ch: print("got ${x}") ;;
  # expect: send 1
  # expect: got 1
  # expect: send 2
  # expect: got 2
  # expect: send 3
  # expect: got 3
  ```
  (실제 순서는 랑데부 규칙에 따라 한 번 돌려 보고 정하되, 반드시 "send 3"보다 "got 1"이 먼저 나와야 한다. 지금 동작은 got이 모두 마지막에 나와서 실패한다.)
  이어서 다음을 둔다.
  - `find`가 일찍 멈추고 남은 값이 채널에 남는 사례: 버퍼 채널 `[1,2,3]`을 닫고 `find(ch, x => x == 2)` 뒤 `recv(ch)`가 `Ok(3)`.
  - `take(ch, 2)` 뒤 `recv(ch)`가 남은 값.
  - `first(ch)`.
  - `map(ch, 3)`이 채널을 비우지 않고 `f` 에러를 내는 사례(뒤의 `recv(ch)`가 첫 값).
- [ ] **Step 2: 실패 확인** — `cargo test -p fung-cli --test golden`이 `channel_stream`에서 실패.
- [ ] **Step 3: VM 단위 테스트** — Review Focus의 두 가지.
  - `each_failure_leaves_rest`: 버퍼 3에 `1,2,3`, 닫기. `each(ch, x => x == 2 then fail("x") else nil)`는 에러이고, `recv(ch)`는 `Ok(3)`.
  - `resend_into_iterated_channel`: 버퍼 1에 `1`, `each(ch, x => x < 3 then send(ch, x + 1) else close(ch))`가 1·2·3을 보고 끝난다.
- [ ] **Step 4: 구현** — 위 Interfaces대로 바꾼다. `drain` 주석은 "Receives until the channel is closed: what the natives that need the whole collection (`sort`, `_list`) walk".
- [ ] **Step 5: 통과 확인과 bless** — 골든과 단위 테스트 통과. 이어서 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`, cli의 `run_expected`·`dump_expected`(`extras/prelude.fung`가 바뀌어 dump가 달라짐)·`tools_expected`. diff에서 채널 사례의 출력 순서만 바뀌었는지, `map(ch, 3)`류의 에러 순서가 바뀌었는지 본다.
- [ ] **Step 6: 문서와 커밋** — 05에 "채널을 순회하면 값이 올 때마다 함수를 부른다. 일찍 멈추는 함수(`find`·`any`·`all`·`take`·`first`)는 나머지를 채널에 남긴다. 전체가 필요한 함수(`sort`·`reverse`·`last`·`zip` 등)는 닫힐 때까지 받는다"를 적는다. 02도 맞춘다. 커밋 `feat(vm): iterate channels as values arrive`.

### Task 8: 정렬을 fung의 안정 병합 정렬로 (사용자 확인 필요)

결정표에 없던 항목이다. 계획 검토 때 사용자에게 묻는다. 거절하면 이 Task는 주석만 고친다.

**Files:**
- Modify: `crates/fung-vm/src/prelude/collections.rs:88-250`(`Stable` 구조체 삭제, `sorted_by`)
- Modify: `docs/design/06-native-and-stdlib.md`(`sort`)

**Interfaces:** `fn sorted_by(xs: Vec<Value>, keys: &[Value]) -> Result<Value, Stop>`(시그니처 그대로). 안에서 위에서 아래로 내려가는 안정 병합 정렬(인덱스 배열, 보조 버퍼 하나)을 쓰고, 첫 비교 에러에서 곧바로 멈추고 그 에러를 돌려준다. `slice::sort_by`는 쓰지 않는다. 비교가 실패할 수 있는데, 일관되지 않은 비교에서 표준 정렬이 패닉할 수 있기 때문이다.

- [ ] **Step 1: 실패하는 테스트** — `collections::tests::sort_is_stable_and_stops_at_first_error`:
  - `sort_by([(2,"a"),(1,"b"),(2,"c"),(1,"d")], p => p[0])`가 `[(1,"b"),(1,"d"),(2,"a"),(2,"c")]`.
  - 길이 0·1·2·21·1000(역순) 정렬.
  - `sort([1, "a"])`가 비교 에러(코드는 지금과 같음).
- [ ] **Step 2: 실패 확인** — 새 함수가 없어 컴파일 실패. 기존 구현으로는 통과할 수 있으니, 먼저 `Stable`을 지우고 실패를 본다.
- [ ] **Step 3: 구현** — 약 30줄.
- [ ] **Step 4: 통과 확인과 bless** — `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`. 섞인 타입을 정렬하는 사례에서 에러에 이름이 나오는 쌍이 바뀔 수 있다. 그 외에는 diff가 없어야 한다.
- [ ] **Step 5: 커밋** — `refactor(prelude): sort with a plain stable merge sort`.

### Task 9: 6c-1 정리

**Files:**
- Modify: Task 1~8에서 손댄 파일의 남은 Go 주석(`nat.rs`, `prelude.rs`, `collections.rs`, `strings.rs`, `convert.rs`, `conc.rs`, `stdin.rs`, `unicode/`, `extras/prelude.fung`)
- Modify: `docs/design/open-questions.md`의 6c-1 항목(유니코드, 런타임 메시지, `channel(n)`, 표준 입력, 채널 순회, "Go 방식 그대로"의 16진 실수·대소문자) — 각각 "→ 해결 (6c-1, 2026-10-17)"과 한 줄 요약. 실행 형식 검사는 "6c-2에서 문구"로 남긴다.
- Modify: 스펙 4절 6c-1 결정표에 Task 8의 결과(채택 여부)를 한 줄 더한다.

- [ ] **Step 1:** `grep -nw Go` 로 위 파일들을 보고 주석을 fung 규칙으로 다시 쓴다(동작 설명은 남기고 출처만 뺀다).
- [ ] **Step 2:** `cargo test -p fung-vm --lib`과 `cargo test -p fung-front`로 주석 정리가 아무것도 깨지 않았는지 본다(빠른 두 묶음만).
- [ ] **Step 3: 커밋** — `docs: close the stage 6c-1 items`.
