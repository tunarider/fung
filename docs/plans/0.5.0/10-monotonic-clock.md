# L10. `now()`뿐이라 경과 시간이 시스템 시각 변경에 흔들린다

상태: **닫힘 (2026-10-27)**. 선택지 (나), 정할 것 1~5 모두 추천대로(사용자 결정 2026-10-27). 목록: [README](README.md).

결과: `native/time.rs`의 `monotonic_fn`과 기준점 `ORIGIN`(`Runtime::new`가 `start_clock`으로 처음 찍음), `extras/time.fung`의 `monotonic`, 효과 표의 순수 쪽(`now`와 같음).
매뉴얼(배우기 12·13장, 레시피 `check-urls`, `prelude/concurrency.md`, `time.md`)과 예제(`urlcheck`, `api`·`serve`의 `uptime`)의 경과 시간을 바꿨다. 파일 `mtime`과 견주는 `find-big-files`처럼 시각이 필요한 곳은 `now()`로 둔다.

## 문제

fung에서 걸린 시간을 재는 방법은 `time.now()` 두 번의 차뿐이다. 매뉴얼과 예제가 모두 이렇게 가르친다
(`manual/learn/12-concurrency.md`·`13-json-and-http.md`, `manual/recipes/check-urls.md`, `manual/reference/prelude/concurrency.md`, `examples/urlcheck.fung`, `examples/api.fung`의 `uptime`).

```
$ target/release/fung -e 'import time
a = time.now()
sleep(20ms)
b = time.now()
print(type_of(b - a), b - a > 15ms)'
'dur true
```

`now()`는 시스템 벽시계(`SystemTime`)만 읽는다. 두 호출 사이에 NTP가 시계를 고치거나 사람이 시각을 바꾸면 `b - a`가 그만큼 틀리고, 음수도 될 수 있다.
그러면 `elapsed < 600ms` 같은 판정이 거짓말을 하고, `took`를 기록하는 감시 스크립트가 음수나 몇 시간짜리 값을 남긴다. SRE 용도(재시도 간격, 응답 시간 기록)에서 흔한 계산이라 "드물다"로 넘길 문제가 아니다.
매뉴얼 `manual/reference/modules/time.md`의 `now()` 설명이 이 한계를 적어 두었지만, 대안이 없으므로 사용자는 피할 수 없다.

시스템 시각을 바꾸는 재현은 루트 권한이 필요해 여기서는 하지 않았다. 원인은 코드로 확인했다(아래).

## 지금 동작과 원인

- `crates/fung-vm/src/native/time.rs`의 `now_fn`: `std::time::SystemTime::now().duration_since(UNIX_EPOCH)`로 초·나노초를 얻어 `Time::from_unix`로 만든다. 주석 "no monotonic reading".
- `crates/fung-vm/src/value/time.rs`의 `Time`: `unix: i64`, `nsec: u32`, `loc: Loc`뿐이다. 단조 값을 실을 자리가 없다. `time - time`은 `sub_times`(벽시계 차), `==`·`<`는 `compare.rs`의 `equal`·`cmp_instant`(순간 비교).
- `extras/time.fung`은 `_native.time`의 이름을 그대로 내보낸다. 명세는 `spec/builtins.txt`의 `module _native.time now()` 등.
- 런타임 안쪽은 이미 단조 시계다. `sleep`(`crates/fung-vm/src/conc.rs`), 기한(`vm/deadline.rs`), 주차 뒤 기한 확인(`vm.rs`의 `after_park`), 소켓 기한(`native/net.rs`), 명령 종료 유예(`native/process.rs`)가 모두 `std::time::Instant`를 쓴다.
  `SystemTime`을 쓰는 곳은 `now_fn`, `fs.rs`의 임시 이름 난수 씨앗, 시험 도우미뿐이다. 그래서 `timeout`·`sleep`·`http`의 `timeout:`은 시계 변경에 흔들리지 않고, 문제는 사용자가 직접 재는 경과 시간에만 있다.
- 배경: R4b 노트 3절(`docs/notes/R4b-time-json.md`). Go는 `time.Now()`에 단조 값을 숨겨 두는데 이식 때 옮기지 않았다.

## 고려할 점

- 벽시계와 단조 시계는 용도가 다르다. 벽시계는 "지금 몇 시인가"(기록·출력·달력 계산), 단조 시계는 "얼마나 지났나"(시간 재기·기한)에 맞다. 단조 시계의 값 자체는 뜻이 없고 차만 뜻이 있다.
- 숨은 상태 문제: Go식으로 `time` 값에 단조 값을 숨기면 같은 ISO 출력을 가진 두 값의 `-`·`<` 결과가 출력과 어긋날 수 있다. Go는 이 때문에 `==`과 `Equal`이 다르고, `Round(0)`으로 단조 값을 벗기는 규칙, 직렬화하면 사라지는 규칙을 문서로 길게 설명한다.
  fung의 `time`은 순수 값이고(02, `time.rs` 머리 주석) `==`는 "같은 순간"이다. 숨은 필드는 이 규칙을 흐린다.
- 다른 언어: Python `time.monotonic()`·`perf_counter()`(실수 초), Rust `Instant`(별도 타입), Java `System.nanoTime()`, Erlang `erlang:monotonic_time()`(시작점이 정해지지 않은 정수), Node `performance.now()`(시작 기준 밀리초). 대부분 벽시계와 따로 둔다.
- 결과의 타입: fung에는 이미 `dur`가 있다. "기준점부터 지난 기간"을 `dur`로 주면 `t1 - t0`이 그대로 `dur`이고 `< 600ms`와 비교된다. 새 타입(`instant`)은 타입 표·비교 규칙·출력·명세를 모두 늘린다.
- 기준점: 프로세스 시작(정적 `OnceLock<Instant>`)으로 하면 값이 작고 출력이 읽힌다(`1.234s`). `fung test`가 파일마다 런타임을 새로 만들어도 프로세스 안에서 이어진다. `dur`는 `i64` 나노초라 292년까지 넘치지 않는다.
- 잠자기(suspend): Rust `Instant`는 Linux에서 `CLOCK_MONOTONIC`, macOS에서 `mach` 시계로, 둘 다 기계가 잠든 동안 멈춘다(Linux `CLOCK_BOOTTIME`은 잠든 시간도 센다). `sleep`·기한과 같은 시계를 쓰는 편이 일관된다. 이 차이는 문서에 적는다(확인은 구현 때 Rust 표준 라이브러리 문서로).
- 시험: 시스템 시각을 바꾸는 시험은 루트가 필요해 CI에서 못 한다. 성질(줄지 않음, `sleep(d)` 뒤에 `d` 이상 늚, 시작 직후 작음)로 시험한다.

## 선택지

(가) **Go식 숨은 단조 값.** `Time`에 `Option<Instant>`(또는 기준점부터의 나노초)를 더하고, 둘 다 가진 두 값의 `-`·`<`·`==`에 그것을 쓴다. `+ dur`는 같이 옮기고, `utc`·`local`은 유지, `parse`·`from_unix`·직렬화는 없음.
장점: 기존 코드(`time.now() - t0`)가 그대로 옳아진다. 단점: 위의 숨은 상태 문제, `==`의 뜻이 두 갈래가 됨, `Time`이 커짐, 규칙 설명이 길다.

(나) **따로 둔 단조 시계 함수.** `time.monotonic()`이 프로세스 시작부터 지난 `dur`를 돌려준다. 경과 시간은 `time.monotonic() - t0`. `now()`는 벽시계로 남기고 설명에 "경과 시간은 `monotonic()`"을 적는다.
장점: 값이 순수하게 남고 규칙이 한 줄이다. 기존 `dur` 산술을 그대로 쓴다. 단점: 사용자가 둘을 구별해야 한다. 매뉴얼·예제의 시간 재기를 모두 바꿔야 한다.

(다) **(나) + 재는 도우미.** `time.measure(f)` → `(결과, dur)` 같은 함수를 함께 둔다. 편하지만 새 기능이라 README 3절 범위에 걸리고, (나)만으로 문제는 풀린다.

(라) **그대로 두고 문서만.** 이미 적혀 있다. 대안이 없어 문제가 남는다.

추천: (나). 문제의 원인이 "두 용도를 한 시계로 함"이므로 용도별로 나누는 것이 근본 해결이다. `dur`를 돌려주면 새 타입 없이 기존 산술·비교가 맞고, `time` 값의 "같은 순간이면 같다"는 규칙이 흐려지지 않는다.

## 정할 것

1. 방식: (가)·(나)·(다)·(라) 중. 추천 (나).
2. 이름: `time.monotonic()`, `time.elapsed()`, `time.uptime()`, `time.clock()`. 추천 `time.monotonic()`(뜻이 정확하고 Python과 같다. `elapsed`는 "무엇부터?"가 모호하다).
3. 기준점: 프로세스 시작, 런타임 시작, 정하지 않음(값을 뜻 없는 것으로 명시). 추천 프로세스 시작이되 명세에는 "기준점은 정하지 않으며 차만 뜻이 있다"고 적는다(나중에 바꿀 여지를 남김).
4. 잠든 시간: 세지 않음(지금 `sleep`·기한과 같은 시계), 셈(Linux `CLOCK_BOOTTIME`, macOS는 잠든 시간을 세는 다른 시계). 추천 세지 않음(런타임 안의 기한과 같은 시계라야 `timeout`과 잰 값이 맞는다).
5. 매뉴얼·예제의 `time.now() - t0`를 모두 `monotonic()`으로 바꿀지. 추천 바꾼다(시간 재기의 올바른 본보기만 남긴다). `examples/api.fung`의 `uptime`도 바꾼다.

## 할 일

1. 시험 먼저: `crates/fung-vm/src/native/time_tests.rs`에 `monotonic`이 `dur`이고, 두 번 부르면 줄지 않고, `sleep(20ms)` 뒤에 20ms 이상 늘며, 시작 직후 작다(예: 60초 미만)는 시험. 골든 `tests/lang/`에 `type_of(time.monotonic())`과 `>=` 성질. 실패를 확인한다.
2. `native/time.rs`에 `monotonic_fn`과 정적 기준점(`OnceLock<Instant>`; 실행 파일 시작 때 한 번 찍도록 `boot`에서 미리 읽을지 구현 때 본다). `extras/time.fung`에 `monotonic = _n.monotonic`.
3. `FUNG_UPDATE_SPEC=1 cargo test spec_file`로 `spec/builtins.txt` 갱신, diff 검토.
4. `value/time.rs`·`native/time.rs` 주석의 "no monotonic reading"을 "경과 시간은 `monotonic`"으로 고친다.
5. 설계 문서: 06의 `time` 표 행(`monotonic()` 추가), 02의 `time` 행에 "경과 시간은 `time.monotonic()`의 차"를 덧붙인다. open-questions "알려진 한계"의 시간 줄을 지우고 닫은 기록을 `docs/notes/open-questions-closed.md`로.
6. 매뉴얼: `manual/reference/modules/time.md`에 `:::fn time.monotonic()` 항목(매개변수 표·예제), `now()` 설명을 고친다. 정할 것 5에 따라 배우기 12·13장, 레시피, `prelude/concurrency.md`, `examples/urlcheck.fung`·`api.fung`를 바꾸고
   기대 출력(`FUNG_EXPECTED=bless …`)과 매뉴얼(`FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`)을 다시 만들어 diff를 검토한다.
7. `CHANGELOG.md` `[Unreleased]`의 더함에 `time.monotonic()`.
8. 커밋 전 검사 전부(CLAUDE.md "커밋 전 검사").

## 끝의 기준

- `time.monotonic()`이 있고 `dur`를 돌려주며, 줄지 않음과 `sleep` 뒤 증가가 시험으로 지켜진다.
- 저장소의 매뉴얼·예제에 경과 시간을 `time.now()`의 차로 재는 곳이 남지 않는다(`grep -rn 'now() - ' manual examples`가 비어 있음, 정할 것 5가 "바꾼다"일 때).
- `now()`의 매뉴얼 설명과 06·02가 두 시계의 용도를 말한다. open-questions에서 이 한계가 빠졌다.
- 커밋 전 검사가 모두 통과한다.
