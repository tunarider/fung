# 미결 사항

구현 중 반드시 부딪히지만 아직 정하지 않은 세부 사항, 남은 일, 알려진 한계를 여기에 적는다.
구현하다가 해당 지점에 닿으면 **추천안과 함께 사용자에게 묻고** 결정 후 해당 섹션 문서를 갱신한다.

이 문서에는 **열린 항목과 지금도 유효한 결정만** 둔다. 닫은 항목은 해당 설계 문서와 노트로 옮긴다.
닫은 항목의 기록: 0.4.0까지(2026-10-24 정리) `docs/notes/open-questions-closed.md`, 그 전의 이식 기록 `docs/notes/R-port-record.md`.
(2026-10-12~13 전체 재점검의 결정: 람다 표기·`_` 범위·숫자·문자열·식별자 → 01, 맵 사본·튜플 인덱스·`?`와 nil·`Err` 내용·순회 대상·함수 `==` → 02,
`defer` → 03, 여러 채널 기다리기 → 05, `print`·`args`·`exit`·표준 입력 → 06, 0으로 나누기 → 07.)

## 알파에서 써 보며 조정할 것

- [ ] prelude와 모듈 함수 목록 (06). 실제 스크립트 2~3개를 옮겨 보고 더하거나 뺀다.
  - M5 `examples/rename.fung`에서 나온 후보: 부수효과 순회의 블록 문법(`each`에 `fn(): … ;;)`가 거슬림 — 보류, 더 자주 거슬리면 재검토).
- [ ] 튜플 원소 접근: 매개변수 패턴으로 대부분 해소되어 보류. 알파 스크립트에서 "튜플 변수의 한 원소"가 자주 나오면 `fst`/`snd` 재검토.

## M8 후속 (보류, 2026-10-14)

- [ ] `time`: 12시간제 토큰(`hh`·`A`), 토큰 이스케이프, 달력 산술(`add_months` 등).
- [ ] `http`: 응답 스트리밍·대용량 본문, 쿠키, TLS 서버.
- [ ] `regex`: 외부 입력 패턴을 `Err`로 검사하는 함수(`regex.compile`류).

## 배포 후속 (보류, 2026-10-14)

- [ ] Windows 지원: 빌드는 되지만 REPL raw 모드 편집기를 확인하지 않았다. 확인 후 릴리스 대상(`scripts/build-release.sh`)에 추가하고 설치는 zip + PowerShell.
- [ ] Homebrew tap: macOS에서 `brew upgrade`로 업데이트. 그때 릴리스 도구를 바꿀지 함께 정한다.
- [ ] `fung upgrade` 자체 업데이트 명령: 지금은 설치 스크립트를 다시 실행하면 된다.
- [ ] macOS 코드 서명·공증: 브라우저로 받은 바이너리는 Gatekeeper가 막는다(`curl` 설치는 해당 없음). Apple 개발자 계정이 필요하다.

## 매뉴얼 (2026-10-20 설계, `docs/superpowers/specs/2026-10-20-manual-structure-design.md`)

1~5단계(뼈대·레퍼런스·배우기·예제 모음·VM)는 0.4.0에 들어갔다.

- [ ] 후속: VM 부록(`manual/vm/internals.md`)의 손으로 적은 `dump` 출력을 검사되는 `dump` 펜스로. 언어 명세(`reference/language.md`)를 PHP식 레퍼런스와 같은 깊이로 다듬기.
- [ ] 보류: 매뉴얼 전문 검색(지금은 함수 필터만).
- [x] 매뉴얼 후속(2026-10-20 최종 검토의 사소한 것): 2026-10-26에 정리했다(0.5.0 목록 S15). 장 끝 구성과 4장은 0.5.0 문서 L18.
- [ ] 매뉴얼을 쓰며 찾은 언어·도구의 문제: `docs/plans/2026-10-20-manual-findings.md`. 결정을 기다리는 것은 아래 "남은 일"에 모았다.

## 성능

측정은 `bench/`(방법과 기준선 `bench/README.md`), 동시성 런타임의 수치는 `docs/notes/R7-scheduler.md` "R단계 전체 요약".
2026-10-14의 첫 측정과 1~3단계 최적화는 `docs/notes/open-questions-closed.md`.

- [ ] 레지스터 기반 VM, 인라인 캐시: 측정을 보고 정한다(명령 해석 루프가 남은 시간의 대부분, 2026-10-14 2단계 결과).
- [ ] JIT: 하지 않는다(추천). 실행 가능한 메모리 관리, 대상 아키텍처마다의 코드 생성, GC·스케줄러와의 상호작용 비용이 용도(입출력 위주 스크립트)에 비해 크다.
- [ ] 배포용 단일 실행 파일(`fung build app.fung -o app`, fung 바이너리 + 소스 묶음): 바이트코드 파일(.fungc)보다 추천. 컴파일은 10ms 미만이라 바이트코드 캐시의 속도 이득은 없다.

### 2026-10-21 다른 언어 비교와 다음 할 일 (P8~P13)

Python·Lua·Ruby·Node와 13개 작업을 견줬다(프로그램과 스크립트 `bench/compare/`, 기록과 원인 분석 `docs/notes/bench-languages.md`).
fung 시간 / 상대 시간의 기하평균: 기본 계산은 Python 1.5배·Ruby 2.6 1.3배·Lua 2.0배·Node 2.3배 느리고, 텍스트(로그 집계·정규식·JSON)는
Python 2.3배·Node 3.5배 느리다. 시작(6ms)·외부 명령·동시성(채널은 Python asyncio의 55배 빠름)은 상위권이다.
약점이 스크립트의 핵심 작업(파일·텍스트)에 몰려 있어 아래 순서로 한다. 번호는 2026-10-20 검토의 P1~P7에 이어 붙였다.
항목마다 시작할 때 `docs/plans/`에 계획을 쓰고, 끝나면 `bench/compare/run.py`와 `scripts/bench.sh <전> <후>`로 다시 재 이 절과 노트를 갱신한다.

- [x] **P8** 파일 흘려 읽기와 짧은 `fs` 호출의 풀 왕복. 파일 핸들의 `read_line`과 `exists`·`stat`이 호출마다 스레드 풀을 왕복해
  건당 약 6~8µs가 든다(20만 줄 `read_line` 1.26초, 통째 읽기 + `lines` 0.086초; `fs` 5만 번 Python의 5배).
  방향: 풀에서 큰 덩어리를 읽어 와 줄은 실행 스레드의 버퍼에서 바로 내주기(덩어리가 바닥날 때만 왕복).
  **결정 필요**: 로컬 경로의 `exists`·`stat`을 실행 스레드에서 바로 할지(빠르지만 느린 네트워크 파일 시스템이면 모든 태스크가 멈춤) — 시작 때 선택지로 묻는다.
  목표: 한 줄씩 읽기가 통째 읽기 + `lines`의 2배 안, `fs` 프로그램이 Python의 2배 안, 로그 처리 메모리가 파일 크기의 2배 안(지금 6배).
  - [x] 핸들의 `read_line`·`fs.write_line` 빠른 길 → 완료 (2026-10-21): 버퍼에 줄이 다 있으면(쓰기는 들어가면) 실행 스레드에서 바로 한다.
    20만 줄 읽기 1.23→0.093초, 쓰기 1.24→0.118초. 흘려 읽는 로그 집계가 통째 읽기와 같은 0.45초에 메모리 9MB(통째 읽기 92MB).
  - [x] `exists`·`stat` → 완료 (2026-10-25, 사용자 결정 B): 태스크가 혼자일 때(차례나 바깥을 기다리는 다른 태스크가 없고 자기 기한이 없음)만
    실행 스레드에서 바로 부른다(`nat::offload_short`). 5만 번 0.404→0.061초, 비교의 `fs` 0.447→0.082초(Python 0.084). 함수를 막히는 것과 아닌 것으로
    나누는 안은 기각. 계획 `docs/plans/2026-10-25-p8-short-fs-calls.md`. 바꾸는 호출(`mkdir`·`remove` 등)은 측정 근거가 생기면 따로 본다.
- [x] **P9** 맵 누적의 transient(Clojure): 아무도 중간 결과를 볼 수 없는 구간(`fold`로 쌓는 중)에서는 내부적으로 제자리 수정. 목표는 Python의 2배 안쪽.
  2026-10-21 측정에서 맵 20만 개가 Python의 3.4배, 키 4개 맵 갱신도 건당 약 0.6µs라 우선순위를 올렸다.
  → 2026-10-25 완료(사용자 결정 B): 제자리 수정 장치(`Rc::make_mut`)는 이미 있었고, 지역 변수가 문장 끝까지 사본을 쥐어 참조 수가 늘 2였던 것이 원인.
  지역 변수의 마지막 읽기가 값을 넘긴다(`MOVELOCAL`, 03 "값의 수명"). 비교의 `map` 0.380 → 0.193초(Python 0.110, 1.75배). 계획 `docs/plans/2026-10-25-p9-move-at-last-read.md`.
- [x] **P10** 텍스트 경로의 prelude 함수: `lines`(날 `split`의 2배, 줄마다 `_chomp` 람다)를 네이티브로 옮길지 측정하고 정한다.
  줄 하나를 단어로 나누는 `split`(줄당 약 0.65µs)도 같이 본다.
  → 2026-10-25 완료(사용자 결정: 네이티브): `lines` 0.080 → 0.034초, 비교의 `text` 0.375 → 0.336·`regex` 0.307 → 0.265초. 계획 `docs/plans/2026-10-25-p10-native-lines.md`.
  `split`의 남은 비용은 조각마다의 할당 두 번(`Rc<String>`: `Rc` 상자와 문자열 버퍼)과 네이티브 호출마다의 인자 `Vec`이다. 값 표현(`Rc<str>`)이나 호출 규약을 바꾸는 일이라 따로 본다.
  - [x] `get` → 네이티브로 옮겼다(2026-10-24 보수 작업 F-1, 에러를 `put`처럼 E0506으로 맞추려고). 키 4개 맵의 `get`+`put` 20만 번이 0.11 → 0.08초(릴리스, M2 Max).
- [x] **P11** `regex.find`의 싼 길: 컴파일은 캐시되지만 맞을 때마다 맵 두 개와 리스트를 만들고 글자 위치를 센다(`is_match`의 4배).
  선택지(필드를 늦게 만들기, 그룹만 돌려주는 함수 추가 등)는 시작 때 묻는다. 새 함수면 매뉴얼·명세 갱신이 따른다.
  → 2026-10-25 완료(사용자 결정: API 그대로, 내부만): 결과 맵의 키와 빈 `named`를 실행 스레드마다 한 번 만들어 나눠 쓴다. 20만 줄의 `find`가 0.242 → 0.179초
  (같은 줄의 `is_match` 0.081), 비교의 `regex` 0.266 → 0.200초. 남은 몫은 엔진의 캡처 검색이다. 새 함수는 지금은 필요 없다. 계획 `docs/plans/2026-10-25-p11-cheaper-find.md`.
- [x] **P12** `fib` 퇴행: 0.3.0 0.203초 → 0.4.0 0.227초(약 11%). 원인과 측정은 `docs/notes/bench-languages.md` 3.4절과 3.4.1절.
  `4372c19`에서 해석 루프에 인라인으로 들어간 드문 에러 경로(`POP`의 E0606, `LOADGLOBAL`의 E0808·E0809)가 루프 전체의 코드 배치를 바꿨다.
  → 2026-10-24: 두 경로를 `#[cold] #[inline(never)]` 함수(`discarded_stop`·`unset_global`)로 빼고 `POP`의 흔한 길은 값싼 `is_err` 검사만 한다.
  같은 부하에서 0.3.0 0.208 · 고치기 전 0.232 · 고친 뒤 0.217초(퇴행의 약 절반). 나머지 갈래(`err`·`at`, `CHECKBOOL`)를 같은 식으로 빼거나
  `Instr::op`에 `#[inline]`을 붙이면 오히려 느려졌다(배치가 다시 흔들림). 해석 루프를 고치는 변경에는 `scripts/bench.sh <전> <후>`의 `fib`·`loop`를 붙인다.
- [ ] **P12b 릴리스 빌드의 LTO** (2026-10-24 측정, 사용자 결정: 지금은 넣지 않고 열어 둔다): 크레이트 경계를 넘어 함께 최적화한다.
  넣는다면 `[profile.release]`에 바로 넣는다(배포용 프로필 같은 예외는 두지 않는다, 사용자 결정 2026-10-24).
  - 실행(비율): `lto = "fat"`·`codegen-units = 1`이면 `fib` −7%, `loop` −8%, `map` −7%, `str` −10%, 실행 파일 6.5 → 5.0 MB.
    `lto = "thin"`은 `fib` −6%이지만 나머지는 −2% 안팎, 크기 그대로.
  - 대가(`cargo test --release --no-run`, 커밋 전 검사와 CI가 매번 하는 일): `vm.rs` 하나 고친 뒤 16초 → thin 54초·fat 70초, 처음부터 26초 → 63초·81초.
    시험 실행 파일 수십 개가 링크마다 다시 최적화되기 때문이다.
  - 판단: 입출력 위주의 스크립트에는 5~10%의 이득이 작고 커밋마다 40~55초가 는다. 같은 정도는 알고리즘 쪽(P9 등)에서 더 크게 얻는다.
    시험 실행 파일 수가 줄거나(예: 시험을 한 실행 파일로 모으기) 해석 루프의 속도가 급해지면 다시 잰다. 자세한 표는 `docs/notes/bench-languages.md` 3.4.1절.
- [x] **P13** 비교 측정 보강: Ruby 3.4 + YJIT로 다시 재기(지금은 macOS 시스템 Ruby 2.6), Linux(musl 정적 빌드)에서도 한 번 재기.
  → 2026-10-25: Ruby 4.0.9 `--yjit`(macOS)와 Docker의 arm64 Linux(glibc·musl 판, Ubuntu 꾸러미 언어들)로 쟀다. 표는 `docs/notes/bench-languages.md` 4.1절.
  `run.py`는 언어 명령을 바꿀 수 있고(`RUBY="ruby --yjit"`), Linux에서는 믿을 수 없는 메모리 열을 뺀다.
- [ ] **P14** Linux glibc 판이 계산에서 musl 판보다 크게 느린 이유(같은 커밋, Docker arm64: `fib` 0.323 대 0.218초, `loop` 0.428 대 0.275초). 배포 판은 musl이라 급하지 않다.

## Rust 구현: 정한 것과 남은 일

fung은 Rust로 구현한다. 문법과 사용자에게 보이는 동작이 핵심이고 내부는 바뀌어도 된다. C 구현은 기각: GC·동시성·TLS를 직접 만들어야 하고,
`http.serve`처럼 외부 입력을 파싱하는 코드에서 메모리 안전을 잃는다. 구현 독립으로 이어지는 것: 설계 문서, 에러 번호, 매뉴얼, `extras/`의 fung 코드,
골든 테스트(`tests/lang`), 기대 출력 파일, `bench/`, 릴리스 흐름.
이식의 기록(단계별 결정, 비교, 해결된 항목)은 `docs/notes/R-port-record.md`, 단계별 설명은 `docs/notes/R<n>-*.md`.

### 정한 것

- [x] 동시성과 메모리 모델 → **실행 스레드 하나가 모든 태스크를 돌린다(M:1, 스택 없는 그린 스레드), 값은 참조 계수.**
  (동시성 v2 R1b~R5, 2026-10-19; 노트 `docs/notes/R7-scheduler.md`. 3a의 "태스크마다 운영체제 스레드 + 실행권 잠금"은 대체됐다.)
  - 기다리는 태스크는 주차하고(`vm/block.rs`·`vm/sched.rs`), 계산만 하는 태스크는 호출 `YIELD_EVERY`(1,000)번마다 차례를 넘긴다.
    소켓·TLS·외부 명령은 kqueue/epoll 이벤트 루프(`vm/poller.rs`), 파일·이름 조회·정규식 컴파일은 스레드 풀(`vm/pool.rs`, 최대 64)이 맡는다.
    비용: 태스크 하나 약 1 µs, 채널 값 하나 약 0.5 µs, 풀로 보내는 파일 시스템 호출 하나 약 9 µs.
  - fung 값(`Rc`)은 실행 스레드에만 살고 타입이 지킨다(`Runtime`은 `!Send`, `World`는 `Runtime` 안의 `RefCell`). 다른 스레드는 값이 없는 `Shared`만 본다.
    손으로 쓴 `unsafe impl Send`·`Sync`는 두지 않는다(`repo_rust_only`가 지킴).
  - 값은 `enum`(종류 태그 + 값 칸, 16바이트). 정수·실수는 상자 없이 칸에 바로 들어간다. 긴 구조의 연쇄 해제는 재귀가 아니라 반복으로 한다.
    태스크 안은 원자적 연산 없는 `Rc`이고, 참조가 하나면 제자리 수정(Roc식, `Rc::make_mut`)도 가능하다.
  - 불변 값이라 태스크 사이의 "공유"와 "복사"를 프로그램이 구분할 수 없다. 여러 코어로 갈 때의 계획은 아래 "남은 일"의 "여러 코어".
  - 내장 함수의 기본값은 `Lit`이다.
- [x] 공유 상태 → **엄격한 `atom`** (동시성 v2 L5, 2026-10-18; 05 "공유 상태: atom"). 3b의 `agent`는 지웠다(기록은 `docs/notes/R3b-agent.md`).
  atom을 거치는 순환은 E0708이 막는다.
- [x] 채널 → **두 끝 `(tx, rx) = channel(n)`, 보내는 끝이 모두 사라지면 자동으로 닫힌다** (L4, 2026-10-18; 05).
  채널에 그 채널의 끝을 보내면 E0705 `channel sent into itself`(3b, L4). `send(tx, v)`가 `v`를 재귀 없이 깊이 훑는다. 훑지 않는 곳과 비용은 아래 "알려진 한계".
- [x] 라이브러리 구조 → **언어 핵심은 최소, 공식 라이브러리는 패키지 구조로 만들되 패키지 관리자가 생길 때까지 바이너리에 함께 넣는다.**
  런타임은 파일·프로세스·TCP 소켓·TLS·시간·정규식 같은 네이티브 원시 기능만 주고, HTTP 프로토콜·JSON·`cli` 등은 fung 라이브러리로 쓴다.
  - 내장 함수 배치 → **prelude에는 확실한 것만** (2026-10-17). 나중에 prelude에 더하는 것은 W0103 경고뿐이라 안전하고 빼는 것은 호환성 깨짐이다.
    정책(동시 실행 수, 재시도 횟수·간격)을 품은 `fan_in`·`pmap`·`retry`는 `conc` 모듈. `until`·`iterate`(`while`이 없는 언어의 반복), `exec`·`sh`(셸을 대신하는 언어의 핵심)는 prelude.
    `remove`·`close`와 `fs.remove`·`fs.close`의 이름 겹침은 유지하고 매뉴얼에 한 줄. 옮긴 이름을 그대로 쓰면 E0204에 `help: moved to module conc: …`(`resolve::MOVED_TO_MODULE`).
  - 구현 언어 지침(2026-10-15): "빠지면 다른 것을 만들기 힘들어지는 것"이 핵심이다. fung으로 쓸 수 없는 것(OS·외부 세계) → Rust.
    핵심이면서 람다를 부르지 않는 순수 계산(문자열, `len`, json 파싱, 정규식) → Rust. 핵심이면서 람다를 부르는 고차 함수(`map`·`fold`·`sort_by`·`each`) → Rust에 두고
    이어 가기 프레임으로 부른다(R2, 2026-10-18: 람다를 직접 부르지 않고 VM에 부탁하므로 태스크를 멈출 수 있고 Rust 스택을 쓰지 않는다. `docs/notes/R7-scheduler.md`). 핵심이 아닌 조합·프로토콜(`take`·`retry`·`pmap`·HTTP) → fung.
    구현 언어 이동은 사용자 코드에 영향이 없어 나중에 바꿔도 된다.
- [x] 의존성 정책 → **허용 목록.** 기본은 Rust 표준 라이브러리. 크레이트는 직접 짜면 위험한 것(암호·TLS)과 운영체제 접근에 `unsafe`가
  필요한 것(이벤트 루프·터미널·시그널)만, 이유를 여기 적고 CI에서 `cargo-deny`·`cargo-audit`로 검사한다. 공부 가치가 있는 것(HTTP, JSON, 시간 서식, `cli`)은 직접 짠다.
  - 들인 것: `libc`·`regex`(4c), `rustls`·`rustls-native-certs`(4d)와 그것들이 끌고 오는 것(아래 표). 이벤트 루프는 크레이트(`mio`) 없이 `libc`의 kqueue/epoll로 직접 짰다(R4a, `vm/poller.rs`).
  - 허용 목록 → **`deny.toml`의 `[bans] allow`가 유일한 원본** (4c, 2026-10-16). 직접 들인 것과 따라오는 것을 모두 적는다. 지키는 장치는 세 겹이다.
    - 저장소 테스트 `crates/fung-cli/tests/deps.rs`(표준 라이브러리만, 도구 없이 `cargo test`에서 돈다): `Cargo.lock`의 모든 패키지(작업 공간 크레이트 제외)가 목록 안인지,
      `crates/fung-front/Cargo.toml`에 의존성 표(`[dependencies]`·`[dev-dependencies]`·`[build-dependencies]`, `[target.*.]`·`[dependencies.이름]` 형태 포함)가 없는지,
      `fung-cli`의 의존성이 `path`뿐인지(테스트 TLS 서버용 dev-dependency `rustls` 하나만 예외).
    - CI `rust` 작업(Ubuntu만): `cargo-deny` 0.20.2·`cargo-audit` 0.22.2를 `cargo install --locked`로 버전 고정 설치하고 버전 문자열까지 확인한 뒤 `cargo deny check`·`cargo audit`.
    - `deny.toml`: 한 크레이트 여러 버전 금지, `*` 버전 금지(작업 공간 경로 의존성은 `publish = false` + `allow-wildcard-paths`), 라이선스는 실제로 필요한 것만(`MIT`·`Apache-2.0`·`ISC`·`BSD-3-Clause`), 출처는 crates.io만.
      **빈 `allow`는 cargo-deny에서 "모두 허용"**이고 비어 있지 않으면 작업 공간 크레이트도 막으므로, 목록에 `fung-front`·`fung-vm`·`fung-cli`를 늘 둔다.
    - **2주 규칙**: 크레이트와 점검 도구는 올라온 지 2주 넘은 버전만 쓴다(최신판이 2주 안 됐으면 `cargo update -p <이름> --precise <그 전 버전>`).
    - 크레이트는 `fung-vm`에만. `fung-front`(앞단)는 0개.

    | 크레이트 | 버전(올라온 날) | 라이선스 | 들인 길 | 왜 직접 짜지 않는가 | 끌고 오는 것 |
    |---|---|---|---|---|---|
    | `regex` | 1.13.1 (2026-07-15) | MIT OR Apache-2.0 | `fung-vm` 직접, 기본 기능 끄고 `std`·`perf`·`unicode`(문법이 이 크레이트의 것이고 유니코드 클래스를 쓴다; 릴리스 실행 파일 +490 KB, 약 9%) | 선형 시간 엔진(여러 엔진과 최적화)은 크고 틀리기 쉽다. RE2 계열이라 선형 시간·역참조 없음을 약속한다. 직접 구현(Thompson NFA)은 공부 과제로 남김 | `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr` |
    | `regex-syntax` | 0.8.11 (2026-06-09) | MIT OR Apache-2.0 | `fung-vm` 직접(`std`·`unicode`) + `regex`·`regex-automata`가 끌고 옴 | 잘못된 패턴의 에러 종류를 한 줄로 쓰려고 패턴을 먼저 파싱한다(`native/regex.rs`). 어차피 `regex`가 끌고 오는 크레이트라 새 코드가 늘지 않는다 | 없음 |
    | `regex-automata` | 0.4.18 (2026-08-04) | MIT OR Apache-2.0 | `regex`가 끌고 옴 | `regex`의 실제 엔진들 | `aho-corasick`, `memchr`, `regex-syntax` |
    | `aho-corasick` | 1.1.5 (2026-08-03) | Unlicense OR MIT | `regex`·`regex-automata`가 끌고 옴 | 여러 글자열 동시 찾기(패턴의 고정 글자열을 먼저 찾는 가속) | `memchr` |
    | `memchr` | 2.8.3 (2026-07-08) | Unlicense OR MIT | `regex`·`regex-automata`·`aho-corasick`가 끌고 옴 | 빠른 바이트 찾기 | 없음 |
    | `libc` | 0.2.189 (2026-07-21; 0.2.190은 2주 미만이라 고정) | MIT OR Apache-2.0 | `fung-vm` 직접, 유닉스 대상만 | `signal`·`raise`·`pthread_sigmask`(SIGPIPE로 죽기)는 표준 라이브러리에 없고, `sigset_t` 배치가 운영체제마다 달라 손으로 선언하면 틀리기 쉽다. 소켓(`native/sock.rs`: 비차단·close-on-exec·`SO_NOSIGPIPE`·`poll`·`getaddrinfo`)에도 쓴다 | 없음 |
    | `rustls` | 0.23.45 (2026-09-14; 공지 아래 참고) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접, 기본 기능 끄고 `ring`·`std`·`tls12` (+ `fung-cli` dev-dependency, 테스트 TLS 서버 `tests/common/tls_server.rs`) | TLS(암호와 프로토콜 상태 기계)는 직접 짜면 위험하다(의존성 정책). 소켓을 만지지 않는 상태 기계라 비차단 소켓과 이벤트 루프(`vm/poller.rs`) 위에서 돈다 | `ring`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `zeroize`, `once_cell` |
    | `rustls-native-certs` | 0.8.4 (2026-06-01; 0.8.5는 2주 미만) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접 | 운영체제 루트 인증서(macOS 키체인, Linux 인증서 파일)와 `SSL_CERT_FILE`·`SSL_CERT_DIR`을 읽는다. 운영체제마다 다른 저장소를 직접 읽는 것은 플랫폼 코드다 | `rustls-pki-types`, `openssl-probe`(macOS 밖 유닉스), `security-framework`(macOS), `schannel`(Windows) |
    | `rustls-pki-types` | 1.15.1 (2026-07-23) | MIT OR Apache-2.0 | `rustls`·`rustls-webpki`·`rustls-native-certs`가 끌고 옴 | 인증서·키·서버 이름 타입 | `zeroize` |
    | `rustls-webpki` | 0.103.16 (2026-10-20; 공지 RUSTSEC-2026-0382가 닿아 2주 규칙의 예외로 바로 올림, 아래 보안 공지 정책) | ISC | `rustls`가 끌고 옴 | 인증서 사슬·이름 검증(rustls의 WebPKI 검증기, `native/tls.rs`) | `ring`, `rustls-pki-types`, `untrusted` |
    | `ring` | 0.17.14 (2025-03-11) | Apache-2.0 AND ISC | `rustls`가 끌고 옴(암호 공급자로 고름) | 암호 기본 연산. C·어셈블리를 빌드하므로 C 컴파일러가 필요하다(CI 러너에 있음) | `cc`·`find-msvc-tools`·`shlex`(빌드 때만), `cfg-if`, `getrandom`, `untrusted`, `windows-sys`(Windows) |
    | `untrusted` | 0.9.0 (2021-07-13) | ISC | `ring`·`rustls-webpki`가 끌고 옴 | 믿을 수 없는 입력을 안전하게 읽는 도우미 | 없음 |
    | `subtle` | 2.6.1 (2024-06-24) | BSD-3-Clause | `rustls`가 끌고 옴 | 상수 시간 비교 | 없음 |
    | `zeroize` | 1.9.0 (2026-06-12; 1.9.1은 2주 미만) | Apache-2.0 OR MIT | `rustls`·`rustls-pki-types`가 끌고 옴 | 비밀 값을 메모리에서 지우기 | 없음 |
    | `once_cell` | 1.21.4 (2026-03-12) | MIT OR Apache-2.0 | `rustls`가 끌고 옴 | 한 번만 초기화하는 칸 | 없음 |
    | `getrandom` | 0.2.17 (2026-01-11) | MIT OR Apache-2.0 | `ring`이 끌고 옴 | 운영체제 난수 | `cfg-if`, `libc`, `wasi`(wasm) |
    | `cfg-if` | 1.0.5 (2026-09-16) | MIT OR Apache-2.0 | `ring`·`getrandom`이 끌고 옴 | 조건부 컴파일 매크로 | 없음 |
    | `cc`, `find-msvc-tools`, `shlex` | 1.5.1 (2026-09-25; 1.8.0은 2주 미만), 0.1.14 (2026-09-25), 2.0.1 (2026-05-17) | 모두 MIT OR Apache-2.0 | `ring`의 빌드 스크립트 | `ring`의 C·어셈블리 빌드(실행 파일에는 들어가지 않음) | 서로 |
    | `openssl-probe` | 0.2.1 (2026-01-20) | MIT OR Apache-2.0 | `rustls-native-certs`, macOS 밖 유닉스 | Linux 등에서 인증서 파일·디렉터리 위치 찾기 | 없음 |
    | `security-framework`, `security-framework-sys`, `core-foundation`, `core-foundation-sys`, `bitflags` | 3.7.0 (2026-02-20), 2.17.0 (2026-02-20), 0.10.1 (2025-05-26), 0.8.7 (2024-08-11), 2.13.2 (2026-09-10) | 모두 MIT OR Apache-2.0 | `rustls-native-certs`, macOS | macOS 키체인의 루트 인증서 읽기 | 서로, `libc` |
    | `schannel`, `wasi`, `windows-sys`, `windows-targets`, `windows_*` 8개 | 0.1.23 (2023-12-27; `windows-sys` 0.52 하나로 맞추려고 낮춤), 0.11.1, 0.52.0 / 0.52.6 | MIT / Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT / MIT OR Apache-2.0 | Windows·wasm 대상만 | `Cargo.lock`에는 있지만 fung의 대상(Linux·macOS)에서는 빌드하지 않는다 | — |

    `OR`로 묶인 라이선스는 하나만 허용되면 된다. `ring`(`Apache-2.0 AND ISC`, 둘 다 필요), `rustls-webpki`·`untrusted`(`ISC`), `subtle`(`BSD-3-Clause`) 때문에
    `MIT` 밖의 셋을 허용한다. `ring`의 라이선스 칸이 깨끗한 SPDX라 `[[licenses.clarify]]`는 필요 없다.
  - **보안 공지 정책** (사용자 결정, 2026-10-16): 공지가 우리 설정에 **닿지 않으면** 2주 규칙을 지키고 이유를 적어 무시하며 해제 날짜를 아래 "남은 일"에서 추적한다.
    **닿으면** 2주를 기다리지 않고 바로 올린다. 무시는 `deny.toml`의 `[advisories] ignore`(이유와 함께)와 `.cargo/audit.toml`(cargo-audit용, 같은 번호) 두 곳에 적는다.
    - 닿은 예: RUSTSEC-2026-0382(2026-10-20, `rustls-webpki` 0.103.15). emSign(eMudhra) 중간 인증서의 개인 키가 알려져, 그것을 거친 사슬을 믿을 수 있었다.
      fung의 https는 운영체제의 루트 인증서를 믿으므로(`rustls-native-certs`) 그 저장소에 emSign 루트가 있으면 닿는다. 2026-10-24 CI의 `cargo deny`가 잡았고 같은 날 0.103.16으로 올렸다(`rustls`는 0.23.45 그대로).
- [x] 정규식 문법 → **`regex` 크레이트의 문법이 fung 정규식 문법이다** (6c-4, 2026-10-18 사용자 결정). RE2 계열(선형 시간, 역참조 없음), 유니코드 클래스, 크레이트의 반복·`$` 규칙.
  에러 문구는 fung 것이고, 크레이트를 올릴 때 문법 변화는 기대 출력이 잡는다(06 "정규식"). 정규식 엔진 직접 구현(Thompson NFA)은 나중의 공부 과제.
- [x] 배포 → **플랫폼별 CI 러너**(TLS 암호 부분의 C 코드 때문에 대상용 C 컴파일러가 필요): `x86_64-unknown-linux-musl`·`aarch64-unknown-linux-musl`(C 컴파일러 `musl-gcc`),
  macOS 두 대상은 Apple 실리콘 러너 하나에서(6a, 2026-10-17; 노트 `docs/notes/R6a-release-prep.md`, `docs/notes/release.md`).
  - Linux 바이너리 → **musl 정적 빌드**(아무 서버에서나 돈다). glibc 정적 링크를 쓰지 않는 이유: `getaddrinfo`가 NSS로 공유 라이브러리를 실행 중에 읽어 같은 버전의 glibc가 있어야 한다.
    `openpty`의 `libutil` 링크는 gnu 대상에서만(`#[link(name = "util")]`; musl은 `libc.a`에 있음).
  - 릴리스 파일 이름·`SHA256SUMS`·빌드 증명·설치 스크립트·CHANGELOG 릴리스 노트. 소스에서 설치하는 길은 `cargo install --locked --git https://github.com/tunarider/fung --tag v<버전> fung-cli`
    (`include_str!` 경로가 모두 저장소 안이라 git에서 빌드된다; 이 길은 `FUNG_BUILD_VERSION`이 없어 `fung version`이 `0.0.0-dev`)와 저장소의 `cargo build --release --locked`. README·매뉴얼 설치 절에 있다.
- [x] 버전 → 0.x 동안은 호환이 깨지는 변화마다 마이너 버전을 올린다(0.2.0 첫 Rust 구현, 0.3.0 동시성 v2, 0.4.0 언어·VM 검토와 매뉴얼). 1.0은 나중에 정한다.
- [x] 테스트 → 출력이 정해지는 검사는 **기대 출력 파일**(`crates/fung-cli/tests/expected/<주제>/`, `crates/fung-vm/tests/expected/`, 형식은 `crates/fung-cli/tests/common/expected.rs` 머리 주석).
  갱신은 `FUNG_EXPECTED=bless …` 뒤 diff 검토, 테스트마다 사례 수를 상수로 단언한다. 실행은 `TZ=UTC`(`common::TZ`), 호스트에 따라 달라지는 값은 치환(`$TMP`·`$PORT`·`$SIZE` 등),
  Linux·macOS CI가 같은 파일로 통과한다. 명세 파일 `spec/builtins.txt`·`errors.txt`는 Rust 표(`fung_vm::boot`, `fung_front::diag::catalog`)에서 생성한다(`FUNG_UPDATE_SPEC=1 cargo test spec_file`).
  테스트가 띄우는 자식 프로세스는 `common::Reaped`가 쥐어 패닉해도 거둔다. 설명은 `docs/notes/R6b-go-removal.md`.
- [x] 옮긴 이름의 안내(`resolve::MOVED_TO_MODULE`, prelude에서 `conc`로 옮긴 `fan_in`·`pmap`·`retry`) → **1.0까지 둔다** (사용자 결정, 2026-10-24).
  표 하나의 비용으로 E0204에 정확한 도움말(`help: moved to module conc: …`)이 붙는다. 1.0을 정할 때 다시 본다.

### 남은 일

- [ ] **다음 릴리스 0.4.1: rustls 공지 4건 해제, 2026-10-28 이후.** RUSTSEC-2026-0346(사용자 세션 저장소의 `Debug` 출력), -0347(ECH 거절 뒤 바깥 서버로 보낸 데이터),
  -0348(brotli 인증서 압축 풀기), -0349(서버 이름별 키 공유 예측). 2026-10-14에 0.23.46의 수정과 함께 나왔고, 넷 다 fung 설정에 닿지 않는다
  (세션 재개 끔, ECH 안 켬, `brotli` 기능 끔, `ring`에 하이브리드(ML-KEM) 그룹이 없음). 2026-10-28 이후 `cargo update -p rustls --precise 0.23.46`으로 올리고
  `deny.toml`의 `ignore` 네 줄과 `.cargo/audit.toml`을 지운다. 그때 새로 따라오는 크레이트가 있으면 위 표와 2주 규칙을 다시 확인한다.
  같은 때에 위 표에서 "2주 미만이라 고정"한 것들(`libc` 0.2.190, `rustls-native-certs` 0.8.5, `zeroize` 1.9.1, `cc` 1.8.0)도 2주가 지났으니 올릴지 본다. 릴리스 태그와 시험 실행은 사용자 확인 뒤.
- [ ] **매뉴얼을 쓰며 찾은 문제 (`docs/plans/2026-10-20-manual-findings.md`)**. 항목의 재현과 선택지는 그 문서에 있다.
  2026-10-24 보수 작업에서 F-1~F-4·F-6~F-10·F-13·D-1·D-4와 3절의 설계 문서 차이를 닫았다(CHANGELOG `[Unreleased]`). 남은 것:
  - F-5(`http.serve`가 정상 종료 때 `Ok(nil)`)와 F-11(`fs.move`가 기본 거절, `overwrite: true`)은 2026-10-24 사용자 결정으로 닫았다(CHANGELOG 호환 깨짐).
  - 2026-10-26에 닫았다: D-2(카탈로그의 help는 기본 도움말, E0308의 기본을 올림, 매뉴얼 라벨 "기본 도움말").
    D-3은 2026-10-26에 닫았다(중첩 E0115, 인자·매개변수 개수 E0230, E0802 은퇴, CHANGELOG 호환 깨짐).
    F-14는 2026-10-26에 닫았다(`perm: 0o600`, CHANGELOG 더함).
- [ ] **언어·VM 검토의 남은 의심**: `docs/plans/2026-10-20-language-vm-review.md` 12절(확인하지 못한 것 12개). 검토의 나머지는 0.4.0에서 모두 처리했다.
- [ ] **여러 코어 (R7 다음)**: 지금은 실행 스레드 하나(여러 코어는 여러 프로세스, 예: `http.serve`의 `reuse_port`). M:N은 보류.
  태스크마다 힙을 따로 두고 경계(`spawn`이 붙잡은 값, 채널로 보낸 값, `await` 결과)에서만 복사한다(Erlang식; 큰 문자열만 공유하는 refc binary).
  그때 할 일: `RtProto.consts`에서 코드와 링크된 `Rc` 상수 나누기(태스크 경계를 넘는 클로저 복사가 코드까지 깊이 복사하지 않게),
  포인터로 비교하는 클로저·내장 함수·모듈(`value/compare.rs`)의 동일성을 복사 뒤에도 유지.
- [ ] 언어 층 후보(지금 결정하지 않음; 구조적 동시성은 L1에서 들였다): 채널 대신 메일박스(태스크 번호 + 골라 받기; 순환이 완전히 사라지지만 자동 종료를 잃고 link·monitor 설계가 필요).
- [x] musl 빌드 벤치마크 → 2026-10-25 Docker의 arm64 Linux에서 glibc 판과 쟀다(`docs/notes/bench-languages.md` 4.1절). 할당이 많은 `map`·`str`·`wait`가 4~15% 느리다. 할당기를 바꿀 만큼은 아니라 그대로 둔다.
- [ ] CI 컴파일 캐시: 아직 넣지 않았다.
- [x] 내부 자리 (2026-10-26 다시 봄, 바꾸지 않음): `need_str_map`("map of strings" E0403)은 `_native.net`에 둔다(쓰는 곳이 `http`의 헤더 검사뿐; 다른 모듈이 쓰게 되면 옮긴다).
  가짜 터미널 도우미 `fung_vm::term::pty`(`#[doc(hidden)]`)는 라이브러리에 둔다(테스트 전용 feature는 `deps.rs`의 `path`만 규칙과 부딪힘; 명령은 `pty`를 부르지 않는다).
  다리 모듈의 표시 이름은 모두 공개 모듈과 같다(`Module::new("fs", "_native.fs")`, `conc`도): 키가 달라 충돌이 없는 일관된 규칙이다.
- [x] 앞단 다듬기 (R1 노트): 2026-10-26에 정리했다(0.5.0 목록 S10: 중복 검사는 집합, `SourceMap`의 번호 넘침은 멈춤, 바꾸지 않은 것과 이유도 거기).
- [x] CI에서 본 간헐 실패 (2026-10-24, main 푸시 네 번) → 같은 날 시간 대신 상태로 판정하게 고쳤다.
  - `perf`의 `collections_stay_linear`(macOS에서 두 번 0.3초 초과): 같은 파일의 무거운 시험과 동시에 돈 탓. `perf` 시험을 잠금 하나로 줄 세웠다.
  - `vm::park_tests::offloads_reuse_pool_threads`(Ubuntu, 일 200개에 스레드 9개, 한도 8): 일마다 풀 스레드가 모두 쉬는 상태(`pool_state()`)를 기다린 뒤 다음 일을 맡기고, 스레드가 정확히 1개인지 본다.
  - `native::process::tests::a_fast_command_does_not_starve_other_tasks`(macOS, `sleep(2ms)` 태스크의 횟수를 시간과 견줌): 출력을 읽는 태스크의 주차 횟수(`parks()`, 300 MB를 약 272 KiB씩이면 1,077번 이상)와
    `sleep` 없이 도는 옆 태스크의 진행이 그 이상인지를 본다. 이 시험을 바꾸다 **런타임 버그**를 찾았다: `swap`의 함수 안에서 끝난 차례를 버려서, 한 바퀴의 호출 수가
    `YIELD_EVERY`의 약수인 반복(`fn(): swap(n, f); lp(k - 1) ;;()`, 릴리스 빌드)이 차례를 영영 넘기지 않았다. 차례를 빚으로 남기게 고쳤다(`block.rs`의 `end_turn`, 시험 `atom_tests`).
  - `http_client_matches_the_expected_output`의 "no server"(Ubuntu, 거부되어야 할 포트에서 다른 시험의 가짜 서버가 403): `refused_port()`가 놓은 `:0` 포트를 옆 시험의 가짜 서버가 받았다.
    거부될 포트를 운영체제의 임시 포트 범위 밖(20000~29999)에서 고른다. `fung-vm`의 `free_port`도 같은 범위에서 고른다(2026-10-26).
- [x] 시험 부채: 2026-10-26에 정리했다(0.5.0 목록 S14, 바꾸지 않은 것과 이유도 거기).
- [ ] 일정: 정하지 않음. 지금은 공부를 겸한 단계.

### 알려진 한계

고치기로 정하지 않은 동작상 한계. 고치면 해당 설계 문서의 규칙과 이 목록을 함께 고친다. 배경은 단계 노트(`docs/notes/R<n>-*.md`)와 `docs/notes/R-port-record.md`.

- 컬렉션 (2026-10-20 검토 P2·P7): 앞에 붙이기 `[x] + acc`는 매번 전체를 복사해 누적하면 n²다(매뉴얼의 안내대로 뒤에 붙이고 `reverse`). 고치려면 영속 벡터에 앞쪽 버퍼를 두거나 RRB 트리로 바꿔야 한다.
  상수 접기(P7)는 리스트 리터럴만 한다. 맵·튜플은 상수 종류가 새로 필요하고, 빈 리스트를 품은 리스트는 빈 리스트 상수가 `args`로 읽혀 접지 않는다.
- 태스크 (05; R3a·R3b 노트)
  - 끝나지 않는 파일 읽기(FIFO 등)는 기한에 버려져도 풀 스레드 하나를 붙잡고, 그런 것이 64개면 다른 파일 일은 줄에서 기다린다(기한·취소에는 끊긴다).
  - 형제 태스크가 시작하는 순서는 보장되지 않는다(실행 스레드 하나가 대기열 순서로 돌지만, 띄운 쪽이 차례를 넘기는 시점과 깨어나는 순서에 달림). 범위에 덧붙는 실패도 실패한 순서다.
    순서에 기대는 프로그램과 `test` 블록은 경쟁 상태다.
  - 띄운 태스크는 띄운 쪽이 차례를 넘기거나(호출 1,000번, R1b) 주차하기 전에는 돌지 못한다. 구조적 동시성(L1)에서는 범위가 자식을 기다리므로 자식은 돈다. 첫 차례 전에 범위가 취소되면 그 자식은 한 번도 돌지 않는다(`scope(fn(s): spawn(s, () => print("child")); cancel(s) ;;)`; 2026-10-26 확인, 전의 문장은 자유 `spawn` 시절의 것. 0.5.0 문서 L14).
  - Rust로 쓴 내장 함수 하나(큰 `json.parse`, 콜백 없는 큰 정렬)는 중간에 차례를 넘기지 않는다. 약 6 MB JSON 파싱 옆의 가벼운 HTTP 요청은 파싱 시간(약 72 ms)만큼 늦다(R6 측정, 스펙 10.4는 기록만).
  - 공정성(R6, 순서를 보장하지 않을 뿐 굶기지는 않음): 오래된 깨움이 태스크에 차례를 한 번 더 줄 수 있다. 연결의 차례 줄은 놓을 때 기다리는 쪽을 모두 깨워 다시 다투게 한다.
  - pidfd가 없는 Linux(5.3 전)에서는 명령마다 풀 스레드 하나가 끝을 기다린다. 64개를 넘는 긴 명령은 다른 파일 일을 줄에 세운다.
  - REPL: 한 묶음에 Ctrl-C 둘이 오면 `release`가 시작하기도 전에 버린다(알림은 "정리가 끝나지 않음"). 버릴 때 태스크를 먼저 버리고 루프 등록을 뒤에 해제한다(닫힌 fd 번호에 해제를 부름; 해는 없음).
  - `fung test`에서 파일 N의 런타임(과 풀)은 파일 N+1이 신호 처리기를 바꿀 때 사라진다(쌓이지는 않음).
  - 시험 도우미: `release_fifo`의 막히는 열기는 줄에서 버려진 일이면 떨어진 시험 스레드를 남긴다.
  - 끝없이 도는 태스크(막히지 않는 것)는 다른 태스크들 사이의 진짜 교착을 가린다. 배경 태스크는 L6에서 없어졌다.
  - 회수되지 않는 순환(`Rc`): 자기 핸들을 결과로 담은 태스크, 서로의 받는 끝을 버퍼에 담은 채 버려진 두 채널(그래서 둘 다 닫히지 않는다; 보내는 끝끼리만 엮인 순환은 마지막 받는 끝이 사라질 때 버퍼를 버려 끊긴다, 2026-10-26 측정. 0.5.0 문서 L7). atom을 거치는 순환은 E0708이 막는다(L5).
  - 외부 명령의 취소(L6): 터미널 앞에서는 명령이 fung과 같은 그룹이라, 취소가 명령 하나만 끝낸다(손자는 남을 수 있음). `setsid`로 빠져나간 손자가 출력을 쥐고 있으면 취소되지 않은 명령은 그것이 끝날 때까지 기다린다(전과 같음).
  - E0705가 훑지 않는 곳: 다른 채널의 버퍼(두 채널 순환), 모듈·클로저의 전역(런타임 끝에 `World::release_values`가 끊음), 태스크 핸들.
  - 변하는 것(채널 끝, 태스크 등)을 품은 값의 `send` 비용은 그 값의 크기에 비례한다. 품지 않은 값은 훑지 않는다: 큰 리스트·맵은 노드의 기억으로 O(1), 작은 것은 공유된 부분을 한 번씩 훑는다(L4). 같은 영속 벡터를 공유하는 여러 보기는 보기마다 다시 훑는다(일부러 만든 입력에서는 지수적).
  - 꼬리 위치 호출: fung으로 쓴 라이브러리 함수(`http.*`, `fs.with_open` 등)를 꼬리 위치에서 부르면 그 프레임이 사라져, 에러가 부른 쪽의 부른 쪽 위치를 가리키고
    경로에 `… (1 tail calls elided)`가 보인다. 꼬리 호출마다 호출 자리를 적어야 해서(모든 꼬리 호출에 저장 하나) 하지 않았다.
- 메모리·스택 (R1·R4b·R6a 노트)
  - 값 출력(`value/format.rs`, `print`·`str`)은 재귀라 큰 스택에 기댄다. 10,000겹 JSON 값과 백만 겹 리스트(진입 스레드)는 출력되지만 5백만 겹 리스트를 `str`하면 디버그 진입 스레드에서 스택이 넘쳐 죽는다(종료 코드 134).
  - 실행 스레드(모든 태스크가 그 위에서 돈다)는 4 GiB 스택을 예약하고, 거부되는 호스트에서는 1 GiB·256 MiB·64 MiB로 줄여 다시 시도한다(`fung_vm::stack::ENTRY_STACKS`).
    줄어든 호스트에서는 아주 깊은 네이티브 재귀(깊은 값 출력·비교·해시)의 한도가 낮다. `map`·`fold`의 깊은 중첩은 이제 프레임 한도 E0804로 끝난다. 디버그 빌드는 약 60만 단계 체인에서 넘친다(릴리스는 400만도 통과; 2026-10-26 측정, 0.5.0 문서 L9).
- 문자열과 바이트 (02·06; R2·R4a 노트)
  - 문자열은 UTF-8이라 바이트를 그대로 품지 않는다. 비UTF-8 프로세스 출력·환경 변수 값·파일 내용(`fs.read`·`read_lines`·`read_line`)·HTTP 바이너리 본문·경로·시간대 약어는
    잘못된 부분마다 U+FFFD 하나로 바뀐다(원래 바이트 보존 없음). 비UTF-8 디렉터리 아래 모듈 경로도 U+FFFD로 표시된다. 문자열을 바이트 기반으로 바꿀 때 다시 본다.
  - `exec`·`sh`의 `env`는 물려받은 환경에 더해지고, 자식이 보는 환경은 이름순으로 정렬된다. (이름에 `=`가 든 키와 빈 이름은 2026-10-26부터 E0403.)
- 파일 (06 `fs`; R4a 노트)
  - `close(2)`의 에러를 보고하지 않는다(`fs.close`·`write`·`append`·`copy`, 버린 핸들의 버퍼 비우기). NFS 같은 파일 시스템에서만 드러난다.
  - 마지막 사본이 사라질 때의 `close(2)`는 실행 스레드에서 돈다(`Drop`). NFS 같은 곳에서 멈추면 그동안 다른 태스크도 멈춘다. 고친다면 배경 닫기 스레드에 넘긴다.
- 시간 (06 `time`; R4b 노트)
  - `now()`에 단조 시계가 없다. 두 `now()` 사이에 시스템 시각이 바뀌면(NTP 보정 등) 경과 시간 재기가 틀린다.
- 정규식 (06; R4c 노트)
  - 컴파일된 크기가 128 MiB를 넘는 패턴(`[a-z]{1000}` 수천 번, `\pL`을 약 5,000번 넘게 반복)은 E0515 `pattern too large`이고, 실패까지 시간이 걸린다(`\pL` × 20,000이 릴리스 약 3초).
    컴파일은 풀에서 돌아 그동안 다른 태스크는 계속 돈다. 같은 패턴을 동시에 놓친 두 태스크는 각자 컴파일한다.
- 소켓·TLS (06; R4d 노트)
  - `set_deadline`은 이미 막힌 호출에 닿지 않는다(막힌 기다림은 시작할 때의 기한을 들고 있고, 새 기한은 다음 기다림부터).
  - 이름 붙은 포트(`:http`)를 찾지 않는다(``invalid port `http` ``). 주소 계열을 경주시키지 않는다(Happy Eyeballs 없음): 주소들을 한 기한 안에서 차례로 시도하고 첫 에러를 보고한다.
  - 한 연결에서 읽기와 쓰기는 각각 한 번에 하나다. 두 번째 읽는 쪽(쓰는 쪽)은 차례 줄에서 주차해 앞의 것이 끝나기를 기다린다(취소·기한에는 끊긴다).
  - 이름 조회(`getaddrinfo`)는 끊을 수 없어 풀(최대 64)에서 돈다. 취소되거나 시간이 다 된 `connect`는 바로 돌아오지만 그 풀 스레드는 해석기가 답할 때까지 붙잡힌다.
  - TLS: close_notify를 기다리지 않고 보낸다(소켓 버퍼가 차 있으면 버려짐). ALPN을 보내지 않는다. `SSL_CERT_FILE`·`SSL_CERT_DIR`이 하나라도 있으면 변수의 위치만 읽는다.
    정적 바이너리도 실행 중에 시스템 CA 파일을 읽으므로 CA 인증서가 없는 `scratch` 컨테이너에서는 https가 실패한다.
- HTTP (06 "HTTP"; R4d 노트)
  - 응답 본문 상한 64 MiB(넘으면 `Err 'network`; 요청별 `max_body` 같은 옵션은 나중). 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축 없음.
    서버는 연결마다 요청 하나라 HTTP/1.1 응답에 늘 `Connection: close`이고 HTTP/1.0 keep-alive에 `Connection: keep-alive`로 답하지 않는다. 101 응답은 본문 없음(연결을 넘겨주지 않음).
  - SOCKS 프록시(`socks5://`·`socks5h://`)와 `https://` 프록시 너머의 https는 지원하지 않는다. 알 수 없는 스킴의 프록시 URL에 포트가 없으면 80으로 연결하고,
    `NO_PROXY`의 공백 다듬기는 ASCII 공백(공백·탭·CR·LF)만 본다.
  - URL: IDN·퓨니코드 없음(`NO_PROXY`·같은 사이트 판정 포함), 호스트의 `%XX`(0x80 이상)를 풀지 않음, 괄호 IPv6는 "IPv4인가 아닌가"만 검사(잘못된 IPv6는 연결에서 다른 문구로 실패),
    에러 URL의 조각(fragment)은 다시 이스케이프하지 않음, 사용자 정보의 비ASCII는 가린 에러 URL에서 `%C3%A9`처럼 보임.
  - 서버가 일찍 닫아 쓰기가 실패하면 답을 읽을 수 있어도 쓰기 에러를 보고한다. `CONNECT` 답은 상태 줄과 헤더 줄만 검사하고, 프록시 요청의 `Host` 헤더 검사가 없다.
  - 인자 에러의 호출 경로에 `extras/http.fung` 프레임이 보인다(`serve`의 `ready` 에러에는 `_ready_check`·`serve` 두 프레임). 06은 extras 프레임을 보이게 둔다.
  - 비용: 헤더를 fung으로 해석하므로 접히거나 끝없는 머리 40만 줄(10 MiB 예산)에 클라이언트 릴리스 약 1초, 꽉 찬 요청 머리(1 MiB, 값 500개) 하나에 서버 요청마다 약 0.1~0.25초,
    1바이트 청크 10만 개에 약 0.6초.
  - 서버: 핸들러가 넣은 틀 헤더는 쓴 그대로(`Content-Length`는 자동 값을 끄지만 본문은 다 씀, `Transfer-Encoding`은 해석하지 않음, 101 전환의 `Connection: upgrade` 세부 없음).
    이름이 대소문자만 다른 응답 헤더는 뒤의 것. 431 뒤에는 반쯤 닫기 없이 읽기만 한다. `PRI * HTTP/2.0`, `CONNECT`의 2xx 답, 요청 컨텍스트를 취소하는 배경 읽기는 없다.
  - 서버: macOS는 받기 전 대기 줄(`kern.ipc.somaxconn`, 기본 128)이 차면 새 연결을 끊는다(`connection reset by peer`). 한꺼번에 128개 넘게 몰려오는 접속은 그만큼 실패한다. fung은 시스템이 허락하는 가장 긴 줄을 요청한다. Linux의 기본값은 4096.
- 도구 (08; R5 노트)
  - REPL 세션의 `SourceMap`은 입력마다 파일 하나씩 계속 커진다(앞서 정의한 함수의 에러가 그 입력을 인용해야 해서). 런타임은 같은 글을 나눠 쥔다(`Arc<str>`, 2026-10-26).
  - 표시 너비는 글자 단위 UAX #11이라 ZWJ로 이은 이모지(각 글자 2칸)와 VS16(U+FE0F, 0칸) 붙은 글자는 터미널과 다를 수 있다.
