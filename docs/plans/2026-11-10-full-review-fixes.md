# 두 번째 전체 점검의 결정과 고침 계획 (2026-11-10)

`docs/plans/2026-11-10-full-review.md`(이하 "점검 문서")의 74개 항목을 **한 작업 브랜치에서 모두** 고치는 계획이다.
1절은 사용자 결정(2026-11-10), 2절은 고칠 때의 규칙, 3절은 작업 순서와 묶음, 4절은 묶음마다 할 일, 5절은 마무리다.
항목의 재현·원인 위치·근거는 점검 문서에 있고 여기서는 되풀이하지 않는다.

## 1. 결정 (2026-11-10, 사용자)

**지침**: 하위 호환 무시, 엄격함과 일관성 우선, 예외·특수한 경우 최소화(매뉴얼이 간단해지는 쪽).
사용자는 "모두 추천대로"로 정했다. 다만 추천을 엄격함·일관성 기준으로 다시 본 뒤 일곱 곳을 고쳤다(★: 점검 문서의 추천과 다름).
고칠 때 각 항목의 점검 문서 절에 `처리:` 줄을 달고, 결정이 점검 문서와 다르면 그 줄에 이 표의 결정을 적는다.

### 1.1 언어 핵심 규칙

| 번호 | 항목 | 결정 |
|---|---|---|
| D1 | CC9 | 범위가 **취소로** 되감기는 중(`cancel(s)`·바깥 취소·기한·`exit`; 원인이 자식의 실패가 아님)에 읽을 수 없게 된 `Err`는 실패로 세지 않는다. 읽는 태스크만 `cancel(t)`로 끊은 경우는 지금처럼 E0607. `FailFast`가 자식 실패로 취소한 경우의 `note: also failed`는 그대로. |
| D2 | SP12 | (가) `Ok`·`Err`는 인자가 **정확히 하나**. 식·패턴·파이프(`x \|> Ok(2)`)에서 개수가 다르면 컴파일 에러(E0117을 넓히거나 새 번호; 구현 때 카탈로그에서 정함). 펼치기 등 컴파일 때 개수를 모르는 경우는 런타임 에러. "보통 태그 `Ok`" 개념과 그 설명(02, `results.md`, E0605 문구의 "with 2 values")을 지운다. 판정 함수는 `result_kind` 하나로(`result_tag` 삭제). 타입 표기 `Ok`·`Err`는 "인자 1개 태그"만 받는다(심볼 `'Ok`는 통과하지 않음). |
| D3 | SP13 | `nil?`은 `nil!`과 같이 E0604. "`!`·`?`는 결과값만 받는다" 규칙 하나. |
| D4 | FE23 | 최상위 `fn` 안쪽 블록에서 바깥(상수) 이름에 대입하면 E0205. 함수 본문 맨 바깥 블록에서 묶은 이름만 지역(`own_bindings`가 안쪽 `Do`로 내려가지 않음). |
| D5 | FE24 | 버려지는 자리의 효과 없는 식은 컴파일 에러(새 번호). 효과 없는 식 = 이름, 리터럴, 람다·익명 `fn`, 부분 적용, 그것들만으로 된 연산자·필드·인덱스·튜플·리스트·맵. 도움말: 이름이면 "call it with `()`", 비교면 "did you mean `=`?". 일부러 버리려면 `_ = 식`. |
| D6 | FE25 | 가드 없는 모두 맞는 절(`_`, 맨 이름, 그것들만의 `\|` 대안) 뒤에 절이 있으면 컴파일 에러(새 번호). 맨 이름이 바깥 변수와 같으면 도움말 "compare with a guard: `v when v == limit`". `=>` 바로 뒤 람다의 E0112에 괄호 도움말. |
| D7 | FE27·SP19 | 괄호를 트리에 남긴다. 파이프는 괄호 없는 호출·태그일 때만 끼워 넣고, 그 밖의 식(괄호 친 호출 포함)은 값을 구해 부른다(`x \|> (mk(10))` = `mk(10)(x)`). 슬라이스는 "범위 값으로 하는 색인" 하나로 일반화해 `xs[r]`도 된다(E0516 포함 같은 규칙). 슬라이스 시작·끝 자리는 `??`·`\|>`도 받는다. ★ **범위 값도 시작이 끝보다 뒤면 에러**: `a..b`는 `a > b`, `a..=b`는 `a > b + 1`이면 에러이고, 빈 범위는 `n..n`·`n..=n-1`뿐이다(슬라이스의 E0516과 같은 선). 번호는 구현 때 정함(E0516의 뜻을 바꾸지 않는 쪽: 새 번호 또는 E0516을 슬라이스 전용으로 두고 범위용 새 번호). |
| D8 | FE38 | 매개변수 기본값 안의 `return`은 컴파일 에러(E0211 문구 확장). `test` 블록의 `return`은 허용("test 본문은 함수 본문"), `testing.md`에 한 줄. |
| D9 | FE31 | 진법 정수도 `-` 뒤 최소값 특례. 기간의 모든 성분에 앞자리 0 규칙, 소수는 마지막 성분만(`1.5h30m`은 E0105). 0이 아닌 리터럴이 0으로 반올림되면(기간 ns, 실수 아래 넘침) E0102/E0105. 런타임 `float`·`parse_float`도 같은 뿌리로(D12). |
| D10 | FE32 | 대입·`for`·매개변수의 패턴을 "구조 분해 패턴" 하나로 통일하고 `grammar.md`에 `destructure` 규칙을 따로 둔다. `match` 리터럴에 `-` 기간을 더한다. |

### 1.2 값과 prelude

| 번호 | 항목 | 결정 |
|---|---|---|
| D11 | SP15 | 개수·한도는 음이 아닌 정수, 아니면 E0403(`must be a non-negative int`): `take`·`drop`·`repeat`·`pad_left/right`·`iterate`·`retry times`(1 이상; "`times` ≤ 1이면 한 번" 특례 삭제)·`pmap limit`. `pmap`의 "제한 없음"은 `limit = nil`(기본). `take`·`drop`의 위쪽 깎기는 그대로. ★ **음수 기간도 E0403**: `sleep`·`timeout`·`retry delay`(그리고 기간을 받는 모든 내장 함수·모듈 인자). 남은 시간을 계산해 음수가 될 수 있는 코드는 `max(d, 0s)`로 드러낸다. `sum([])`는 `0`(문서에 "기간이면 `fold(xs, 0s, …)`"). 02에 "nil·Err·런타임 에러를 고르는 기준" 한 절. |
| D12 | SP14 | `parse_int`·`parse_float`·`parse_dur`(그리고 `int`·`float`의 문자열 갈래)는 "리터럴 문법 + 앞의 부호 `-`/`+` 하나 + 앞뒤 공백 거절" 하나. `"01"`은 거절(도움말 `0o`). `parse_dur`의 `trim_matches` 삭제. 에러 문구는 입력을 따옴표로. |
| D13 | SP21 | 맵을 키로 찾거나 고치는 모든 자리(`m[k]`, 동적 키, `k in m`, `get`·`put`·`remove`, `group_by`)에서 문자열이 아닌 키는 E0514. 심볼 키면 도움말 "map keys are strings: …". |
| D14 | SP23 | ★ **엄격한 쪽**: `take`·`drop`의 문자열 갈래를 없앤다. 컬렉션 함수(`first`·`last`·`reverse`·`take`·`drop`)는 리스트(와 범위)만, 문자열은 슬라이스 `s[0..n]`와 `chars`로. `strings.md`·`collections.md`에 관용구 한 줄. |
| D15 | SP26 | 심볼 → 문자열은 `sym_name(s)`(다른 타입은 E0403). `sym("a")`의 짝. |
| D16 | VA18 | `sum`·`min`·`max`의 원소 하나도 `+`·`<`의 검사를 받는다(버그). ★ **`sort`의 원소 하나도 비교 가능한지 검사**(`x < x`와 같은 E0501). |
| D17 | VA19 | 호출 경로에서 `Builtin` 표시 프레임을 감춘다(주 위치를 옮기는 규칙과 같게). `keys`·`values`·`chars`·`unique`는 자기 이름·동사로 말한다(`chars`는 E0403 `argument `s` must be str`). |
| D18 | VA20 | `split`의 빈 구분자는 E0403(`argument `sep` must be a non-empty str`). 글자로 나누기는 `chars` 하나. |
| D19 | VA21 | 범위의 `==`·해시는 원소 열로(둘 다 비면 같고, 아니면 첫 수·마지막 수가 같으면 같다). 출력은 쓴 모양 그대로. |
| D20 | VM15 | 내장 함수 인자를 작은 값은 안에 담는 타입(`Items`를 넓히거나 `Args`)으로 바꾼다. `unsafe` 없이. `internals.md` 145행 고침. |

### 1.3 라이브러리와 도구

| 번호 | 항목 | 결정 |
|---|---|---|
| D21 | SP16 | `fs.exists`는 `Ok(bool)`: `ENOENT`·`ENOTDIR`만 `Ok(false)`, 그 밖의 실패는 `Err`. 링크는 지금과 같은 기준(따라감)으로 두고 문서에 적는다. 저장소 안의 호출을 `fs.exists(p)!`로. |
| D22 | LB29 | 파일·명령·표준 입력·소켓에서 읽은 값이 문자열 한도를 넘으면 그 함수의 `Err`(`'io`, `… longer than the string limit (100000000 characters)`). `exec`는 읽는 중에 한도를 넘으면 명령을 끝내고(취소와 같은 길) `Err`. |
| D23 | LB24 | 자동 `Referer`를 없앤다(필요하면 사용자가 헤더로). 조각 문제도 함께 사라진다. |
| D24 | LB26 | 응답과 서버 `req`에 받은 순서 그대로의 `header_lines: [(name, value)]`를 더한다. `headers` 맵은 지금 규칙(", "로 이음) 그대로. |
| D25 | LB27 | ★ `req`에 `target`(받은 그대로의 요청 대상)과, 헤더와 같은 모양의 `query_pairs: [(name, value)]`(받은 순서, 반복 키 보존)를 더한다. 잘못된 쿼리 이스케이프는 경로처럼 400. `query` 맵은 지금 규칙(반복 키는 마지막이 이김)을 문서에 적는다. |
| D26 | LB28 | ★ 신호로 끝난 명령은 `code: nil` + `signal: n`, 문구 ``…`was killed by signal 9 (SIGKILL)` ``. 실제 종료 코드 137과 섞이지 않는다. |
| D27 | LB34 | 대소문자만 다른 같은 이름 사용자 헤더는 E0403, 빈 메서드는 E0403, 메서드는 쓴 그대로 보낸다. |
| D28 | LB36·SP25 | `time.parse`: 같은 필드를 두 번 쓰는 형식은 E0403, `ddd`는 날짜의 요일과 다르면 `Err 'parse`. `time.format`: 토큰이 없거나 `%`가 든 형식은 E0403(`%`면 도움말 "tokens are `YYYY`, …"). |
| D29 | LB38 | `cli.parse`: `bools` 원소를 `str`로 검사(E0403이 `bools`를 말함). ★ **같은 플래그의 반복은 실패**. 스위치의 `=값`은 `true`/`false`만. 실패는 `Err({kind: 'invalid, message})`(바깥 입력), 성공은 `Ok({flags, rest})` — 스크립트는 `cli.parse(args)!`. |
| D30 | TL6 | 인자로 준 경로가 없거나 디렉터리 순회 실패는 3, 읽기 실패(권한·UTF-8 아님)는 모든 명령에서 2. `fmt --check`의 1은 "고칠 파일 있음"만. |
| D31 | TL8 | `fung fmt`도 `decode_source`로 읽어 UTF-8이 아닌 소스를 거절(파일은 건드리지 않음, 종료 코드 D30). `lex_bytes`의 바이트 매핑 삭제. |

### 1.4 이름·카탈로그·문서

| 번호 | 항목 | 결정 |
|---|---|---|
| D32 | SP27 | 숫자 리터럴은 소문자 접두사·지수만(`0XFF`·`1E5`는 E0102, 도움말 소문자). 술어 `valid_utf8` → `is_utf8`. 파일 핸들 매개변수는 `file`, 패턴 매개변수는 `pattern`, 아무 값은 `x`. `env()`(전체 맵)는 `env_all()`로 나누고 `env(name)`만 남긴다. `range()` 함수는 삭제한다(범위를 만드는 길은 리터럴 하나; 사용자 결정 2026-11-10). 07의 "`range(...)` 함수와 같은 범위 값" 문장과 레퍼런스도 지운다. |
| D33 | SP24 | E0705 은퇴(E0709로 합침, 번호는 재사용하지 않음). E0224·E0231은 E0224를 남기고 E0231 은퇴(문구에 "ends with an assignment to `x`"). E0805 help "not used: every builtin is implemented". E0506 동사 정리(`unique`·`zip`·`get`). |
| D34 | SP17 | 설계 문서와 매뉴얼을 대조하는 시험은 **두지 않는다**. 이번에 빠진 규칙을 매뉴얼에 채우고, 설계 문서 머리마다 "규범은 매뉴얼 언어 명세, 이 문서는 결정의 기록"을 적는다. |

결정 없이 처리하는 것: VA18의 `sum`·`min`·`max` 부분(버그), SP22의 `fsync` 비용 문구(문서), LB24의 조각(D23으로 사라짐).

## 2. 고칠 때의 규칙

- **브랜치**: 지금 작업 브랜치(`claude/language-spec-vm-review-e5b085`) 위에서 한다. main 스쿼시는 사용자가 요청할 때.
- **커밋**: 4절의 묶음마다 하나(필요하면 둘). Conventional Commits, 영어, 동작이 바뀌면 `feat!`/`fix!`. 작성자 `tunarider <tunarider@tunaground.net>`.
- **TDD**: 항목마다 시험을 먼저 쓰고 실패를 확인한다. 언어 동작은 `tests/lang/*.fung`의 `# expect:`·`# expect-error:`, 내부는 해당 `*_tests.rs`. 재현은 점검 문서의 것을 그대로 시험으로 옮긴다.
- **검사**: 묶음마다 `cargo fmt --check`와 해당 크레이트 시험. 커밋 직전에는 CLAUDE.md의 전체 검사(clippy 두 번, `cargo test` 두 번, `cargo deny`·`cargo audit`).
  전체 검사는 시간이 들므로 묶음 커밋마다 하되, 실패하면 다음 묶음으로 넘어가지 않는다.
- **에러 번호**: 새 번호는 `crates/fung-front/src/diag/catalog.rs`에서 붙이고 `FUNG_UPDATE_SPEC=1 cargo test spec_file`. 은퇴 번호(E0705·E0231)는 표에 은퇴로 남긴다. 이미 붙은 번호의 뜻은 바꾸지 않는다.
- **저장소의 `.fung`**: D2·D5·D6·D11·D12·D14·D18·D21·D29·D32는 `extras/`·`examples/`·`tests/lang/`·매뉴얼 펜스의 코드를 깨뜨릴 수 있다. 묶음마다 `grep`으로 쓰는 곳을 먼저 찾아 같은 커밋에서 고친다.
  특히 D11(음수 기간)은 `extras/http.fung`·`extras/conc.fung`의 남은 시간 계산, D7(거꾸로 된 범위)은 `0..len(x) - 1`·`a..b` 꼴 계산을 모두 본다.
- **기대 출력**: `.fung`·출력이 바뀌면 `FUNG_EXPECTED=bless …`로 다시 쓰고 diff를 검토한다. 사례 수 상수(`DUMP_CASES`·`FMT_INPUTS` 등)를 함께 고친다.
- **매뉴얼**: 문법·내장 함수·내부 구조가 바뀌는 묶음은 원고를 고치고 `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`로 다시 만든 뒤 diff를 검토한다.
- **CHANGELOG**: 사용자에게 보이는 변화는 묶음마다 `[Unreleased]`에(호환 깨짐은 "호환 깨짐" 아래).
- **점검 문서**: 항목을 고칠 때마다 점검 문서 해당 절에 `처리:` 줄(커밋, 바꾼 문서, 결정 번호).
- **성능 항목**: 고치기 전에 기준을 재고(같은 바이너리를 번갈아 9회, CPU 시간 중앙값; 점검 문서 0절), 고친 뒤 같은 측정을 `docs/notes/bench-languages.md`에 기록한다.

## 3. 작업 순서

의존 관계로 정한 순서다. 앞 묶음이 뒤 묶음의 전제를 만든다.

| 순서 | 묶음 | 항목 | 이유 |
|---|---|---|---|
| W0 | 준비 | — | 성능 기준 측정, 쓰는 곳 목록 |
| W1 | 동시성 정확성 | CC9 CC10 CC12 CC11 CC13 | 높음 하나 포함, 다른 묶음과 파일이 겹치지 않음 |
| W2 | 컴파일러·VM 정확성과 정리 | VM14 VM17 VM18 | VM17 정리가 W10의 VM16을 한 곳으로 만든다 |
| W3 | 앞단: 새 컴파일 에러 | FE24 FE25 FE23 FE38 SP12(컴파일 쪽) | 언어 규칙. 뒤 묶음이 이 규칙 아래서 `.fung`을 고치도록 먼저 |
| W4 | 앞단: 괄호·범위·패턴·리터럴 | FE27 SP19(파서) FE32 FE31 SP27(리터럴) | 파서 구조 변경을 한 번에 |
| W5 | 앞단: 진단·로더·포매터·성능 | FE26 FE28 FE29 FE30 FE33 FE34 FE35 FE36 FE37 FE39 | 동작 변화 작음 |
| W6 | 결과값 규칙(VM·prelude) | SP12(런타임) SP13 | W3의 컴파일 규칙과 짝 |
| W7 | 값·prelude | VA18 VA19 VA20 VA21 SP14 SP15(prelude) SP19(런타임) SP21 SP23 SP26 SP27(이름) VA22 VA23 VA24 VA25 VA26 VA27 | VA25(Rust로 옮김)가 VA18·VA19·SP23을 함께 푼다 |
| W8 | fung으로 쓴 모듈의 인자 | SP18 SP15(`conc`) LB38 | `extras/cli.fung`·`conc.fung`·`fs.fung` |
| W9 | fs·프로세스·시간 | SP16 LB28 LB29 LB35 LB36 SP25 LB37 | `native/fs.rs`·`process.rs`·`time*.rs` |
| W10 | HTTP·TLS | LB23 LB24 LB25 LB26 LB27 LB30 LB31 LB32 LB33 LB34 LB39 | `extras/http.fung`·`native/tls.rs`·`net.rs` |
| W11 | CLI | TL6 TL7 TL8 | `crates/fung-cli` |
| W12 | VM 성능 | VM15 VM16 | 측정이 필요하고 앞의 정확성 변경 뒤에 재는 것이 맞다 |
| W13 | 문서와 카탈로그 | SP17 SP20 SP22 SP24 LB40, 설계 문서 머리말(D34), CLAUDE.md의 `agent` | 동작이 정해진 뒤 한 번에 |
| W14 | 마무리 | — | 5절 |

## 4. 묶음마다 할 일

각 항목의 원인 위치와 재현은 점검 문서. 여기서는 고칠 곳, 시험, 함께 바꿀 문서만 적는다.

### 4.0 W0 준비

- `cargo build --release --locked` 뒤 기준 측정: `fib`(점검 문서 VM16의 `fib35.fung`), 내장 호출 루프(VM15의 `len` 루프), `bench/list.fung`, VA22~VA26의 측정 프로그램. 결과를 이 문서 끝 "측정 기록"에 적는다.
- 깨질 코드 목록을 `grep`으로 만든다: `valid_utf8`, `env()`, `fs.exists`, `cli.parse`, `take(`/`drop(`의 문자열 인자, `range(`, `split(… "")`, `?`가 `nil`에 닿는 곳, 음수 기간이 될 수 있는 계산, `Ok(`/`Err(`의 인자 둘 이상, 대문자 `0X`·`E` 리터럴.

### 4.1 W1 동시성 정확성

- **CC9 (D1)**: `vm/scope.rs` `ScopeState::lose`·`Scope::closed`에서 범위 노드가 취소되었고 `failure`가 없으면 `Unread`를 버린다. `vm/task.rs` `TaskHandle::drop` 경로 확인.
  시험: 점검 문서의 재현 다섯(`scope`·`supervise`·`cancel(s)`·`pmap`·기한 뒤 `Err`)이 `Err timeout`/`Err cancelled`·종료 코드 0, 그리고 취소 없는 E0607 세 경우(05:117~119)와 `cancel(t)`만 끊은 경우는 그대로 E0607. `fung test`의 순서 섞기에서도 같은 결과(실행 순서와 무관).
  문서: 05의 E0607 절에 "취소로 되감기는 범위의 `Err`는 세지 않는다", 매뉴얼 동시성 레퍼런스.
- **CC10**: 표준 입력·파일 핸들의 읽기 에러(`EAGAIN`이 아닌 것, `EISDIR` 등)를 `Err('io)`로. 비차단 표준 입력은 이벤트 루프로 기다린다(또는 차단으로 되돌림; 점검 문서의 고치는 법). `vm/stdin.rs`·`native/file.rs`.
- **CC12**: `conc.rs` `wait_closed`의 기다림이 `Cont::receives_on`처럼 채널을 알리게 해 `vm/deadlock.rs` `own_sender`가 도움말을 붙이게.
- **CC11**: 05·VM 매뉴얼을 구현(`swap` 안에서는 차례를 넘기지 않음)에 맞추고, `block.rs` `end_turn`의 `pure > 0` 차가운 경로를 시계·잠금 없이(점검 문서 고치는 법). 긴 `swap`이 런타임을 세우는 것은 open-questions "알려진 한계"에 적는다.
- **CC13**: 스펙 `2026-10-18-concurrency-v2-r7-design.md`의 `timeout(d, f)` → `timeout(f, d)`, `channel.rs`·`stdin.rs` 머리 주석.

### 4.2 W2 컴파일러·VM 정확성과 정리

- **VM14**: `compiler.rs` `branches`가 앞 갈래들의 패턴 칸을 다음 갈래의 `held`에 넣어 놓기 대상으로 삼는다. 시험: 점검 문서의 `b2`(가드 거짓)·`b3`(패턴 중간 어긋남)가 교착 없이 `done`. `MOVELOCAL`·`CLEARLOCAL`이 너무 이른 경우가 없는지 기존 생존 시험 전체.
- **VM17**: `call.rs`의 `Params`/`Callee` 두 enum 합치기, `call_op`의 중복 블록 하나로, 쓰이지 않는 `RtProto::return_type` 삭제, `TAILCALL` 뒤 닿지 않는 `RETURN`을 내지 않기, 가드 붙은 이름 하나 패턴의 `MATCH`+`JUMPIFFALSE` 생략. `dump` 기대 출력이 바뀐다(bless, diff 검토).
- **VM18**: `compiler/fold.rs` `fold`가 이미 접은 자식을 다시 내려가지 않게(아래에서 위로 한 번). 시험: `x + x + …` 16000항의 컴파일 시간 상한(perf 시험, 릴리스 전용).

### 4.3 W3 앞단: 새 컴파일 에러

- **FE24 (D5)**: `resolve/value.rs` `check`의 `used` 계산에 "버려지는 자리의 효과 없는 식" 판정을 더한다. 새 번호. 시험: 줄 앞 `- 2`, 괄호 잊은 `cleanup`, `print(1, _)`, `x == 1`, 람다 문장, 그리고 통과해야 하는 것(호출·대입·`assert`·`_ = x`).
- **FE25 (D6)**: `resolve/pattern.rs`(또는 `compiler/m4.rs` `match_expr`)에 도달성 판정. 새 번호. 맨 이름이 바깥 이름과 같을 때 도움말. `=>` 뒤 람다의 E0112 도움말. `match.md` 경계 사례.
- **FE23 (D4)**: `resolve.rs` `own_bindings`가 안쪽 `Do`로 내려가지 않게. `for`·`match` 묶음도 같은 규칙인지 확인. REPL `redefine` 모드에서 어떻게 되는지 시험 하나(점검 문서 12절의 미확인).
- **FE38 (D8)**: 기본값 안의 `return`은 E0211 확장, test 블록은 허용. `testing.md`.
- **SP12 컴파일 쪽 (D2)**: `Ok`·`Err`의 인자 개수 검사(식·패턴·파이프, E0117 확장 또는 새 번호). 타입 표기 `Ok`·`Err`는 `value/types.rs` `matches_type`에서 인자 1개 태그만.

### 4.4 W4 앞단: 괄호·범위·패턴·리터럴

- **FE27·SP19 파서 (D7)**: `parser/collections.rs` `paren_or_tuple_inner`가 괄호를 표시로 남긴다(노드 플래그 또는 `Head::Paren`). `desugar`의 파이프 변환이 그 표시를 보고, `parser/expr.rs` `index_or_slice_inner`는 `[` 바로 안의 최상위 `..`만 슬라이스로. 슬라이스 끝 자리에 `??`·`|>`. 포매터가 괄호를 지키는지 왕복 시험. `grammar.md`·`operators.md`의 파이프 규칙 문장.
- **FE32 (D10)**: `parser/stmt.rs` `statement`의 `try_destructure`, `for_expr`, 매개변수 패턴을 같은 `destructure` 함수로. `parser/pattern.rs` `pat_term`의 `-` 뒤 기간. `grammar.md`.
- **FE31 (D9)**: `lexer/number.rs` `prefixed_int`의 최소값 특례, `duration`의 성분 규칙과 0 반올림 검사, 실수 아래 넘침. `lexical.md` 경계 사례.
- **SP27 리터럴 (D32)**: 대문자 `0X`·`0O`·`0B`·`E`는 E0102(도움말 소문자). 저장소의 대문자 리터럴을 먼저 찾는다.

### 4.5 W5 앞단: 진단·로더·포매터·성능

- **FE26**: 닫히지 않은 괄호·빠진 쉼표에 여는 위치(`note: opened here`)와 기대한 토큰. `'hello'`·`"${}"`·`\u{110000}`의 문구. `parser.rs` `unexpected`를 부르는 곳들, `lexer/string.rs` `escape`.
- **FE28**: E0224 패턴 대입의 문구·밑줄. **FE29**: 모듈 읽기 실패를 "cannot read"와 "cannot find"로 나누고 경로 표시 하나로, E0306 위치를 이름으로. **FE30**: 로더 `is_name`을 렉서의 판정으로(예약어 거절).
- **FE33**: 블록 밖 이름 진단을 묶는 방법(블록·`for`·`match`)마다 같게, 도움말 고정 문자열 고침. **FE34**: 원소 타입을 문자열 대신 파서가 만든 트리로 검사(`parser/fn_.rs` `type_term`, `resolve/alias.rs`). **FE35**: `format/spacing.rs` `prefix_context`에서 `true`·`false`·`nil`·`;;` 뒤는 이항 `-`.
- **FE36**: "비슷한 이름" 후보를 한 번만 모아 정렬해 두고 재사용. **FE37**: `desugar.rs` `fn_node`가 필요할 때만 `names_in`, 결과는 집합. **FE39**: `loader.rs`의 `surface`·`core` 복사를 `dump`일 때만. 각각 측정 전후 기록.

### 4.6 W6 결과값 규칙

- **SP12 런타임 (D2)**: `value/format.rs` `result_tag` 삭제, 보간·`join`이 `vm/m4.rs` `result_kind`를 쓴다. 펼치기 등 런타임에 개수가 틀린 `Ok`/`Err` 생성은 런타임 에러. `extras/prelude.fung` `is_ok`·`is_err`, `conc.retry`가 같은 판정. 02·`results.md`·E0605 문구에서 "보통 태그 `Ok`" 삭제.
- **SP13 (D3)**: `vm/m4.rs`의 `?` 처리에서 `ResultKind::Nil` 통과 갈래를 E0604로. 02·`values.md`·`results.md` 표의 nil 행.

### 4.7 W7 값·prelude

- **VA25 먼저**: `reverse`·`keys`·`values`·`sum`·`min`·`max`를 Rust로(`prelude/collections.rs`), 조각 실행(`vm/pieces.rs` `Work`) 규칙을 지킨다. 그러면 VA18(D16: `sum`·`min`·`max`·`sort`의 원소 하나 검사)과 VA19의 문구가 함께 풀린다.
- **VA19 (D17)**: 호출 경로 조립에서 `Builtin` 프레임 감추기, 남은 fung prelude 함수의 문구.
- **VA20 (D18)**, **VA21 (D19)**: `strings.rs` `split_fn`, `value/compare.rs` `equal_leaf`·`hash_leaf`.
- **SP14 (D12)**: `prelude/convert.rs` `parse_int`·`parse_float`·`parse_dur_fn`과 `int`·`float`의 문자열 갈래를 렉서의 숫자 규칙과 같은 함수로(가능하면 `fung-front`의 숫자 판정을 공유하지 않고 같은 규칙의 시험 표로 지킨다). 내부 호출자(`cli` 등)가 trim에 기대는지 확인.
- **SP15 prelude (D11)**: `extras/prelude.fung` `_clamp`·`_padding`·`iterate`, `strings.rs` `repeat_fn`, `sleep`·`timeout`의 음수 기간. `deadline.rs`의 문구.
- **SP19 런타임 (D7)**: 범위 값 만들 때 거꾸로면 에러, `value/index.rs`가 범위 값 색인을 받는다.
- **SP21 (D13)**: `value/index.rs`의 맵 갈래, `nat.rs` `need_key`, `collections.rs` `get_fn` 등을 E0514로.
- **SP23 (D14)**: `take`·`drop`의 `'str` 갈래 삭제(저장소 쓰는 곳은 슬라이스로).
- **SP26 (D15)**: `sym_name` 더하기(`spec/builtins.txt` 재생성, 레퍼런스).
- **SP27 이름 (D32)**: `valid_utf8` → `is_utf8`, 매개변수 이름 `file`·`pattern`·`x`, `env()` → `env_all()`, `range()` 삭제(저장소 쓰는 곳은 리터럴로, `spec/builtins.txt` 재생성). 레퍼런스 매개변수 표(`manual.rs`가 지킴).
- **VA22**: `==`의 E0517 검사와 비교를 한 걸음으로. **VA23**: `sort`에 달리기 검출(정렬된 입력 O(n)). **VA24**: `sort_by`의 `xs.clone()` 제거, `unique`의 두 벌. **VA26**: 한도 검사가 `Text`의 기억된 글자 수를 쓴다. **VA27**: `nat::str_args` 삭제, `args` 정의 하나로.

### 4.8 W8 fung으로 쓴 모듈의 인자

- **SP18**: `extras/cli.fung`·`conc.fung`·`fs.fung`의 공개 함수에 타입 표기와 E0403 검사(부작용 전에). `with_open`은 열기 전에 `f`를 검사.
- **SP15 conc (D11)**: `pmap`의 `limit` 기본값 `nil`, 음수·0은 E0403, `retry`의 `times`·`delay`.
- **LB38 (D29)**: `cli.parse`가 `Ok({flags, rest})`/`Err`, 반복 플래그 실패, 스위치 `=true|false`만. 저장소의 `cli.parse` 호출에 `!`. `cli.md`.
- 시험: extras의 공개 함수마다 틀린 인자 한 사례(점검 문서 1절 "다섯째"의 표). 표는 `crates/fung-cli/tests/`에 기대 출력으로.

### 4.9 W9 fs·프로세스·시간

- **SP16 (D21)**: `native/fs.rs` `exists_fn`을 `Ok(bool)`로. 저장소 쓰는 곳에 `!`. `fs.md`.
- **LB28 (D26)**: `native/process.rs` `CommandWait`의 끝: `code: nil`, `signal`, 문구. `exec`·`sh` 레퍼런스.
- **LB29 (D22)**: 읽는 길(`fs.rs` `read_fn`, `process.rs`의 출력 모으기, 표준 입력, 소켓 `read_all`)에서 한도를 넘으면 `Err`, `exec`는 읽는 중에 판정해 명령을 끝낸다. 메모리 시험(300 MB 출력에서 RSS가 한도 근처에서 멈춤).
- **LB35**: `cwd` 문구에 디렉터리, `env`를 줄 때 `PWD`를 `cwd`로, `cwd: ""`는 E0403.
- **LB36·SP25 (D28)**: `native/timefmt.rs` `names_a_date` 자리에서 필드 중복 검사, `ddd` 대조, `format` 쪽 토큰 없는 형식·`%` 거절.
- **LB37**: 지역 시간대로 본 연도가 0000~9999 밖이면 `Err`(또는 표시 범위 검사; 점검 문서의 고치는 법).

### 4.10 W10 HTTP·TLS

- **LB23**: `native/tls.rs` `fill`이 레코드 사이의 `UnexpectedEof`를 에러로, "닫힘까지" 본문은 close_notify가 있어야 `Ok`. 시험: close_notify 없는 끝 → `Err`.
- **LB24 (D23)**: 자동 `Referer` 삭제(`_hop_headers`).
- **LB25**: `_parse_proxy`가 읽지 못한 프록시 값을 `Err`로(직접 연결로 바꾸지 않음).
- **LB26 (D24)**·**LB27 (D25)**: `header_lines`·`target`·`query_pairs`, 잘못된 쿼리 이스케이프 400. `http.md`·06.
- **LB30**: 사용자 `Proxy-Authorization`은 프록시에만 한 번, 터널에서는 `CONNECT`에만. **LB31**: `_fix_pragma`·`_close_header` 삭제(받은 그대로). **LB32**: absolute-form의 호스트를 `req.host`로, 빈 경로 `"/"`. **LB33**: HTTP/1.0 응답의 `Transfer-Encoding`은 서버와 같은 규칙(거절).
- **LB34 (D27)**: `request`의 `upper(method)` 삭제, 빈 메서드·대소문자 중복 헤더 E0403.
- **LB39**: `_native.net.set_deadline`의 `0s` 특례 삭제.

### 4.11 W11 CLI

- **TL6 (D30)**: `main.rs` `read_error`, `fmt.rs` `run_fmt`, `test.rs` `test_file`의 종료 코드. 08 표.
- **TL7**: 모든 명령이 같은 옵션 파서(`--help`·`--version`·`--`·남는 인자), `fmt - -` 거절, REPL `:` 명령의 남는 인자 거절과 `error:` 머리.
- **TL8 (D31)**: `format.rs` `format_bytes`·`lex_bytes` 정리, `fmt`가 `decode_source`.

### 4.12 W12 VM 성능

- **VM15 (D20)**: `value.rs` `NativeFn`·`StepFn` 서명을 새 인자 타입으로, `call.rs` `Task::take_args`. 측정: W0의 내장 호출 루프, `bench/list.fung`.
- **VM16**: (a) 링크 때 명령 미리 해독(`bytecode/op.rs` `Instr::op` 호출 제거), (b) 비꼬리 호출의 `Vec::remove(callee_at)` 없애기, (c) 명령마다 위치 읽기를 에러 때로 미루기. 측정: `fib`(기대 약 −20%), 호출 위주 루프. 매뉴얼 VM 문서(`08-bytecode.md` 73행 등) 고침.

### 4.13 W13 문서와 카탈로그

- **SP17 (D34)**: modules.md·functions.md·scope.md·match.md·lexical.md·types.md·`grammar.md`에 빠진 규칙과 경계 사례 펜스. 설계 문서 00~08 머리에 "규범은 매뉴얼" 한 줄.
- **SP20**: `regex.md`에 "`.`·문자 클래스는 UTF-8이 아닌 바이트에 맞지 않는다(엔진의 유니코드 모드)".
- **SP22**: 03·06·08·`grammar.md`의 낡은 예·표, 06의 `fsync` 문장과 비용, CLAUDE.md의 `agent` 모듈 언급 삭제.
- **SP24 (D33)**: 카탈로그 은퇴·문구, `spec/errors.txt` 재생성, `errors.md`.
- **LB40**: `fs.md` 27행·`io.md` 223행의 `'invalid`.

## 5. 마무리 (W14)

- CHANGELOG `[Unreleased]`의 "호환 깨짐" 정리(결정 D2·D3·D4·D5·D6·D7·D9·D10·D11·D12·D14·D18·D21·D22·D23·D25·D26·D27·D28·D29·D30·D31·D32·D33).
- 매뉴얼 다시 만들기와 diff 검토, 기대 출력 bless와 diff 검토.
- 전체 검사(CLAUDE.md "커밋 전 검사") 두 판 모두.
- 점검 문서 74개 항목 모두 `처리:` 줄 확인, 이 문서의 결정과 다른 항목이 없는지.
- open-questions: 새 알려진 한계(CC11의 긴 `swap`), 닫힌 것 옮기기.
- CLAUDE.md "현재 상태"에 두 번째 점검 처리 줄.
- 측정 기록을 `docs/notes/bench-languages.md`에.
- main 스쿼시는 사용자 요청 때(그 뒤 브랜치를 main 위로 `rebase --onto`).

## 측정 기록

W0은 따로 하지 않고, 재는 작업이 바로 앞 커밋(또는 기준 `857db0f`)을 저장소 밖에 따로 빌드해 번갈아 쟀다(6절). 기계는 모두 Apple Silicon Mac, 표는 `docs/notes/bench-languages.md`.

- W1 CC11 (Task 1, 2026-11-10): ef61ef8과 고친 뒤를 번갈아 9번, CPU 시간 중앙값. 꼬리 호출 1,000만 번 `spin(0)` 407.5 → 410.4 ms(잡음 안), `swap` 안 476.5 → 407.4 ms, 기한 안의 `swap` 565.2 → 407.1 ms. 4.5절.
- W7a VA25 (Task 8, 2026-11-10): ec159a7과 고친 뒤를 번갈아 9번, 호출 앞뒤 `time.monotonic()` 차의 중앙값. 200만 원소 `reverse` 171.1 → 20.7 ms, `max` 142.2 → 17.1 ms, `min` 125.1 → 17.2 ms, `sum` 107.8 → 25.4 ms, `sum(0..2_000_000)` 100.7 → 0.001 ms, 100만 키 `keys` 238.7 → 19.6 ms, `values` 236.0 → 18.2 ms. 4.6절.
- W7c VA22~VA24·VA26 (Task 10, 2026-11-10): 53ef633과 고친 뒤를 번갈아 5번, 같은 방법. `==` ×10 370.0 → 148.5 ms, `sort` 정렬된 입력 112.4 → 27.7 ms·무작위 정수 213.7 → 170.0 ms·무작위 문자열 722.2 → 702.3 ms(득 없음), `sort_by` 218.4 → 134.3 ms, 한글 4천만 자 `trim` ×20 1340 → 0.007 ms, 최대 상주 `sort_by` 201.3 → 155.9 MB·`unique` 290.2 → 257.9 MB. 4.7절.
- W5 FE36·FE37·FE39와 W2 VM18은 점검 문서 각 항목의 `처리:` 줄과 CHANGELOG "성능"에 적었다(시험 `perf.rs`가 선형을 지킴).
- W12 (Task 16, 2026-11-11): 857db0f·018e635·고친 뒤를 번갈아 7번, CPU 시간 중앙값(초). `fib35` 1.928·1.955 → 1.546, `len` 루프 3.056·3.060 → 2.253, 호출 루프 0.920·0.913 → 0.729, `bench/list.fung` 0.214·0.207 → 0.207(득 없음). 단계별 표는 `docs/notes/bench-languages.md` 4.8절.

## 6. 작업 단위 (Task)

3절의 묶음을 한 에이전트가 맡을 만한 크기로 나눈 것이다. 순서대로 하나씩 하고, 각 작업은 끝에 커밋 하나(필요하면 둘)를 남긴다.
모든 작업의 공통 규칙은 2절이고, 결정은 1절의 D 번호, 항목의 재현·원인 위치는 점검 문서(`docs/plans/2026-11-10-full-review.md`)의 해당 ID 절이다.
W0은 따로 하지 않는다: 쓰는 곳 찾기는 각 작업이 자기 결정에 대해 하고, 성능 기준은 측정하는 작업(Task 10·16)이 기준 커밋(`857db0f`)을 따로 빌드해 잰다.

### Task 1: W1 동시성 정확성
항목 CC9(D1) CC10 CC12 CC11 CC13. 할 일은 4.1.

### Task 2: W2 컴파일러·VM 정확성과 정리
항목 VM14 VM17 VM18. 할 일은 4.2.

### Task 3: W3a 효과 없는 문장과 도달할 수 없는 절
항목 FE24(D5) FE25(D6). 할 일은 4.3의 해당 줄. 저장소의 `.fung`(extras·examples·tests/lang·매뉴얼 펜스)에서 새 에러에 걸리는 곳을 모두 고친다.

### Task 4: W3b 블록 안 대입, `return`, `Ok`/`Err` 인자 개수(컴파일)
항목 FE23(D4) FE38(D8) SP12의 컴파일 쪽(D2: 식·패턴·파이프의 인자 개수, 타입 표기). 할 일은 4.3의 해당 줄.

### Task 5: W4 괄호·범위·패턴·리터럴(앞단)
항목 FE27·SP19의 파서 쪽(D7) FE32(D10) FE31(D9) SP27의 리터럴(D32 첫 문장). 할 일은 4.4. 범위 값의 런타임 규칙(거꾸로 된 범위 에러, `xs[r]`)은 Task 9.

### Task 6: W5 앞단 진단·로더·포매터·성능
항목 FE26 FE28 FE29 FE30 FE33 FE34 FE35 FE36 FE37 FE39. 할 일은 4.5.

### Task 7: W6 결과값 규칙(런타임)
항목 SP12의 런타임 쪽(D2) SP13(D3). 할 일은 4.6. Task 4가 컴파일 쪽을 이미 했다.

### Task 8: W7a prelude를 Rust로, 이름 정리
항목 VA25 VA18(D16) VA19(D17) SP23(D14) SP26(D15) SP27의 이름(D32: `is_utf8`·`file`·`pattern`·`x`·`env_all`·`range()` 삭제). 할 일은 4.7의 해당 줄.

### Task 9: W7b 값 규칙
항목 VA20(D18) VA21(D19) SP14(D12) SP15의 prelude 쪽(D11, `sleep`·`timeout`의 음수 기간 포함) SP19의 런타임 쪽(D7) SP21(D13). 할 일은 4.7의 해당 줄. 음수 기간을 막으면 `extras/http.fung`·`conc.fung`의 남은 시간 계산을 모두 확인한다.

### Task 10: W7c 값 층 성능과 정리
항목 VA22 VA23 VA24 VA26 VA27. 할 일은 4.7의 해당 줄. 전후 측정을 `docs/notes/bench-languages.md`에.

### Task 11: W8 fung으로 쓴 모듈의 인자
항목 SP18 SP15의 `conc` 쪽(D11) LB38(D29). 할 일은 4.8.

### Task 12: W9 fs·프로세스·시간
항목 SP16(D21) LB28(D26) LB29(D22) LB35 LB36·SP25(D28) LB37. 할 일은 4.9.

### Task 13: W10a TLS·리다이렉트·프록시
항목 LB23 LB24(D23) LB25 LB30 LB39. 할 일은 4.10의 해당 줄.

### Task 14: W10b HTTP 요청·응답 모양
항목 LB26(D24) LB27(D25) LB31 LB32 LB33 LB34(D27). 할 일은 4.10의 해당 줄.

### Task 15: W11 CLI
항목 TL6(D30) TL7 TL8(D31). 할 일은 4.11.

### Task 16: W12 VM 성능
항목 VM15(D20) VM16. 할 일은 4.12. 기준 바이너리는 `857db0f`를 따로 빌드해 같은 방법으로 번갈아 잰다.

### Task 17: W13 문서와 카탈로그
항목 SP17(D34) SP20 SP22 SP24(D33) LB40, 설계 문서 머리말, CLAUDE.md의 `agent`. 할 일은 4.13.

### Task 18: W14 마무리
5절. 점검 문서 74개 항목의 `처리:` 줄을 모두 확인하고 빠진 것을 채운다.
