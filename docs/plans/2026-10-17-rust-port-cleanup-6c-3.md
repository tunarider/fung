# Rust 이식 6c-3 (시간·JSON) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-3 결정표(T1~T8, J1~J6, U)를 fung 규칙으로 구현하고 손댄 파일의 Go 이름·주석을 걷어낸다.

**Architecture:** 시각 값(`value/time.rs`)을 유닉스 초 + 나노초로 다시 쓰고, 시간대(`native/tz.rs`)의 벽시계 변환·POSIX 규칙·찾기 규칙을 고치고, 서식·파싱(`native/timefmt.rs`)을 fung 규칙과 문구로 바꾼다. JSON(`native/json.rs`)은 파서를 새로 쓰고 인코더의 이스케이프·한도를 바꾼다. UTF-8 해독(`fung_front::source::decode_go`)을 표준 방식으로 바꾼다.

**Tech Stack:** Rust 표준 라이브러리.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` ("원칙", "2. 검증", "4. 6c-3 시간·JSON")

## Global Constraints

- 6c-1·6c-2 계획의 Global Constraints를 그대로 따른다. bless 도우미 `.superpowers/sdd/6c-tools/bless.sh [테스트…]`, fung-vm은 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`. 시간 기대 출력은 `time_expected`, JSON은 `json_expected`.
- 각 Task의 마지막 확인은 `cargo test -q -p fung-vm --lib`와 그 영역의 cli 테스트.
- 시간 범위: UTC 기준 `0000-01-01T00:00:00Z` ≤ t ≤ `9999-12-31T23:59:59.999999999Z`(유닉스 초 −62,167,219,200 ~ 253,402,300,799).
- 기대 출력은 `TZ=UTC`로 돈다(`common::TZ`); 시간대 테스트는 단위 테스트에서 이름으로 읽은 zone으로 한다.

## Review Focus

- 범위 끝의 시각에 오프셋을 더해 보일 때(`0000-01-01T00:00:00Z`를 `-05:00`으로 보기): 연도가 −1이 되어도 패닉 없이 보인다. → Task 1 `edges_show_in_any_offset`.
- 서머타임 경계: 뉴욕 `2024-03-10 02:30`(없음) → `03:30-04:00`, `2024-11-03 01:30`(두 번) → `01:30-04:00`; 베를린 `2024-10-27 02:30` → `02:30+02:00`. → Task 2.
- `TZ`가 잘못된 값이면 UTC로 조용히 돈다(문서화된 규칙). `TZ=KST-9`는 `+09:00`. → Task 2.
- JSON에서 `[1, 2,]`의 에러는 `]` 자리(열 7), 끝에 남은 글자 `{"a":1} x`는 `x` 자리, 깊은 중첩 10,001은 거절, 10,000은 통과. → Task 4.
- 잘린 UTF-8 `\xE2\x82`는 U+FFFD 하나, `\xFF\xFF`는 둘. → Task 6.

---

### Task 1: 시각 값 (T2, T6의 출력)

**Files:** Modify `crates/fung-vm/src/value/time.rs`(다시 쓰기), `crates/fung-vm/src/native/time.rs`(`from_unix`, `now`), `value/time_tests.rs`.

**Interfaces:**
- `pub struct Time { unix: i64, nsec: u32, loc: Loc }`. 상수 `MIN_UNIX: i64 = -62_167_219_200`, `MAX_UNIX: i64 = 253_402_300_799`.
- `pub(crate) fn from_unix(sec: i64, nsec: i64, loc: Loc) -> Option<Time>`(범위 밖이면 `None`; `nsec`는 초로 넘침).
- `add(&self, d: i64) -> Option<Time>`, `sub(&self, u: &Time) -> Option<i64>`(i128로 계산, 범위·i64 밖이면 `None`). `add_time_dur`·`sub_times`는 `None`을 E0503으로.
- `unix() -> i64`, `nanosecond()`, `zone() -> (&str, i32)`, `date()`, `clock()`, `weekday()`(0 = 일요일): 날짜는 일수 ↔ 연월일 변환(Hinnant의 `civil_from_days`, 주석에 출처 이름만)으로 직접 계산한다. `tz::abs_date`·`UNIX_TO_ABSOLUTE` 등 절대 기원 상수에 기대지 않는다.
- `iso()`: RFC 3339, 소수는 0이 아닐 때만, 오프셋은 `Z`·`±hh:mm`, 초가 있으면 `±hh:mm:ss`(T6). 연도는 4자리(음수는 `-0001`).
- `time.from_unix(n)`의 E0403 문구: ``argument `n` must be a unix time from year 0000 to 9999, got <n>``.

- [ ] **Step 1: 실패하는 테스트**(`time_tests.rs`): `range_edges`(`from_unix(MIN_UNIX)`·`MAX_UNIX` 된다, ±1 밖은 `None`), `add_and_sub_stay_in_range`(끝에서 1 ns 더하면 `None`, `MAX - MIN`이 i64 나노초를 넘으므로 `None`), `edges_show_in_any_offset`(`MIN_UNIX`를 `Fixed(-18000)`으로 `iso()` → `-0001-12-31T19:00:00-05:00`), `offset_seconds_show`(`Fixed(8*3600+27*60+52)`의 `iso()`가 `+08:27:52`로 끝남), `dates_match_known_days`(1970-01-01 목요일, 2000-02-29, 2024-12-31, 0000-03-01).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**. `native/time.rs`의 `from_unix_fn`·`now_fn`을 새 `Option`에 맞춘다.
- [ ] **Step 4: 통과 확인과 bless** — vm lib, `bless.sh time_expected json_expected`, fung-vm bless. 옛 감싸기·붙기 단위 테스트는 새 규칙으로 바꾼다.
- [ ] **Step 5: 커밋** — `feat(vm): keep times as unix seconds within years 0000 to 9999`. 02·06의 시간 절, CHANGELOG.

### Task 2: 시간대 (T1, T5, T7)

**Files:** Modify `crates/fung-vm/src/native/tz.rs`(`resolve_local`, `Zone::date`, `tzset`의 해 계산, 상수와 주석), `tz_tests.rs`.

**Interfaces:**
- `pub(crate) fn resolve_local(tz: Option<&OsStr>, tzdir: Option<&OsStr>) -> Zone`: 없음 → `/etc/localtime`; `""`·`UTC` → UTC; 앞 `:` 뗌; 절대 경로 → 그 파일; 상대 이름 → `tzdir`(비지 않으면) 아니면 `/usr/share/zoneinfo`; 파일이 없거나 TZif가 아니면 **POSIX 문자열**로 `parse_footer`를 시도(되면 그 규칙만 가진 zone); 다 실패하면 UTC. `local()`은 `TZ`·`TZDIR`을 읽는다.
- `Zone::date(...) -> (i64, i32)`(시그니처 그대로): 벽시계 초 `L`의 후보 오프셋 `oa = lookup(L).offset`, `ob = lookup(L - oa).offset`; 유효한 것(`lookup(L - o).offset == o`)이 있으면 그중 **가장 큰 오프셋**(이른 순간), 없으면(빈틈) **가장 작은 오프셋**(빈틈만큼 뒤로 밀린 시각).
- `tzset`: 해의 시작을 실제 날짜 계산으로(윤년 366일).
- `PLATFORM_ZONE_SOURCES`는 지운다.

- [ ] **Step 1: 실패하는 테스트**(`tz_tests.rs`): Review Focus의 서머타임 세 사례(`load("America/New_York")`, `load("Europe/Berlin")`, 시스템에 파일이 없으면 건너뜀), `posix_tz_string`(`resolve_local(Some("KST-9"), None)`이 +09:00, `EST5EDT,M3.2.0,M11.1.0`이 7월에 −04:00), `tzdir_is_used`(임시 디렉터리에 `Asia/Seoul` 파일을 복사하고 `TZDIR`로), `posix_rule_in_leap_years`(규칙만 있는 zone에서 2028-03-12의 전환 시각이 맞다).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**.
- [ ] **Step 4: 통과 확인과 bless**.
- [ ] **Step 5: 커밋** — `feat(vm): resolve the local zone like the C library and pick DST wall times explicitly`. 06 시간 절에 규칙(T1·T5).

### Task 3: 서식과 파싱 (T3, T4, T6의 파싱, T8)

**Files:** Modify `crates/fung-vm/src/native/timefmt.rs`(`parse_rfc3339` 다시 쓰기, `parse_time` 문구, `pad`·`time_quote`·`parse_error`·Go 레이아웃 상수 삭제), `timefmt_tests.rs`.

**Interfaces:**
- `parse_rfc3339(s: &str) -> Result<Time, String>`(지역 zone 인자는 필요 없어진다: 오프셋은 항상 `Loc::Fixed`, `Z`는 `Loc::Utc`). 문법: `YYYY-MM-DD[Tt]hh:mm:ss[.f{1,9}](Z|z|±hh:mm[:ss])`. 범위: 월 1~12, 일 1~그 달, 시 0~23, 분 0~59, 초 0~59, 오프셋 시 0~23·분 0~59·초 0~59.
- 에러 이유(문자열만, 호출하는 쪽이 ``cannot parse `x` as a time: `` 를 붙임): `expected 4 digits for the year`, `expected 2 digits for the month`(일·시·분·초·오프셋 시·분 같음), ``expected `-` after the year`` 같은 구분자, `month out of range`, `day out of range`, `hour out of range`, `minute out of range`, `second out of range`, `offset out of range`, `expected 1 to 9 digits after the decimal point`, `expected Z or an offset like +09:00`, `unexpected text after the time`, `time out of range`(연도 범위).
- 토큰 서식의 `pad`는 4자리 연도만 다루면 되므로 `{:04}`/`{:02}`로. 이름 토큰 에러는 `expected a month name like Oct`·`expected a weekday name like Sun`.

- [ ] **Step 1: 실패하는 골든** `tests/lang/time_rules.fung`(`import time`):
  ```
  print(time.parse("2026-10-17T10:02:03Z"), time.parse("2026-10-17t10:02:03.5z"), time.parse("2026-10-17T10:02:03+08:27:52"))
  # expect: Ok(2026-10-17T10:02:03Z) Ok(2026-10-17T10:02:03.5Z) Ok(2026-10-17T10:02:03+08:27:52)
  print(is_err(time.parse("2026-10-17T1:02:03Z")), is_err(time.parse("2026-10-17T10:02:03,5Z")), is_err(time.parse("2026-10-17T10:02:03+24:00")), is_err(time.parse("2026-10-17T10:02:03.1234567891Z")), is_err(time.parse("2026-10-17 10:02:03Z")))
  # expect: true true true true true
  print(time.parse("2026-13-01T00:00:00Z"))
  # expect: Err({kind: 'parse, message: "cannot parse `2026-13-01T00:00:00Z` as a time: month out of range"})
  ```
  와 `time.format(time.parse("2026-10-17T10:02:03+09:00")!, "Z")`가 `+09:00`(고정 오프셋, T4).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**.
- [ ] **Step 4: 통과 확인과 bless** — 골든 사례 수 상수, `bless.sh time_expected json_expected`, fung-vm bless.
- [ ] **Step 5: 커밋** — `feat(vm): parse RFC 3339 strictly with fung's messages`. 06 시간 형식 절, 매뉴얼 `time.parse`, CHANGELOG.

### Task 4: JSON 파서 (J1~J4)

**Files:** Modify `crates/fung-vm/src/native/json.rs`(`Wire`·`Decoder`·`Cause`·`V1Err`·`transform`·`replace_v2_words`·`decode` 등을 지우고 새 파서), `json_tests.rs`.

**Interfaces:**
- `fn decode(s: &str) -> Result<Value, JsonError>`(이름 그대로), `struct JsonError { line: usize, col: usize, msg: String }`. 위치는 문제가 있는 글자의 줄·열(1부터, 글자 단위, BOM도 한 글자); 입력 끝이면 끝 위치.
- 반복(명시적 스택) 파서. 공백은 RFC 8259의 넷(스페이스, 탭, LF, CR)뿐. 숫자는 RFC 8259 문법, 소수점·지수가 없으면 int(i64 밖이면 `number out of range`), 아니면 유한한 float.
- 문구: `unexpected character 'c'`(`c`는 `fung_front::diag::quote`처럼 보이지 않는 글자를 `\u{X}`로), `unexpected end of input`, `unexpected text after the value`, `expected a string for an object key`, ``expected `:` after an object key``, ``expected `,` or `}` ``, ``expected `,` or `]` ``, `invalid escape`, `invalid \u escape`, `unpaired surrogate`, `control character in a string`, `invalid number`, `number out of range`, `nesting too deep (limit 10000)`, ``duplicate key `k` ``(키는 fung 리터럴로 따옴표 안에).
- 바깥 문구는 그대로 `invalid JSON at line L, column C: <msg>`.

- [ ] **Step 1: 실패하는 테스트**(`json_tests.rs`): `errors_point_at_the_problem`(`[1, 2,]` → 1:7 `unexpected character ']'`, `{"a":1} x` → 1:9 `unexpected text after the value`, `[1` → 1:3 `unexpected end of input`, `"\ud800"` → `unpaired surrogate`, `{"a":1,"a":2}` → ``duplicate key `a` ``, `"\x01"`(실제 제어 문자) → `control character in a string`, `01` → `invalid number`), `depth_limit`(10,000 통과, 10,001 거절), `values_keep_order_and_exact_ints`(지금 테스트 재사용).
- [ ] **Step 2: 실패 확인**.
- [ ] **Step 3: 구현**.
- [ ] **Step 4: 통과 확인과 bless** — vm lib, `bless.sh json_expected http`, fung-vm bless. 옛 Go 문구 단언 테스트는 새 규칙으로.
- [ ] **Step 5: 커밋** — `feat(vm): parse JSON with one plain parser and fung's messages`. 06 JSON 절, 매뉴얼, CHANGELOG(같은 키·짝 없는 서로게이트는 에러).

### Task 5: JSON 출력 (J5, J6)

**Files:** Modify `json.rs`의 `quote_json`, `newline`, `encode`.

**Interfaces:** `quote_json`은 `"` `\` `\b \f \n \r \t`, 그 밖의 U+0020 미만은 `\u00XX`(소문자 16진), 나머지 그대로. `newline`의 큰 들여쓰기 분기는 지우고, `encode`가 결과 글자 수를 세어 `strings::MAX_CHARS`를 넘으면 `too_many_chars()`(6c-1의 E0806).

- [ ] **Step 1: 실패하는 테스트**: `stringify("\u{2028}")`이 `"\"\u{2028}\""`(그대로), `stringify([[1]], indent: 100000000)`이 E0806 `too many characters`.
- [ ] **Step 2~4**: 실패 확인, 구현, 통과·bless.
- [ ] **Step 5: 커밋** — `fix(vm): escape only what JSON needs and cap stringify output`.

### Task 6: UTF-8이 아닌 바이트 (U)

**Files:** Modify `crates/fung-front/src/source.rs`(`decode_go` → `decode_lossy`), 모든 호출하는 곳(약 43곳, `grep -rn decode_go crates`).

**Interfaces:** `pub fn decode_lossy(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).into_owned() }`(유니코드 권장 방식: 잘못된 부분 수열마다 U+FFFD 하나).

- [ ] **Step 1: 실패하는 테스트**(`source.rs`): `decode_lossy(b"a\xE2\x82b") == "a\u{FFFD}b"`, `decode_lossy(b"\xFF\xFF") == "\u{FFFD}\u{FFFD}"`.
- [ ] **Step 2~4**: 실패 확인, 구현과 이름 바꾸기, 통과·bless(`bless.sh http tls json_expected`, fung-vm bless, `stdin`·`sigpipe`).
- [ ] **Step 5: 커밋** — `refactor: decode invalid UTF-8 the way Unicode recommends`. 02·06의 U+FFFD 서술, 매뉴얼, CHANGELOG의 "알려진 차이".

### Task 7: 6c-3 정리

- [ ] **Step 1:** 손댄 소스 파일(`value/time.rs`, `native/time.rs`, `tz.rs`, `timefmt.rs`, `json.rs`, `source.rs`)의 남은 Go 주석, `timefmt::quote_bytes`가 남았다면 정리.
- [ ] **Step 2:** open-questions의 4b 시간·JSON 항목 다섯 개(시간대 탐색, 시각 표현, 서식·파싱, jsontext 경로, 이스케이프)와 "알고 남긴 차이"의 U+FFFD를 해결로.
- [ ] **Step 3:** `cargo test -q -p fung-front && cargo test -q -p fung-vm --lib`, 커밋 `docs: close the stage 6c-3 items`.
