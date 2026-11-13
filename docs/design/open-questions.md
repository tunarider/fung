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
- [ ] `math` 모듈 (06·07, 2026-11-08 SP6): 지금 fung에는 `abs`·`floor`·`ceil`·`round`·`sqrt`·비트 연산이 없다. 후보 목록과 이름(`band`·`bor`·`bxor`·`shl`·`shr`), 정수·실수 규칙을 정해 더한다.
- [ ] 튜플 원소 접근: 매개변수 패턴으로 대부분 해소되어 보류. 알파 스크립트에서 "튜플 변수의 한 원소"가 자주 나오면 `fst`/`snd` 재검토.

## M8 후속 (보류, 2026-10-14)

- [ ] `time`: 12시간제 토큰(`hh`·`A`), 토큰 이스케이프, 달력 산술(`add_months` 등).
- [ ] `time`: 필드에서 시각 만들기(`{year, month, …}` → 시각), 이름 있는 시간대(`"Asia/Seoul"`로 바꿔 보기; 지금은 `utc`·`local`과 고정 오프셋만). 2026-11-11 D73에서 미룸(LB49).
- [ ] `http`: 응답 스트리밍·대용량 본문, 쿠키, TLS 서버.
- [ ] `regex`: 외부 입력 패턴을 `Err`로 검사하는 함수(`regex.compile`류).
- [ ] `exec`·`sh`: 출력 흘려보내기(줄마다 받기, 긴 명령의 진행). 지금은 끝난 뒤 모아서 준다. 2026-11-11 D73에서 미룸(LB49).
- [ ] `fs.stat`: 링크인지, 권한 비트(`mode`). 2026-11-11 D73에서 미룸(LB49).
- [ ] `fung test`: 이름으로 골라 돌리기(`fung test path -k 이름`류). 2026-11-11 D83에서 미룸(TL12). 고를 때의 짝 맞추기(부분 문자열·글롭)와 고른 것이 없을 때의 종료 코드를 정한다.

## 배포 후속 (보류, 2026-10-14)

- [ ] Windows 지원: 빌드는 되지만 REPL raw 모드 편집기를 확인하지 않았다. 확인 후 릴리스 대상(`scripts/build-release.sh`)에 추가하고 설치는 zip + PowerShell.
- [ ] Homebrew tap: macOS에서 `brew upgrade`로 업데이트. 그때 릴리스 도구를 바꿀지 함께 정한다.
- [ ] `fung upgrade` 자체 업데이트 명령: 지금은 설치 스크립트를 다시 실행하면 된다.
- [ ] macOS 코드 서명·공증: 브라우저로 받은 바이너리는 Gatekeeper가 막는다(`curl` 설치는 해당 없음). Apple 개발자 계정이 필요하다.

## 매뉴얼 (2026-10-20 설계, `docs/superpowers/specs/2026-10-20-manual-structure-design.md`)

1~5단계(뼈대·레퍼런스·배우기·예제 모음·VM)는 0.4.0에 들어갔다.

- [x] 후속(L16, 2026-11-08): VM 부록의 손으로 적은 `dump`를 검사되는 `dump` 펜스로 바꿨다(`fn=NAME`으로 한 함수만; dump 모양의 `plain`·`core` 펜스는 이제 에러).
  언어 명세는 주제마다 한 쪽(`reference/language/*.md`, 15쪽)으로 나누고 쪽마다 "경계 사례와 에러"(실행해 검사하는 예)를 더했다. `reference/language.html`은 목차와 옛 앵커 안내로 남았다.
- [ ] 보류: 매뉴얼 전문 검색(지금은 함수 필터만).
- [x] 매뉴얼 후속(2026-10-20 최종 검토의 사소한 것): 2026-10-26에 정리했다(0.5.0 목록 S15). 장 끝 구성과 4장은 0.5.0 문서 L18.
- [ ] 매뉴얼을 쓰며 찾은 언어·도구의 문제: `docs/plans/2026-10-20-manual-findings.md`. 결정을 기다리는 것은 아래 "남은 일"에 모았다.

## 성능

측정은 `bench/`(방법과 기준선 `bench/README.md`), 동시성 런타임의 수치는 `docs/notes/R7-scheduler.md` "R단계 전체 요약".
2026-10-14의 첫 측정과 1~3단계 최적화는 `docs/notes/open-questions-closed.md`.

- [x] 레지스터 기반 VM, 인라인 캐시 → **하지 않는다**(L15, 2026-11-08, 사용자 결정 "추천대로"). 인라인 캐시는 전역이 이미 번호 칸이라(`LOADGLOBAL`이 이름을 찾지 않음)
  대상이 거의 없고, 맵에 숨은 모양(shape)이 없어 `Field`도 캐시할 것이 없다. 레지스터 VM(명령 수 20~30% 감소)은 컴파일러·VM·`dump`·기대 출력·매뉴얼 VM 부록을 다시 짜는 일이라 0.5.0의 범위를 넘는다.
  묶음 명령(superinstruction) 실험은 0.5.0 뒤. **다시 볼 조건**: 계산이 주 용도인 사용 사례가 생기거나, 텍스트 쪽 할당 문제를 고친 뒤에도 `bench/compare`의 계산 묶음이 Python의 1.5배를 넘을 때.
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
  → 인자 `Vec`은 2026-11-11 VM15에서 없앴다(네 개까지는 값 안에 담는 `Args`, `docs/notes/bench-languages.md` 4.8절). 조각마다의 할당은 그대로다.
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
- [x] **P12b 릴리스 빌드의 LTO** → **기각**(L15, 2026-11-08): 정한 규칙("시험을 모은 뒤 `--release --no-run` 증가 15초 이하면 넣음")에 `fung-vm` 단위 시험 실행 파일 하나가 이미 64초로 걸린다.
  표는 `docs/notes/bench-languages.md` 4.3절. 아래는 2026-10-24의 첫 측정. 크레이트 경계를 넘어 함께 최적화한다.
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
- [x] **P14** Linux glibc 판이 계산에서 musl 판보다 크게 느린 이유 → 닫음(L15, 2026-11-08). `loop`의 원인은 glibc malloc의 스레드별 아레나였다: 실행 스레드를 띄우기 전에
  `mallopt(M_ARENA_MAX, 1)`(Linux glibc만)로 0.447 → 0.287초(musl 0.280). `fib`(0.327 대 0.225)는 할당·`memcpy`·스택·링크 방식·LTO가 아니고, 더 가리려면 `perf`가 필요해 상한에 따라 닫았다.
  실험 표와 재현 조건은 `docs/notes/bench-languages.md` 4.4절.

## Rust 구현: 정한 것과 남은 일

fung은 Rust로 구현한다. 문법과 사용자에게 보이는 동작이 핵심이고 내부는 바뀌어도 된다. C 구현은 기각: GC·동시성·TLS를 직접 만들어야 하고,
`http.serve`처럼 외부 입력을 파싱하는 코드에서 메모리 안전을 잃는다. 구현 독립으로 이어지는 것: 설계 문서, 에러 번호, 매뉴얼, `extras/`의 fung 코드,
골든 테스트(`tests/lang`), 기대 출력 파일, `bench/`, 릴리스 흐름.
이식의 기록(단계별 결정, 비교, 해결된 항목)은 `docs/notes/R-port-record.md`, 단계별 설명은 `docs/notes/R<n>-*.md`.

### 정한 것

- [x] 동시성과 메모리 모델 → **실행 스레드 하나가 모든 태스크를 돌린다(M:1, 스택 없는 그린 스레드), 값은 참조 계수.**
  (동시성 v2 R1b~R5, 2026-10-19; 노트 `docs/notes/R7-scheduler.md`. 3a의 "태스크마다 운영체제 스레드 + 실행권 잠금"은 대체됐다.)
  - 기다리는 태스크는 주차하고(`vm/block.rs`·`vm/sched.rs`), 계산만 하는 태스크는 호출 `YIELD_EVERY`(1,000)번마다 차례를 넘긴다.
    입력 크기에 비례하는 내장 함수와 큰 값의 해제는 약 1 ms 분량의 조각마다 넘긴다(L6, `vm/pieces.rs`, `value/release.rs`의 `LATER`).
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
  채널에 그 채널의 끝을 보내는 것(3b, L4)도, 다른 채널·태스크를 거쳐 닿는 고리(L7, 05)도 E0709다(E0705는 2026-11-10 SP24에 합침). `send(tx, v)`가 `v`를 재귀 없이 깊이 훑는다. 비용은 아래 "알려진 한계".
- [x] 라이브러리 구조 → **언어 핵심은 최소, 공식 라이브러리는 패키지 구조로 만들되 패키지 관리자가 생길 때까지 바이너리에 함께 넣는다.**
  런타임은 파일·프로세스·TCP 소켓·TLS·시간·정규식 같은 네이티브 원시 기능만 주고, HTTP 프로토콜·JSON·`cli` 등은 fung 라이브러리로 쓴다.
  - 내장 함수 배치 → **prelude에는 확실한 것만** (2026-10-17). 더하는 것도 빼는 것도 호환성 깨짐이다(더한 이름을 쓰던 코드는 D38의 E0240이 된다; 그 전에는 W0103 경고뿐이라 더하기는 안전했다).
    정책(동시 실행 수, 재시도 횟수·간격)을 품은 `fan_in`·`pmap`·`retry`는 `conc` 모듈. `until`·`iterate`(`while`이 없는 언어의 반복), `exec`·`sh`(셸을 대신하는 언어의 핵심)는 prelude.
    `remove`·`close`와 `fs.remove`·`close`의 이름 겹침은 유지하고 매뉴얼에 한 줄. 옮긴 이름을 그대로 쓰면 E0204에 `help: moved to module conc: …`(`resolve::MOVED_TO_MODULE`).
  - 구현 언어 지침(2026-10-15): "빠지면 다른 것을 만들기 힘들어지는 것"이 핵심이다. fung으로 쓸 수 없는 것(OS·외부 세계) → Rust.
    핵심이면서 람다를 부르지 않는 순수 계산(문자열, `len`, json 파싱, 정규식) → Rust. 핵심이면서 람다를 부르는 고차 함수(`map`·`fold`·`sort_by`·`each`) → Rust에 두고
    이어 가기 프레임으로 부른다(R2, 2026-10-18: 람다를 직접 부르지 않고 VM에 부탁하므로 태스크를 멈출 수 있고 Rust 스택을 쓰지 않는다. `docs/notes/R7-scheduler.md`). 핵심이 아닌 조합·프로토콜(`take`·`retry`·`pmap`·HTTP) → fung.
    구현 언어 이동은 사용자 코드에 영향이 없어 나중에 바꿔도 된다(진단도 같다: 호출 경로가 내장 함수의 프레임을 언어와 무관하게 뺀다, 2026-11-10 VA19).
    원소마다 람다를 부르던 조합 중 모든 원소를 도는 것(`reverse`·`keys`·`values`·`sum`·`min`·`max`)은 비용 때문에 Rust로 옮겼다(2026-11-10 VA25).
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
    | `memchr` | 2.8.3 (2026-07-08) | Unlicense OR MIT | `regex`·`regex-automata`·`aho-corasick`가 끌고 옴, 그리고 `fung-vm` 직접(L4: 문자열이 바이트가 되어 `split`·`replace`·`in`의 부분 열 찾기를 `memmem`으로) | 빠른 바이트 찾기 | 없음 |
    | `libc` | 0.2.191 (2026-10-16) | MIT OR Apache-2.0 | `fung-vm` 직접, 유닉스 대상만 | `signal`·`raise`·`pthread_sigmask`(SIGPIPE로 죽기)는 표준 라이브러리에 없고, `sigset_t` 배치가 운영체제마다 달라 손으로 선언하면 틀리기 쉽다. 소켓(`native/sock.rs`: 비차단·close-on-exec·`SO_NOSIGPIPE`·`poll`·`getaddrinfo`)에도 쓴다 | 없음 |
    | `rustls` | 0.23.46 (2026-10-14) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접, 기본 기능 끄고 `ring`·`std`·`tls12` (+ `fung-cli` dev-dependency, 테스트 TLS 서버 `tests/common/tls_server.rs`) | TLS(암호와 프로토콜 상태 기계)는 직접 짜면 위험하다(의존성 정책). 소켓을 만지지 않는 상태 기계라 비차단 소켓과 이벤트 루프(`vm/poller.rs`) 위에서 돈다 | `ring`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `zeroize`, `once_cell` |
    | `rustls-native-certs` | 0.8.5 (2026-10-13) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접 | 운영체제 루트 인증서(macOS 키체인, Linux 인증서 파일)와 `SSL_CERT_FILE`·`SSL_CERT_DIR`을 읽는다. 운영체제마다 다른 저장소를 직접 읽는 것은 플랫폼 코드다 | `rustls-pki-types`, `openssl-probe`(macOS 밖 유닉스), `security-framework`(macOS), `schannel`(Windows) |
    | `rustls-pki-types` | 1.15.1 (2026-07-23) | MIT OR Apache-2.0 | `rustls`·`rustls-webpki`·`rustls-native-certs`가 끌고 옴 | 인증서·키·서버 이름 타입 | `zeroize` |
    | `rustls-webpki` | 0.103.16 (2026-10-20; 공지 RUSTSEC-2026-0382가 닿아 2주 규칙의 예외로 바로 올림, 아래 보안 공지 정책) | ISC | `rustls`가 끌고 옴 | 인증서 사슬·이름 검증(rustls의 WebPKI 검증기, `native/tls.rs`) | `ring`, `rustls-pki-types`, `untrusted` |
    | `ring` | 0.17.14 (2025-03-11) | Apache-2.0 AND ISC | `rustls`가 끌고 옴(암호 공급자로 고름) | 암호 기본 연산. C·어셈블리를 빌드하므로 C 컴파일러가 필요하다(CI 러너에 있음) | `cc`·`find-msvc-tools`·`shlex`(빌드 때만), `cfg-if`, `getrandom`, `untrusted`, `windows-sys`(Windows) |
    | `untrusted` | 0.9.0 (2021-07-13) | ISC | `ring`·`rustls-webpki`가 끌고 옴 | 믿을 수 없는 입력을 안전하게 읽는 도우미 | 없음 |
    | `subtle` | 2.6.1 (2024-06-24) | BSD-3-Clause | `rustls`가 끌고 옴 | 상수 시간 비교 | 없음 |
    | `zeroize` | 1.9.1 (2026-10-06) | Apache-2.0 OR MIT | `rustls`·`rustls-pki-types`가 끌고 옴 | 비밀 값을 메모리에서 지우기 | 없음 |
    | `once_cell` | 1.21.4 (2026-03-12) | MIT OR Apache-2.0 | `rustls`가 끌고 옴 | 한 번만 초기화하는 칸 | 없음 |
    | `getrandom` | 0.2.17 (2026-01-11) | MIT OR Apache-2.0 | `ring`이 끌고 옴 | 운영체제 난수 | `cfg-if`, `libc`, `wasi`(wasm) |
    | `cfg-if` | 1.0.5 (2026-09-16) | MIT OR Apache-2.0 | `ring`·`getrandom`이 끌고 옴 | 조건부 컴파일 매크로 | 없음 |
    | `cc`, `find-msvc-tools`, `shlex` | 1.10.0 (2026-10-23), 0.1.15 (2026-10-19), 2.0.1 (2026-05-17) | 모두 MIT OR Apache-2.0 | `ring`의 빌드 스크립트 | `ring`의 C·어셈블리 빌드(실행 파일에는 들어가지 않음) | 서로 |
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

- [x] **매뉴얼을 쓰며 찾은 문제 (`docs/plans/2026-10-20-manual-findings.md`)** → 아래 항목까지 모두 닫았다(0.5.0). 항목의 재현과 선택지는 그 문서에 있다.
  2026-10-24 보수 작업에서 F-1~F-4·F-6~F-10·F-13·D-1·D-4와 3절의 설계 문서 차이를 닫았다(CHANGELOG `[Unreleased]`). 남은 것:
  - F-5(`http.serve`가 정상 종료 때 `Ok(nil)`)와 F-11(`fs.move`가 기본 거절, `overwrite: true`)은 2026-10-24 사용자 결정으로 닫았다(CHANGELOG 호환 깨짐).
  - 2026-10-26에 닫았다: D-2(카탈로그의 help는 기본 도움말, E0308의 기본을 올림, 매뉴얼 라벨 "기본 도움말").
    D-3은 2026-10-26에 닫았다(중첩 E0115, 인자·매개변수 개수 E0230, E0802 은퇴, CHANGELOG 호환 깨짐).
    F-14는 2026-10-26에 닫았다(`perm: 0o600`, CHANGELOG 더함).
- [x] **언어·VM 검토의 남은 의심**: `docs/plans/2026-10-20-language-vm-review.md` 12절(확인하지 못한 것 12개) → 0.5.0 목록 S17과 큰 문제(L1·L3·L7·L9 등)로 옮겨 모두 처리했다(0.5.0). 검토의 나머지는 0.4.0에서 모두 처리했다.
- [ ] **여러 코어 (R7 다음)**: 지금은 실행 스레드 하나(여러 코어는 여러 프로세스, 예: `http.serve`의 `reuse_port`). M:N은 보류.
  태스크마다 힙을 따로 두고 경계(`spawn`이 붙잡은 값, 채널로 보낸 값, `await` 결과)에서만 복사한다(Erlang식; 큰 문자열만 공유하는 refc binary).
  그때 할 일: `RtProto.consts`에서 코드와 링크된 `Rc` 상수 나누기(태스크 경계를 넘는 클로저 복사가 코드까지 깊이 복사하지 않게),
  포인터로 비교하는 클로저·내장 함수·모듈(`value/compare.rs`)의 동일성을 복사 뒤에도 유지.
- [ ] 언어 층 후보(지금 결정하지 않음; 구조적 동시성은 L1에서 들였다): 채널 대신 메일박스(태스크 번호 + 골라 받기; 순환이 완전히 사라지지만 자동 종료를 잃고 link·monitor 설계가 필요).
- [x] musl 빌드 벤치마크 → 2026-10-25 Docker의 arm64 Linux에서 glibc 판과 쟀다(`docs/notes/bench-languages.md` 4.1절). 할당이 많은 `map`·`str`·`wait`가 4~15% 느리다. 할당기를 바꿀 만큼은 아니라 그대로 둔다.
- [ ] CI 컴파일 캐시: 아직 넣지 않았다.
- [x] 내부 자리 (2026-10-26 다시 봄, 바꾸지 않음): `need_str_map`("map of strings" E0403)은 `_native.net`에 둔다(쓰는 곳이 `http`의 헤더 검사뿐; 다른 모듈이 쓰게 되면 옮긴다).
  → 2026-11-10 SP18: `cli`·`conc`도 이름 붙은 인자 검사가 필요해져 `need_count`·`need_dur`·`need_str_map`을 새 다리 `_native.arg`(`count`·`positive`·`dur`·`list_of`·`str_map`)로 옮겼다.
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
- [ ] **세 번째 전체 점검(2026-11-11)을 고치며 남은 것** (`docs/plans/2026-11-11-full-review.md`의 `처리:` 줄, 결정은 `docs/plans/2026-11-11-full-review-fixes.md`):
  - `type_of`와 태그의 타입: `type_of(Circle(1))`은 `'Circle`(태그 이름)이다. `'tag`를 돌려줄지, 명세가 "태그 타입"(같은 `type` 선언의 태그들)을 정의할지 정한다.
    (D91, 2026-11-13: `==`가 타입을 묻지 않게 되어 "비교 규칙과 `type_of`가 다른 말을 한다"는 문제는 사라졌다. 남은 것은 `type_of`의 답 자체다.)
  - `http.serve`의 `req`에는 `(이름, 값)` 튜플 리스트(`header_lines`·`query_pairs`)가 들어 있어 D80 뒤로 `json.stringify(req)`가 E0513이다. 요청을 통째로 로그에 남기려면 필드를 골라야 한다.
    선택지: 매뉴얼 `http.md`에 적고 그대로 두기, 또는 튜플 대신 `[이름, 값]` 리스트로 바꾸기.
  - 리눅스(epoll)에서 돌려 보지 않은 것: W2의 FIFO 핸들 차례(CC14, `native/turn.rs`의 `Hold`)와 표준 출력·에러 쓰기 스레드(CC15, D64), W11의 CI 변경(주 1회 `schedule`, SHA로 고정한 액션, linux/arm64 작업, `[lints]`, TL10·D84). 브랜치를 올려 CI가 돌 때 확인한다.
  - 앞단의 구조(FE62)에서 하지 않은 것: 소스 연산자 이름(`"not_in"` 등)의 열거형화, `resolve/value.rs`의 `partial_shape`·`hidden`이 desugar 모양을 다시 알아보는 것(desugar가 표시를 남기게), 보간 끝 `Eof` 토큰의 `"}"` 표시, `Token`의 크기(`1 + 1 + …` 100만 항의 `check`가 2 GB).
  - `fmt`(TL11)에서 하지 않은 것: 링크와 대상이 같은 디렉터리에 있으면 고친 목록에 링크 이름만 적히는 것, 사용 오류마다 사용법 12줄 전체를 찍는 것.
  - 이름(LB61)은 문서로만 맞췄다: `replace(s, old, new)`와 `regex.replace(s, pattern, repl)`의 매개변수 이름, 불리언 하나가 반환 모양을 바꾸는 `fs.list(dir, full = false)`는 그대로다.

### 알려진 한계

고치기로 정하지 않은 동작상 한계. 고치면 해당 설계 문서의 규칙과 이 목록을 함께 고친다. 배경은 단계 노트(`docs/notes/R<n>-*.md`)와 `docs/notes/R-port-record.md`.

- 외부 명령 (CC4, 2026-11-08): 큰 힙(원소 수백만)에서 `exec`·`sh`를 띄우면 `fork`가 주소 공간을 잠가 다른 태스크가 17~19 ms 멈춘다(작은 힙은 약 1.3 ms). `posix_spawn`이면 사라지지만, 표준 라이브러리 경로는 자식의 시그널 마스크를 비우지 못하고 열린 파일 한도를 되돌리지 못한다. `libc::posix_spawn`을 직접 쓰는 길은 2026-11-09에 사용자 결정으로 하지 않음(지금대로; 열린 파일 한도 복원을 지킴, 점검 문서 CC4). 실제로 문제가 되면 다시 연다.
- 컬렉션 (L5): 큰 리스트 둘의 이어 붙이기 `a + b`는 짧은 쪽 길이에 비례한다(둘 다 길면 O(n)). O(log n)으로 하려면 영속 벡터를 RRB 트리로 바꿔야 한다(색인이 조금 느려지고 노드 기억을 다시 짜야 함).
  0.5.0의 범위가 아니다(사용자 결정 2026-11-08). 상수 접기(P7, L5)는 모든 리터럴에 하지만, 상수 하나가 `Rc` 하나로 공유되므로 "참조가 하나면 제자리 수정"이 상수에는 닿지 않는다(첫 수정에 복사가 한 번).
- 태스크 (05; R3a·R3b 노트)
  - 형제 태스크의 시작 순서는 약속하지 않는다(05). `fung test`가 순서를 섞어 기대는 시험을 드러내지만, `fung run`에서 순서에 기대는 프로그램은 여전히 조용하다(경쟁 검출기 없음; L14).
  - 한 번에 도는 연산(L6에서 조각으로 나누지 않은 것): 큰 리스트의 `==`·`in`·`+`(둘 다 길 때)·`str`, `flatten`의 한 원소가 긴 리스트일 때 그 원소, 정규식 찾기·바꾸기, `upper`·`lower`, 큰 맵의 해제,
    문자열 전체를 한 번 읽거나 복사하는 연산(`+`, 처음 부른 `len`, `s[i]`·슬라이스가 그 자리까지 읽는 것; 1억 글자 한도 안에서 수십 ms),
    (2026-11-08 VA10: `chars`·`bytes`·`from_bytes`·`pairs`·`merge`·`trim`·`group_by`의 결과 맵 만들기는 조각으로 나눴다. 2026-11-10 VA25에서 Rust로 옮긴 `reverse`·`keys`·`values`·`sum`·`min`·`max`도 조각이다. 200만 원소에서 측정: `==` 20 ms(VA22 뒤 약 9 ms), `in` 9 ms, `+` 23 ms, `str` 37 ms, 9 MB 문자열의 `upper` 30 ms),
    조각으로 나눈 함수가 범위(`0..n`)를 벡터로 펼치는 일, `sort`·`unique`가 끝에서 작업 상태를 한 번에 해제하는 일(50만 원소에 약 0.6 ms),
    `unique`의 첫 걸음이 새 해시 표의 페이지를 건드리는 일. 200만 원소 리스트의 `str`은 약 95 ms 동안 다른 태스크를 세운다(L6 측정).
    기한도 그 연산이 끝난 뒤에 본다. 고치려면 각 연산을 조각으로 나눠야 한다(`vm/pieces.rs`의 `Work`).
  - `swap`의 함수(05 "공유 상태", CC11): 함수는 한 차례 안에 끝나므로, 오래 도는 함수는 끝날 때까지 다른 모든 태스크를 세운다(3,000만 번 꼬리 호출이 약 1.4초).
    기한·신호·취소는 차례 몫(호출 1,000번)마다 본다. 서버의 핸들러 하나가 긴 `swap`을 돌면 모든 연결이 그동안 멈춘다. 고치려면 `f`가 차례를 넘겨도 다시 시도가 끝없지 않게 해야 한다(여러 코어 때 다시 본다).
  - 시험 도우미: `release_fifo`의 막히는 열기는 줄에서 버려진 일이면 떨어진 시험 스레드를 남긴다.
  - 끝없이 도는 태스크(막히지 않는 것)는 다른 태스크들 사이의 진짜 교착을 가린다(05 "종료": 오탐 없는 부분 판정 규칙이 없어 그대로 둠, L14 검토).
  - 외부 명령의 취소(L6): 터미널 앞에서는 명령이 fung과 같은 그룹이라, 취소가 명령 하나만 끝낸다(손자는 남을 수 있음). `setsid`로 빠져나간 손자가 출력을 쥐고 있으면 취소되지 않은 명령은 그것이 끝날 때까지 기다린다(전과 같음).
  - 고리 검사(E0709, L7)는 받는 채널의 끝이 버퍼나 결과에 들어간 적이 있을 때만 버퍼를 건너 따라간다. 그런 채널에 큰 사슬(앞 채널의 끝을 품은 채널 수천 개)을 품은 값을 거듭 보내면 보낼 때마다 그 사슬을 훑는다(표시는 내리지 않는 보수적인 것).
- 메모리·스택 (R1·R4b·R6a 노트, L9)
  - 실행 스레드(모든 태스크가 그 위에서 돈다)는 4 GiB 스택을 예약하고, 거부되는 호스트에서는 1 GiB·256 MiB·64 MiB로 줄여 다시 시도한다(`fung_vm::stack::ENTRY_STACKS`).
    값의 출력·비교·해시·해제와 fung 호출(`map`·`fold`의 콜백 포함)은 깊이와 상관없이 Rust 스택을 쓰지 않는다(L9, R2). 남은 것은 **앞단의 긴 평평한 사슬**(`1 + 1 + …`, `x.b.b…`)이다:
    중첩 한도(E0115)에 세지 않는데 뒤 단계가 고리마다 재귀해, 넘치면 종료 코드 134로 죽는다. 고리 하나에 릴리스에서 약 1 KiB:
    64 MiB 약 6.8만, 256 MiB 약 27만, 4 GiB 400만 고리 통과. 디버그 빌드는 4 GiB에서 45만은 되고 60만에서 넘친다(2026-10-28 측정, L9 결정: 사람이 쓰지 않는 길이라 문서로 둠).
    고친다면 앞단 전 단계(변환·resolve·컴파일러·포매터·`Drop`)를 반복으로 바꾼다. 4 GiB 예약을 줄이면 이 한도가 함께 내려간다.
  - 값 스택의 칸 번호는 프레임 안에 32비트로 담는다(VM16, 프레임 48바이트). 한 태스크의 스택이 2^32칸을 넘으면 E0804가 아니라 `expect` 패닉(`vm/frame.rs` "a stack index within 32 bits")으로 죽는다.
    칸 하나가 16바이트라 그만큼 쌓으려면 약 64 GiB가 필요해 실제로는 닿지 않는다(2026-11-11 VM16에서 정함).
  - 코드가 지키지 않는 불변식(VM36, 주석으로 남김): 이어 가는 내장 함수의 `resume`이 패닉하면 그 함수의 정리(`unwind`)는 돌지 않는다(`vm/call.rs` `Hole`; 패닉은 런타임 버그라 E0801로 끝남).
    `result_kind`는 `Ok`·`Err`가 늘 값 하나라는 것에 기댄다(`vm/m4.rs`, 컴파일러의 E0236과 런타임의 생성 규칙이 지킴). 실패한 `match` 절이 덮어쓴 지역 칸은 컴파일러의 칸 배정 규칙이 읽지 않게 한다(`vm/matcher.rs` `Binder::matches`).
- 문자열과 바이트 (02·06; L4 노트)
  - 문자열은 바이트를 그대로 담는다(L4, 2026-11-07). 글로 다루는 몇 자리만 아직 U+FFFD로 읽는다: REPL에 직접 친 줄(파일과 `:load`는 UTF-8이 아니면 거절),
    UTF-8이 아닌 디렉터리 아래의 모듈 경로(모듈 키·위치가 글이다), 시간대 이름(`TZ`, 표준상 ASCII). 셋 다 값이 되는 바깥 데이터가 아니다.
  - `exec`·`sh`의 `env`는 물려받은 환경에 더해지고, 자식이 보는 환경은 이름순으로 정렬된다. (이름에 `=`가 든 키와 빈 이름은 2026-10-26부터 E0403.)
- 파일 (06 `fs`; R4a 노트, L11)
  - 문자 장치·소켓 파일의 읽기는 아직 풀에서 한다. 끝나지 않는 장치 읽기는 기한에 버려져도 풀 스레드를 쥔다(FIFO는 L11에서 이벤트 루프로 옮김).
    `fs.read`는 글이 문자열 한도를 넘는 순간 멈추므로(`/dev/zero`, 2026-11-11) 메모리는 한도 근처까지만 쓰고, 오지 않는 데이터를 기다리는 장치(tty)만 스레드를 계속 쥔다.
  - macOS에서 FIFO의 쓰는 쪽이 열고 아무것도 쓰지 않고 닫으면 읽는 쪽이 보지 못할 수 있다(06 "FIFO", OS가 알리지 않음).
- 정규식 (06; R4c 노트)
  - 컴파일된 크기가 128 MiB를 넘는 패턴(`[a-z]{1000}` 수천 번, `\pL`을 약 5,000번 넘게 반복)은 E0515 `pattern too large`이고, 실패까지 시간이 걸린다(`\pL` × 20,000이 릴리스 약 3초).
    컴파일은 풀에서 돌아 그동안 다른 태스크는 계속 돈다. 같은 패턴을 동시에 놓친 두 태스크는 각자 컴파일한다.
- 소켓·TLS (06; R4d 노트, L12)
  - 이름 조회(`getaddrinfo`)는 끊을 수 없다(`getaddrinfo_a`는 glibc에만 있고, 직접 쓴 DNS 해석기는 `/etc/hosts`·nsswitch·운영체제의 망별 설정을 잃는다). 취소되거나 시간이 다 된 `connect`는
    바로 돌아오지만 조회 스레드는 해석기가 답할 때까지 남는다. 조회만의 풀(최대 8)에서 돌아 파일 일은 막지 않는다. 같은 이름의 동시 조회를 하나로 합치지는 않는다(L12에서 미룸: 합치려면 풀 밖의 줄이 필요).
  - `close` 없이 버린 TLS 연결은 close_notify를 보내지 않는다(실행 스레드의 해제에서 기다릴 수 없음, `close`는 최대 1초 기다림).
- HTTP (06 "HTTP"; R4d 노트, L13). 아래는 0.5.0 뒤의 **기능**이다(지금 동작은 RFC와 맞는 "안 함").
  - 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축, SOCKS 프록시, https 프록시 너머의 https(TLS 안의 TLS), 101 뒤 연결 넘겨주기(업그레이드·웹소켓),
    본문 흘려 받기·보내기, IDNA(비ASCII 호스트는 퓨니코드로 쓰라는 에러), 서버의 `CONNECT` 2xx 답, 요청 컨텍스트를 취소하는 배경 읽기.
    서버는 연결마다 요청 하나라 HTTP/1.1 응답에 늘 `Connection: close`이고 HTTP/1.0 keep-alive에 `Connection: keep-alive`로 답하지 않는다.
  - 비용(L13 측정): 머리 줄은 네이티브로 읽지만(`_net.read_fields`) 헤더 맵을 만들고 응답 머리를 쓰는 것은 fung이다. 헤더 2만 줄인 응답 하나에 서버와 클라이언트를 합쳐 릴리스 약 0.3초,
    1바이트 청크 10만 개에 약 0.6초(청크는 fung이 읽음).
  - 프록시 환경 변수는 `HTTP_PROXY`·`HTTPS_PROXY`·`NO_PROXY`(와 소문자)만 읽는다. `ALL_PROXY`는 읽지 않는다(curl은 읽음; SOCKS 프록시가 없어 그 값의 흔한 쓰임을 받을 수 없음).
  - `NO_PROXY`의 공백 다듬기는 ASCII 공백(공백·탭·CR·LF)만 본다(환경 변수에 유니코드 공백이 들어가는 쓰임이 없음; curl과 같은지는 확인하지 않음).
  - 서버: macOS는 받기 전 대기 줄(`kern.ipc.somaxconn`, 기본 128)이 차면 새 연결을 끊는다(`connection reset by peer`). 한꺼번에 128개 넘게 몰려오는 접속은 그만큼 실패한다. fung은 시스템이 허락하는 가장 긴 줄을 요청한다. Linux의 기본값은 4096.
- 도구 (08; R5 노트)
  - REPL 세션의 `SourceMap`은 입력마다 파일 하나씩 계속 커진다(앞서 정의한 함수의 에러가 그 입력을 인용해야 해서). 런타임은 같은 글을 나눠 쥔다(`Arc<str>`, 2026-10-26).
  - 화면의 칸은 글자 묶음 단위다(L17). 코드 포인트 단위(`wcwidth`)로 그리는 터미널에서는 ZWJ 이모지 묶음과 VS16 붙은 글자의 칸이 다르게 보인다(어느 쪽을 택해도 어떤 터미널과는 다름).
