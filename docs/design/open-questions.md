# 미결 사항

구현 중 반드시 부딪히지만 아직 정하지 않은 세부 사항을 여기에 적는다.
구현하다가 해당 지점에 닿으면 **추천안과 함께 사용자에게 묻고** 결정 후 해당 섹션 문서를 갱신한다.

2026-10-12~13의 전체 재점검으로 이전 목록은 모두 해당 섹션에 확정되었다.
(람다 표기·`_` 범위·숫자·문자열·식별자 → 01, 맵 사본·튜플 인덱스·`?`와 nil·`Err` 내용·순회 대상·함수 `==` → 02,
`defer` → 03, 여러 채널 기다리기 → 05, `print`·`args`·`exit`·표준 입력 → 06, 0으로 나누기 → 07.)

## 알파에서 써 보며 조정할 것

- [ ] prelude와 모듈 함수 목록 (06). 실제 스크립트 2~3개를 옮겨 보고 더하거나 뺀다.
  - M5 `examples/rename.fung`에서 나온 후보: 부수효과 순회의 블록 문법(`each`에 `fn(): … ;;)`가 거슬림 — 보류, 더 자주 거슬리면 재검토).
    - [x] `fs.list`의 전체 경로 옵션 → `fs.list(dir, full = false)` (06, 2026-10-14).
    - [x] 글자 필터링은 `regex`(M8)로 해결됨: `regex.replace(name, "[^a-z0-9]+", "-")` (2026-10-14).
  - M8에서 나온 후보 (2026-10-14):
    - [x] 키워드를 이름 자리(`.` 뒤, 맵 키, 패턴 키)에서 허용 (01, 2026-10-14). `regex.is_match`는 이름 유지.
    - [x] 맵 리터럴·패턴의 `{key}` 축약 (01·02, 2026-10-14).
  - 알파 샘플(`examples/logsum.fung`·`tidy.fung`·`envcheck.fung`, 2026-10-13)에서 나온 후보:
    - [x] 맵 리터럴의 대문자 키 → 식별자 모양이면 대소문자 무관하게 축약 (02, 2026-10-13).
    - [x] 줄 나누기 → `lines(s)`와 `fs.read_lines(p)` 추가 (06).
    - [x] 람다·`fn` 매개변수에 패턴 허용 (01·03).
    - [ ] 튜플 원소 접근: 매개변수 패턴으로 대부분 해소되어 보류. 알파 스크립트에서 "튜플 변수의 한 원소"가 자주 나오면 `fst`/`snd` 재검토.
    - [x] 명령행 옵션 파싱 → `cli.parse(argv, bools = [])` 모듈 (06, fung 코드).
    - [x] `fung test <dir>`에서 깨진 라이브러리의 에러가 파일마다 반복됨 → 같은 에러는 한 번만, 로드 실패 파일은 `FAIL path: <load>`로
      집계 (08, 2026-10-14).
- [x] 경고 항목 추가 (08): 값 자리의 블록이 대입문으로 끝남 → W0104 (2026-10-14).
- [ ] REPL: 한글 등 폭 2 글자의 커서 위치 — **보류** (2026-10-14): 폰트로는 해결되지 않고(터미널이 칸을 2개 배정) East Asian Width
  표의 축약판이 필요하다. CJK를 REPL에서 쳐 보다 불편하면 그때 넣는다.
  - [x] 기록 파일 저장 `~/.fung_history` (08, 2026-10-14).
  - [x] 런타임 에러로 선언만 된 전역(`x = 1 / 0` 뒤의 `x`)의 처리 → E0808 (M6에서 이미 처리됨, 2026-10-14 확인).

## 구현 중 정할 것

- [ ] 에러 번호의 구체적 할당 (08의 분류 안에서, 카탈로그 파일에).
- [x] `http.serve`의 요청·응답 형태 (M8) → 06 "HTTP" (2026-10-14).
- [x] `time.format`의 토큰 목록 (M8) → 06 "시간 형식" (2026-10-14).
- [x] 파일 핸들을 여러 태스크가 쓸 때의 규칙 → 연산 단위 뮤텍스 (06, 2026-10-14).

## M8 후속 (보류, 2026-10-14)

- [ ] `time`: 12시간제 토큰(`hh`·`A`), 토큰 이스케이프, 달력 산술(`add_months` 등).
- [ ] `http`: 응답 스트리밍·대용량 본문, 쿠키, TLS 서버.
- [ ] `regex`: 외부 입력 패턴을 `Err`로 검사하는 함수(`regex.compile`류).

## 알파 이후

- [x] 프로젝트 extras에서 `_native.*` import 허용 여부 → **허용하지 않는다** (06, 2026-10-14). 측정해 보니 Go 원시 46개 중 44개가
  공개 모듈에 같은 Go 함수 값으로 이미 나와 있어(비용 없음) 열어도 얻는 성능이 없고, 숨김 층을 바꿀 자유만 잃는다(Node의
  `process.binding` 폐기 사례). 사용자가 Go 코드를 붙이는 확장도 열지 않는다. 필요한 Go 원시는 fung 본체에 추가한다.
- [x] 불변 컬렉션의 누적 비용 (2026-10-14 측정): `fold`로 `put(acc, k, v)`를 쌓으면 맵 전체를 매번 복사해 n²이다(2만 개에 18.5초).
  `acc + [x]`(2만 개 0.85초), `acc + s`(0.31초)도 같은 성질. 후보: 영속 자료구조(맵 HAMT·리스트 벡터 트라이), 소유자 확인 후 제자리
  덧붙이기, 일괄 구성 원시(`from_pairs`). → **영속 자료구조로 해결** (M10, 2026-10-14). 맵·리스트 누적과 꼬리 재귀는
  선형(2만 개 13~61ms). 문자열 `acc + s`는 그대로 n²이고, 리스트에 모아 `join`하라고 매뉴얼에 안내한다.

## 배포 후속 (보류, 2026-10-14)

- [ ] Windows 지원: 빌드는 되지만 REPL raw 모드 편집기를 확인하지 않았다. 확인 후 릴리스 대상(`scripts/build-release.sh`)에 추가하고 설치는 zip + PowerShell.
- [ ] Homebrew tap: macOS에서 `brew upgrade`로 업데이트. 그때 GoReleaser로 넘어갈지 함께 정한다.
- [ ] `fung upgrade` 자체 업데이트 명령: 지금은 설치 스크립트를 다시 실행하면 된다.
- [ ] macOS 코드 서명·공증: 브라우저로 받은 바이너리는 Gatekeeper가 막는다(`curl` 설치는 해당 없음). Apple 개발자 계정이 필요하다.

## 태스크 실패 후속 (보류, 2026-10-14)

- [ ] `spawn(f, on_fail: e => …)`: 만들 때 실패 처리기를 붙여, 결과를 버리는 백그라운드 작업의 실패를 GC를 기다리지 않고 즉시 처리.
- [ ] 전역 처리기 `on_task_failure(fn(e): …)`: 확인되지 않은 실패의 기본 동작(보고 + 종료 코드 1)을 바꾸는 도구.
- [ ] 구조적 동시성(`scope`): 태스크 수명을 블록에 묶어 실패가 언제나 범위를 연 코드로 가게 한다. 기존 `spawn`과 함께 둘지 포함해 별도 마일스톤에서 검토.

## 성능 (2026-10-14 측정)

같은 기계에서 4가지 작업(`fib(30)`, 300만 번 산술 반복, 문자열 50만 개 잇기, 맵 20만 개)을 10개 언어로 쟀다. fung 0.1은
기하평균으로 8위(Go·Rust·Swift·Node·Perl·Python·Ruby 다음, Tcl·awk 앞), 호출·산술은 Python의 2~4배 느리고 문자열은
Python과 같으며 시작 시간(6ms)은 Python(22ms)보다 빠르다. fung 쪽 벤치마크는 `bench/`에 둔다.

- [x] 1단계: 산술 디스패치의 Go 맵 조회(모든 명령에서 먼저 수행), 호출마다 다시 만드는 매개변수 표, `fold`·`map` 등이 범위를 리스트로 펼치는 것. → 완료 (2026-10-14)
- [x] 2단계: fung 함수 호출의 할당 0 빠른 길(위치 인자·개수 일치 시 인자를 VM 스택의 프레임으로 바로), 내장 함수가 람다를 부를 때 인자 버퍼 재사용. → 완료 (2026-10-14)
  - 결과(`bench/`, 5회 중 최고): fib 0.513→0.205초(2.5배), loop 0.563→0.241초(2.3배, Python 0.276초보다 빠름), str 0.065→0.053초,
    map 0.492→0.458초(맵은 영속 구조 비용이라 거의 그대로). 할당은 fung 함수 호출 0, 내장 함수 호출 1(인자 배열), `fold` 항목당 0
    (`internal/vm/alloc_test.go`가 지킨다). 남은 시간은 명령 해석 루프 자체(3단계).
- [x] 3단계: 값 표현 변경(정수를 인터페이스 상자에 담지 않기), 정수 전용 명령, 묶음 명령. → **취소** (2026-10-14). Go 구현에서 값 표현을 바꾸는 일은
  Rust 재구현(아래 "Rust 이전")에서 처음부터 `enum`으로 해결되므로 버려질 작업이다.
- [ ] 맵 누적의 transient(Clojure): 아무도 중간 결과를 볼 수 없는 구간(`fold`로 쌓는 중)에서는 내부적으로 제자리 수정. 목표는 Python의 2배 안쪽.
- [ ] 레지스터 기반 VM, 인라인 캐시: 3단계 이후 측정을 보고.
- [ ] JIT: 하지 않는다(추천). Go에서는 실행 메모리·어셈블러 두 벌·GC·고루틴 선점과 충돌하고, 용도(입출력 위주 스크립트)에 비해 비용이 크다.
- [ ] 배포용 단일 실행 파일(`fung build app.fung -o app`, fung 바이너리 + 소스 묶음): 바이트코드 파일(.fungc)보다 추천. 컴파일은 10ms 미만이라 바이트코드 캐시의 속도 이득은 없다.

## Rust 이전 (검토 중, 2026-10-14)

장기 목표로 fung을 Rust로 다시 구현한다. 지금 Go 구현에는 집착하지 않는다. 문법과 사용자에게 보이는 동작이 핵심이고, 내부는 바뀌어도 된다.
C 재구현은 기각: GC·동시성·TLS를 직접 만들어야 하고, `http.serve`처럼 외부 입력을 파싱하는 코드에서 메모리 안전을 잃는다.
그대로 이어지는 것: 설계 문서, 에러 번호, 매뉴얼, `extras/`의 fung 코드, 골든 테스트(구현 독립 적합성 테스트), `bench/`, 릴리스 흐름.

- [x] 동시성과 메모리 모델 → **Erlang VM식 런타임 + 참조 계수.**
  - 불변 값이라 태스크 사이의 "공유"와 "복사"를 프로그램이 구분할 수 없다. 그래서 태스크마다 힙을 따로 두고, 경계(`spawn`이 붙잡은 값,
    채널로 보낸 값, `await` 결과)에서만 복사한다. 태스크 안은 원자적 연산 없는 `Rc`, 그러면서도 태스크들은 진짜 병렬로 돌 수 있다.
  - 태스크 안에서는 순환이 생기지 않으므로 참조 계수만으로 회수가 완전하다. 참조가 하나면 제자리 수정(Roc식, Rust의 `Rc::make_mut`)도 가능하다.
  - 값은 `enum`(종류 태그 + 값 칸, 16바이트). 정수·실수는 상자 없이 칸에 바로 들어간다. 긴 구조의 연쇄 해제는 재귀가 아니라 반복으로 한다.
  - 큰 문자열만 예외로 태스크 사이에 공유한다(Erlang의 refc binary).
  - 스케줄러는 VM이 직접: 태스크를 명령 사이에서 주차·재개하고, 명령 일정 개수마다 차례를 넘기며(Erlang의 reduction), 운영체제가
    비동기로 못 하는 일(표준 입력, 프로세스 대기)은 스레드 풀로. 처음에는 워커 하나로 시작하고, 여러 코어로 늘리는 것은 내부 변경으로 남긴다.
    **3a는 운영체제 스레드 + 실행권 잠금(2026-10-15), 그린 스레드는 4단계 이벤트 루프와 함께 재검토.** 태스크마다 스레드 하나, fung 코드는
    실행권을 쥔 스레드만 돌리고(Python GIL·OCaml 4 모델), 막히는 곳은 `crates/fung-vm/src/vm/block.rs`의 `block` 하나로 모아 갈아끼울 자리를 남겼다.
    측정과 판단 자료는 `docs/notes/R3a-runtime.md` 9절(태스크 생성 약 55 µs로 Go의 약 25배, 채널 주고받기 약 2.5 µs로 약 3배; 태스크 하나일 때는 비용 없음).
    **4d(2026-10-16)에서 다시 미룸**: 그린 스레드는 6단계(Go 삭제) 뒤의 별도 단계로 한다. 아래 "R7 스케줄러" 항목.
    태스크별 힙과 경계 복사도 여러 코어로 갈 때 한다(실행권 아래에서는 값 공유가 안전하다).
  - 람다를 부르는 Rust 내장 함수 안에서 태스크가 막히는 경우("C 경계를 넘는 yield"): 그 경우만 워커째 막고 대체 워커를 띄운다(Go의 syscall 처리 방식).
    3a는 태스크가 스레드째 잠들므로 이 문제가 없다. 그린 스레드를 만들 때(R7) 다시 본다.
- [x] `atom` → **Rust 구현에서 처음부터 없앤다** (2026-10-14). 공유되는 변경 칸을 언어에서 없애 원칙 5·6에 맞추고 순환의 주된 원인을 없앤다.
  상태는 그 상태를 쥔 태스크가 지역 변수로 들고, 다른 태스크는 채널로 편지를 보낸다(Erlang·Elixir). 이 패턴을 감싼 fung 라이브러리
  (Elixir의 `Agent` 같은 것, 이름·API·`update(a, f)`의 `f` 실패를 부른 쪽에 돌려주는 규칙은 만들 때 정함)가 대신한다.
  대가: 읽기·쓰기가 편지 왕복이라 자물쇠보다 느리다. Go 버전은 그대로 두고, 0.x의 호환성 깨짐으로 Rust 버전 CHANGELOG에 적는다.
  고칠 코드: `examples/api.fung`의 요청 카운터, `tests/lang`의 `atoms`·`conc_helpers`·`http_serve`.
  - 완료 (3b, 2026-10-15): `agent(v)`·`swap(a, f)`·`deref(a)`(atom의 이름을 이음, Elixir `Agent`처럼). fung 코드 `extras/rust/agent.fung`(Rust 부팅만 읽음)와
    `_native.conc`의 Rust 전용 도우미. 에이전트는 배경 태스크(프로그램 끝을 막지 않고 교착 집계에 따로 들며 `at_exit` 뒤에 취소)이고, 핸들은 닫개다.
    `f`는 에이전트의 태스크에서 돌고, 실패하면 상태는 그대로 두고 같은 에러를 부른 쪽 `swap(` 위치에서 다시 낸다. 자기 `f` 안의 같은 에이전트 `swap`은 E0704.
    `tests/lang`의 atom 파일은 `# only: go`(`atoms`·`err_atom_reentry`·`conc_atoms`·`http_serve`), `conc_helpers`의 atom 부분은 `conc_atoms`로 떼어 냄.
    예제는 6단계에서 고친다(아래 "Rust 첫 릴리스(0.2.0)에서 바뀌는 것"). 노트 `docs/notes/R3b-agent.md`.
- [x] 채널의 순환 → **유지하고, 채널에 그 채널 자신을 보내면 에러.** 두 채널이 서로를 버퍼에 담은 채 버려지는 간접 순환은 잡지 못한다(읽히지 않은 메시지가 남은 채 버려진 경우라 드묾).
  - 완료 (3b, 2026-10-15): `send(ch, v)`가 `v`를 깊이 훑어 `ch`가 있으면 E0705 `channel sent into itself`(재귀 없이, 공유된 부분은 한 번만). 에이전트 내부 요청은 검사하지 않는다.
    비용은 "거의 없다"가 아니라 **보내는 값의 크기에 비례**한다(아래 3b 목록). 훑지 않는 곳도 아래 3b 목록에.
  - 자동 닫힘 → **에이전트에 한해, 닫개로.** 에이전트 핸들의 마지막 사본이 사라지면 요청 채널을 닫아 에이전트가 `Err('closed)`를 받고 끝난다.
    일반 채널은 보내는 쪽과 받는 쪽이 같은 값이라 자동으로 닫지 않고 지금처럼 `close`로 닫는다(3b 스펙).
- [ ] 언어 층 후보(지금 결정하지 않음): 구조적 동시성(`scope`), 채널 대신 메일박스(태스크 번호 + 골라 받기; 순환이 완전히 사라지지만
  자동 종료를 잃고 link·monitor 설계가 필요).
- [x] 라이브러리 구조 → **언어 핵심은 최소, 공식 라이브러리는 패키지 구조로 만들되 패키지 관리자가 생길 때까지 바이너리에 함께 넣는다.**
  런타임은 파일·프로세스·TCP 소켓·TLS·시간·정규식 같은 네이티브 원시 기능만 주고, HTTP 프로토콜·JSON·`cli` 등은 fung 라이브러리로 쓴다.
- [x] 의존성 정책 → **허용 목록.** 기본은 Rust 표준 라이브러리. 크레이트는 직접 짜면 위험한 것(암호·TLS)과 운영체제 접근에 `unsafe`가
  필요한 것(이벤트 루프·터미널·시그널)만, 이유를 설계 문서에 적고 CI에서 `cargo-deny`·`cargo-audit`로 검사한다. 공부 가치가 있는 것(HTTP, JSON, 시간 서식, `cli`)은 직접 짠다.
  - 허용 후보: `libc`(운영체제 함수), `mio`(이벤트 루프), `rustls` + 루트 인증서(TLS; 암호 부분에 C·어셈블리가 있어 크로스 빌드에 C 컴파일러 필요), `regex`(RE2 계열이라 지금 의미 유지).
    `libc`·`regex`는 4c, `rustls`·`rustls-native-certs`는 4d에서 들였다(아래 표). `mio`는 들이지 않았고 R7에서 판단한다.
  - 정규식 엔진 직접 구현(Thompson NFA)은 나중의 공부 과제로 남긴다.
  - 허용 목록 → **`deny.toml`의 `[bans] allow`가 유일한 원본** (4c, 2026-10-16). 직접 들인 것과 따라오는 것을 모두 적는다. 지키는 장치는 세 겹이다.
    - 저장소 테스트 `crates/fung-cli/tests/deps.rs`(표준 라이브러리만, 도구 없이 `cargo test`에서 돈다): `Cargo.lock`의 모든 패키지(작업 공간 크레이트 제외)가 목록 안인지,
      `crates/fung-front/Cargo.toml`에 의존성 표(`[dependencies]`·`[dev-dependencies]`·`[build-dependencies]`, `[target.*.]`·`[dependencies.이름]` 형태 포함)가 없는지.
    - CI `rust` 작업(Ubuntu만): `cargo-deny` 0.20.2·`cargo-audit` 0.22.2를 `cargo install --locked`로 버전 고정 설치하고 버전 문자열까지 확인한 뒤 `cargo deny check`·`cargo audit`.
    - `deny.toml`: 한 크레이트 여러 버전 금지, `*` 버전 금지(작업 공간 경로 의존성은 `publish = false` + `allow-wildcard-paths`), 라이선스는 실제로 필요한 것만(4c는 `MIT` 하나, 4d에서 `Apache-2.0`·`ISC`·`BSD-3-Clause`를 더함), 출처는 crates.io만.
      **빈 `allow`는 cargo-deny에서 "모두 허용"**이고 비어 있지 않으면 작업 공간 크레이트도 막으므로, 목록에 `fung-front`·`fung-vm`·`fung-cli`를 늘 둔다.
    - **2주 규칙**: 크레이트와 점검 도구는 올라온 지 2주 넘은 버전만 쓴다(최신판이 2주 안 됐으면 `cargo update -p <이름> --precise <그 전 버전>`).
    - 크레이트는 `fung-vm`에만. `fung-front`(앞단)는 0개.

    | 크레이트 | 버전(올라온 날) | 라이선스 | 들인 길 | 왜 직접 짜지 않는가 | 끌고 오는 것 |
    |---|---|---|---|---|---|
    | `regex` | 1.13.1 (2026-07-15) | MIT OR Apache-2.0 | `fung-vm` 직접, 기본 기능 끄고 `std`·`perf` | 선형 시간 엔진(여러 엔진과 최적화)은 크고 틀리기 쉽다. RE2 계열이라 Go `regexp`의 약속(선형 시간, 역참조 없음)이 이어진다. 직접 구현(Thompson NFA)은 공부 과제로 남김 | `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr` |
    | `regex-syntax` | 0.8.11 (2026-06-09) | MIT OR Apache-2.0 | `fung-vm` 직접(`std`만) + `regex`·`regex-automata`가 끌고 옴 | 옮긴 Go 파서(`native/regex_go.rs`)가 HIR을 만들고 찍고 클래스 타입을 쓴다. 어차피 `regex`가 끌고 오는 크레이트라 새 코드가 늘지 않는다 | 없음 |
    | `regex-automata` | 0.4.18 (2026-08-04) | MIT OR Apache-2.0 | `regex`가 끌고 옴 | `regex`의 실제 엔진들 | `aho-corasick`, `memchr`, `regex-syntax` |
    | `aho-corasick` | 1.1.5 (2026-08-03) | Unlicense OR MIT | `regex`·`regex-automata`가 끌고 옴 | 여러 글자열 동시 찾기(패턴의 고정 글자열을 먼저 찾는 가속) | `memchr` |
    | `memchr` | 2.8.3 (2026-07-08) | Unlicense OR MIT | `regex`·`regex-automata`·`aho-corasick`가 끌고 옴 | 빠른 바이트 찾기 | 없음 |
    | `libc` | 0.2.189 (2026-07-21; 0.2.190은 2주 미만이라 고정) | MIT OR Apache-2.0 | `fung-vm` 직접, 유닉스 대상만 | `signal`·`raise`·`pthread_sigmask`(SIGPIPE로 죽기)는 표준 라이브러리에 없고, `sigset_t` 배치가 운영체제마다 달라 손으로 선언하면 틀리기 쉽다. 4d부터 소켓(`native/sock.rs`: 비차단·close-on-exec·`SO_NOSIGPIPE`·`poll`·`getaddrinfo`)에도 쓴다 | 없음 |
    | `rustls` | 0.23.45 (2026-09-14; 공지 아래 참고) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접(4d), 기본 기능 끄고 `ring`·`std`·`tls12` | TLS(암호와 프로토콜 상태 기계)는 직접 짜면 위험하다(의존성 정책). 소켓을 만지지 않는 상태 기계라 비차단 소켓 + `poll` + 취소 깨우개 위에서 돈다 | `ring`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `zeroize`, `once_cell` |
    | `rustls-native-certs` | 0.8.4 (2026-06-01; 0.8.5는 2주 미만) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접(4d) | 운영체제 루트 인증서(macOS 키체인, Linux 인증서 파일)와 `SSL_CERT_FILE`·`SSL_CERT_DIR`을 읽는다. 운영체제마다 다른 저장소를 직접 읽는 것은 플랫폼 코드다 | `rustls-pki-types`, `openssl-probe`(macOS 밖 유닉스), `security-framework`(macOS), `schannel`(Windows) |
    | `rustls-pki-types` | 1.15.1 (2026-07-23) | MIT OR Apache-2.0 | `rustls`·`rustls-webpki`·`rustls-native-certs`가 끌고 옴 | 인증서·키·서버 이름 타입 | `zeroize` |
    | `rustls-webpki` | 0.103.15 (2026-08-21) | ISC | `rustls`가 끌고 옴 | 인증서 사슬·이름 검증(`native/tls.rs`의 `GoOrder`가 공개 부품으로 Go의 검사 순서를 조립) | `ring`, `rustls-pki-types`, `untrusted` |
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

    `OR`로 묶인 라이선스는 하나만 허용되면 된다. 4c까지는 `MIT` 하나로 모두 덮였다. 4d에서 `ring`(`Apache-2.0 AND ISC`, 둘 다 필요), `rustls-webpki`·`untrusted`(`ISC`),
    `subtle`(`BSD-3-Clause`) 때문에 `Apache-2.0`·`ISC`·`BSD-3-Clause`를 더했다. `ring`의 라이선스 칸이 깨끗한 SPDX라 `[[licenses.clarify]]`는 필요 없었다.
    `cargo-deny check`: advisories·bans·licenses·sources 모두 ok, `cargo-audit`: 41개 크레이트에서 무시한 4건 밖의 발견 없음(2026-10-16, 4d Task 2).
  - **보안 공지 정책** (사용자 결정, 2026-10-16): 공지가 우리 설정에 **닿지 않으면** 2주 규칙을 지키고 이유를 적어 무시하며 해제 날짜를 여기서 추적한다.
    **닿으면** 2주를 기다리지 않고 바로 올린다. 무시는 `deny.toml`의 `[advisories] ignore`(이유와 함께)와 `.cargo/audit.toml`(cargo-audit용, 같은 번호) 두 곳에 적는다.
  - [ ] **추적: rustls 공지 4건 해제, 2026-10-28 이후.** RUSTSEC-2026-0346(사용자 세션 저장소의 `Debug` 출력), -0347(ECH 거절 뒤 바깥 서버로 보낸 데이터),
    -0348(brotli 인증서 압축 풀기), -0349(서버 이름별 키 공유 예측). 2026-10-14에 0.23.46의 수정과 함께 나왔고, 넷 다 fung 설정에 닿지 않는다
    (세션 재개 끔, ECH 안 켬, `brotli` 기능 끔, `ring`에 하이브리드(ML-KEM) 그룹이 없음). 2026-10-28 이후 `cargo update -p rustls --precise 0.23.46`으로 올리고
    `deny.toml`의 `ignore` 네 줄과 `.cargo/audit.toml`을 지운다. 그때 새로 따라오는 크레이트가 있으면 이 표와 2주 규칙을 다시 확인한다.
- [x] 정규식 문법 → **Go `regexp/syntax` 파서를 옮긴 것(`crates/fung-vm/src/native/regex_go.rs`)이 fung 정규식 문법의 정의** (사용자 결정, 2026-10-16).
  계획은 `regex-syntax` 구문 트리를 고치는 얇은 층이었으나, 그 방식으로는 Go가 먼저 내는 에러·에러의 문제 부분·`\<`·`[[a]]`처럼 뜻이 다른 패턴을 맞출 수 없었다(노트 `docs/notes/R4c-regex-crates.md` 3절).
  매칭은 `regex` 크레이트가, 받아들이는 문법과 뜻은 이 파서가 정하므로 크레이트를 올려도 문법이 바뀌지 않는다. Go를 지운 뒤에도 남기고 **6단계 뒤에 다시 판단한다.**
  그래서 파서 자체는 아래 "과도기 잔가지"가 아니다(Go의 유니코드 표와 에러 문구 쪽은 잔가지). 무작위 패턴의 Go 기대값(`regex_go_fixtures.rs`)은 Go를 지운 뒤 회귀 검사로 남는다.
- [x] 옮기는 방식 → **같은 저장소에 Rust 코드를 나란히 두고, Go 버전을 정답지로 쓴다.** 동등해지면 릴리스를 Rust로 바꾸고 Go 코드를 지운다.
  - 첫 이식은 명령 체계를 지금과 똑같이 옮겨, 같은 소스의 `fung dump ast`·`core`·`bytecode` 출력이 두 구현에서 글자 단위로 같은지 비교한다.
    명령 설계를 바꾸는 것은 그 다음.
  - 순서: ① 앞단(dump 비교) ② 값·VM, 태스크 하나(동시성 없는 골든 테스트) ③ 런타임(태스크별 힙·스케줄러·채널, `atom` 제거)
    ④ 네이티브 원시 기능(파일·프로세스·시간·정규식·소켓·TLS)과 fung 라이브러리(HTTP 프로토콜은 fung으로 새로 작성) ⑤ 도구(REPL·`fmt`·`test`·`check`)
    ⑥ 동등 도달(골든 테스트 136개 전부, 벤치마크) → 릴리스 전환, Go 삭제.
  - 6단계 전환 직전에 내장 함수 배치를 한 번 검토한다(2026-10-15): prelude에 둘지 모듈(`import`)로 둘지, Rust로 쓸지 fung으로 쓸지.
    Rust 첫 릴리스(0.2.0)는 `atom` 제거로 어차피 호환성이 깨지므로 위치 조정을 함께 넣는다. 원칙: prelude에는 확실한 것만
    (나중에 prelude에 더하는 것은 W0103 경고뿐이라 안전하고, 빼는 것은 호환성 깨짐).
  - 구현 언어 지침(2026-10-15, 사용자 원칙을 다듬음): "빠지면 다른 것을 만들기 힘들어지는 것"이 핵심이다. 함수마다 아래 표로 정한다.
    fung으로 쓸 수 없는 것(OS·외부 세계) → Rust. 핵심이면서 람다를 부르지 않는 순수 계산(문자열, `len`, json 파싱, 정규식) → Rust.
    핵심이면서 람다를 부르는 고차 함수(`map`·`fold`·`sort_by`·`each`) → 지금은 Rust, R7 스케줄러(그린 스레드) 때 다시 판단(4d에서 미룸)
    (Rust 안에서 람다를 부르면 그 사이에 태스크를 멈출 수 없고 Rust 스택을 쓴다). 핵심이 아닌 조합·프로토콜(`take`·`retry`·`pmap`·HTTP) → fung.
    구현 언어 이동은 사용자 코드에 영향이 없어 나중에 바꿔도 된다.
  - 골든 테스트는 에러 출력까지 글자 단위로 비교하므로 Rust 구현도 에러 형식을 똑같이 맞춘다.
  - 1단계(앞단) 완료 (2026-10-15): dump 세 단계·컴파일 에러가 Go와 같음, 계획 `docs/plans/2026-10-14-rust-front-end.md`, 노트 `docs/notes/R1-front-end.md`
  - 2단계(값·VM) 완료 (2026-10-15): `fung run`/`-e`가 Go와 같은 출력·종료 코드(실행 비교 152건, `tests/lang` 136개 중 118개), 제외 18개(동시성 9, 네이티브 모듈 9: `crates/fung-cli/tests/run_parity_skip.txt`; 이 파일과 제외 장치는 4d에서 지웠다, 지난 기록), 계획 `docs/plans/2026-10-15-rust-vm.md`, 노트 `docs/notes/R2-vm.md`
  - 3a단계(동시성, Go와 같은 의미) 완료 (2026-10-15): 태스크·채널·취소·`atom`·교착 감지·꺼내지 않은 실패·어디서든 `exit`가 Go와 같은 출력·종료 코드
    (실행 비교 187건: `tests/lang` 136개 중 127개, `bench/` 6개, `-e` 47개 중 동시성 23개, 실행 방식 7가지; 동시성 31건은 릴리스에서 Rust·Go 각 50번 반복),
    제외는 네이티브 모듈 9개만. 스펙 `docs/superpowers/specs/2026-10-15-rust-runtime-3a-design.md`, 계획 `docs/plans/2026-10-15-rust-runtime-3a.md`, 노트 `docs/notes/R3a-runtime.md`
  - 3b단계(Go와 일부러 갈라지는 언어 변경) 완료 (2026-10-15): `atom` → `agent`(배경 태스크 + 닫개), E0704·E0705, 갈라지는 테스트의 `# only: go|rust` 표시와
    Rust 골든 실행기(`crates/fung-cli/tests/golden.rs`, Go 없이 파일의 기대값과 비교: `tests/lang` 130개 확인, `# only: go` 3개, 네이티브 모듈 9개 건너뜀, 바닥 127).
    실행 비교는 표시된 파일을 빼고 그대로 같음(`tests/lang` 125개; 그때는 네이티브 모듈 제외 목록이 따로 있었고 4d에서 지웠다). 스펙 `docs/superpowers/specs/2026-10-15-rust-agent-3b-design.md`,
    계획 `docs/plans/2026-10-15-rust-agent-3b.md`, 노트 `docs/notes/R3b-agent.md`. 예제·매뉴얼·CHANGELOG는 6단계(아래 "Rust 첫 릴리스(0.2.0)에서 바뀌는 것").
  - 4a단계(`fs`·`path`) 완료 (2026-10-16): `_native.fs` 17개·`_native.path` 6개, 운영체제 에러의 Go 문구, 파일 핸들(`Value::File`, 핸들 잠금, 실행권 밖 입출력,
    마지막 사본에서 닫힘), `fs_and_path`를 Go 대조와 Rust 골든 실행기에 넣음, 3b 취소 노드 누수 수정. 실행 비교 `tests/lang` 131개(네이티브 모듈 8개·`# only: go` 3개 제외),
    Rust 골든 131개, 경로 입력 38개·에러 11개, errno 표 161개(살아 있는 `go run`과 대조), 무작위 `Match` 30만 쌍, 핸들 프로그램 14개가 Go와 같음.
    스펙 `docs/superpowers/specs/2026-10-15-rust-fs-4a-design.md`, 계획 `docs/plans/2026-10-15-rust-fs-4a.md`, 노트 `docs/notes/R4a-fs.md`.
  - 4b단계(`time`·`json`) 완료 (2026-10-16): `_native.time` 8개·`_native.json` 2개, `Value::Time`(Go `time.Time`의 벽시계 표현), TZif 시간대(`native/tz.rs`, Go `initLocal` 순서),
    순서를 지키고 정수·실수를 구분하는 반복 JSON 해석기(Go 1.27 `jsontext` 경로), `time_module`·`time_values`·`json`·`err_json_stringify`·`keyword_names`를 Go 대조와 Rust 골든 실행기에 넣음.
    실행 비교 187건(`tests/lang` 131개), Rust 골든 136개, 시간 프로그램 21개 × `TZ` 9가지 = 189건(`crates/fung-cli/tests/time_parity.rs`),
    JSON 입력 약 150개 + 버퍼 경계 13개·프로그램 36개(`json_parity.rs`)가 Go와 같음. 남은 실행 비교 제외는 `regex`·`http`(3개).
    스펙 `docs/superpowers/specs/2026-10-16-rust-time-json-4b-design.md`, 계획 `docs/plans/2026-10-16-rust-time-json-4b.md`, 노트 `docs/notes/R4b-time-json.md`.
  - 4c단계(`regex`, 첫 크레이트, SIGPIPE) 완료 (2026-10-16): 크레이트 허용 목록 장치(`deny.toml` + `crates/fung-cli/tests/deps.rs` + CI `cargo-deny`·`cargo-audit`),
    `libc`로 표준 출력·에러의 EPIPE에서 Go처럼 신호 13으로 죽기(`crates/fung-vm/src/sigpipe.rs`), Go `regexp/syntax` 파서를 옮긴 Go 의미 층(`native/regex_go.rs`, 위 "정규식 문법" 결정) 위의
    `_native.regex` 5개(`native/regex.rs`, `regex` 크레이트), `regex`·`err_regex`를 Go 대조와 Rust 골든 실행기에 넣음. 실행 비교 189건(`tests/lang` 133개), Rust 골든 138개,
    남은 실행 비교 제외는 `http_serve.fung` 하나. Go 대조 800쌍·프로그램 47개(`regex_parity.rs`), 파서 표 334개·매치 173쌍·무작위 2,000개 + 겹친 반복 600개, SIGPIPE 11개(`sigpipe.rs`).
    스펙 `docs/superpowers/specs/2026-10-16-rust-regex-crates-4c-design.md`, 계획 `docs/plans/2026-10-16-rust-regex-crates-4c.md`, 노트 `docs/notes/R4c-regex-crates.md`.
  - 4d단계(소켓·TLS·fung으로 쓴 HTTP) 완료 (2026-10-16, 커밋 `6bdcab6..4660516`): `_native.net` 20개(최종 수정의 `read_raw_line` 포함; `native/net.rs`·`sock.rs`, 비차단 소켓 + `poll` + 취소 노드의 깨우개 파이프 `vm/watch.rs`,
    `Value::Conn`·`Value::Listener`), rustls TLS 클라이언트(`native/tls.rs`, 위 허용 목록 표), `_native.conc`의 `set_asker`·`describe`, HTTP/1.1 클라이언트·리다이렉트·프록시·`CONNECT` 터널·서버를
    fung으로 쓴 `extras/rust/http.fung`. 06에 "fung HTTP 약속"을 적고 그 안에서 Go와 같다. 지원 안 함(E0805) 장치와 `run_parity_skip.txt`를 지웠다: 이제 구현마다 갈라지는 파일은 `# only:` 표시뿐이다.
    실행 비교 189건(`tests/lang` 143개 중 표시 없는 133개, 빼는 파일 없음), Rust 골든 139개(`http_serve_rust` 포함), `http_parity.rs` 14개(디버그 13개; 클라이언트 프로그램 61개·연결 없는 경우 30개·인자 에러 13개,
    리다이렉트·프록시, `NO_PROXY` 표 23줄, 서버 원시 요청 88개를 Go·Rust 서버에 보내 바이트 비교, 헤더 10초 한도, 바인드 실패, 예제), `tls.rs`(Go TLS 서버로 인증서 문구 비교, macOS·Linux).
    스펙 `docs/superpowers/specs/2026-10-16-rust-net-http-4d-design.md`, 계획 `docs/plans/2026-10-16-rust-net-http-4d.md`, 노트 `docs/notes/R4d-net-http.md`.
- [ ] 1단계(앞단) 후속, 보류한 사소한 항목 (최종 리뷰에서 머지 전 수정 불필요로 분류):
  - 스택 4 GiB 예약이 거부되는 환경(엄격한 `ulimit -v` 등)에서는 시작 시 패닉(GitHub Actions의 Linux·macOS 러너에서는 정상 동작 확인, 2026-10-15). 디버그 빌드는 약 80만 단계 체인에서 여전히 넘침(릴리스는 200만까지).
  - ~~유니코드 근사~~ 해결 (2026-10-15, 2단계 Task 12): `is_print`와 식별자 판정도 아래 `spec/unicode.txt` 표를 쓴다.
  - 비UTF-8 디렉터리 아래 모듈 경로는 Go와 달리 U+FFFD로 표시. 비UTF-8 파일 이름 복원은 단위 테스트만(macOS에서 만들 수 없음).
  - 견고성: `desugar`·`compiler`의 `lit_str`/`lit_index`가 잘못된 리터럴 종류에 빈 값을 돌려줌(Go는 패닉, 사용자 입력으로는 도달 불가), 카탈로그 파싱이 깨진 줄을 조용히 받아들임, `SourceMap::add`의 `as u32`.
  - 성능: 맵 리터럴·매개변수 중복 검사가 O(n²), 렉서가 토큰마다 `String`을 여러 번 만듦, `Display for Node`의 임시 문자열.
  - 테스트 다듬기: 명세 테스트의 맵 순회 순서·`MkdirAll`·읽기 에러 문구, 정답지 비교 실패 이름표가 `-e`에서 잘림, 이른 실패 시 임시 파일 정리, `has_source` 테스트 도우미, Go 테스트 일부의 약한 단언.
  - 2단계 이후: REPL용 `resolve::Options`의 `Globals`·`Redefine`(Go의 `Redefine` 분기 포함), 깊은 입력 테스트를 실제 CLI 스택 크기와 `MAX_DEPTH`에서.
- [ ] 2단계(값·VM)에서 알고 남긴 차이와 결정 (진행 기록 `.superpowers/sdd/2026-10-15-rust-vm/progress.md`):
  - 비UTF-8 프로세스 출력·환경 변수 값은 잘못된 바이트마다 U+FFFD로 바뀐다. Rust 문자열은 UTF-8, Go 문자열은 바이트라서. 문자열을 바이트 기반으로 바꿀 때 다시 본다.
  - 자식 프로세스 환경의 순서: std `Command`는 만든 환경을 정렬하고, Go는 `os.Environ` 순서를 지키며 같은 이름은 마지막 값을 남긴다. 또 `env: {"HOME=z": …}`에서 원래 `HOME`이 남는다. 프로세스 기능을 다시 만드는 4단계(네이티브 원시 기능)에서 다시 본다.
  - ~~닫힌 표준 출력·에러 파이프: Go는 SIGPIPE로 죽고, Rust는 조용히 `exit(141)`.~~ 해결 (4c, 2026-10-16): 표준 출력·에러의 EPIPE에서 `libc`로 SIGPIPE를 기본 동작으로 되돌리고 막힘을 푼 뒤 `raise`(Go `dieFromSignal`과 같은 순서, 그래도 살면 141). 어느 태스크·종료 처리기에서든, SIGPIPE를 무시한 채 시작해도 신호 13으로 죽는다. 다른 파일·자식 표준 입력의 EPIPE는 그대로 에러 값, 자식은 기본 동작으로 시작(std `Command`가 되돌림). `crates/fung-cli/tests/sigpipe.rs`가 Go와 비교.
  - 해결 (Task 12): 유니코드 글자·숫자·대문자·출력 가능 판정(렉서, 로더, 맵 키·문자열 따옴표, `sym`)은 Go가 생성한 `spec/unicode.txt`(`unicode.IsLetter`/`IsDigit`/`IsUpper`/`IsPrint`/`IsSpace`의 구간, Unicode 17.0.0)를 Rust가 읽어 Go와 같다.
  - ~~동시성 내장 함수는 `E0805 not supported yet`~~ 해결 (3a, 2026-10-15): 모두 Go와 같이 동작한다(`sleep`도 취소되면 E0701로 풀림). 실행 비교 제외는 네이티브 모듈 9개가 남았고 4단계에서 하나씩 비교에 넣는다.
  - ~~2단계에 없는 표준 모듈(`fs`·`path`·`json`·`time`·`regex`·`http`)을 `import`하면 직접이든 사용자 모듈을 거쳐서든 그 `import` 위치에서 `E0805 not supported yet: <모듈>`. 진짜 등록 누락은 그대로 E0801.~~
    지난 기록: 4a~4d에서 모듈이 하나씩 들어왔고, 4d에서 `http`까지 돌게 되어 이 장치(`boot::unsupported_module`, `ModuleState.unsupported`)를 지웠다. E0805 번호는 카탈로그에 남는다(Go에 있음).
- [ ] 3a단계(동시성)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-15-rust-runtime-3a/progress.md`, 노트 `docs/notes/R3a-runtime.md` 8절).
  출력이 Go와 갈라지는 것은 모두 시점이나 스케줄에 달린 경우라서 실행 비교에는 결정적인 사례만 넣었다. 3b·Go 제거 때 어느 쪽을 언어 규칙으로 삼을지 05에 적는다.
  - `at_exit` 핸들러 안의 교착: 앞에서 `exit`이 불렸거나 교착이 이미 보고된 뒤라면 Go는 교착 감지가 꺼져 Go 런타임이 죽는다
    ("all goroutines are asleep", 종료 코드 2). Rust는 핸들러마다 따로 판정해 E0702를 보고하고 그 핸들러만 끊은 뒤 다음 핸들러로 가며, 종료 코드는 이미 정해진 값
    (없으면 1). 멈춰 버리는 것보다 낫고 Go의 정상 경로(핸들러의 E0702는 다시 출력하지 않음)와 같은 규칙이다.
  - `at_exit` 핸들러 안의 `exit()`: Rust는 그 핸들러의 취소 노드만 취소한다(그 핸들러와 그것이 띄운 태스크가 끝남). 남은 핸들러는 각자 새 노드에서 항상 돈다.
    Go는 핸들러 컨텍스트 전체를 취소하지만 플래그를 고루틴이 늦게 세워 경쟁 상태다(짧은 핸들러는 대개 돌고 긴 것은 E0701로 끊김).
  - 표준 입력을 기다리는 태스크: Rust는 읽기 전용 스레드가 줄이 필요할 때만 Go처럼 4 KiB씩 읽고 태스크는 실행권 밖에서 줄을 기다리며, 취소되면
    기다림을 그만두고 E0701로 풀린다. 그래서 다른 태스크의 `exit(n)`은 아무도 치지 않는 줄을 기다리지 않고 곧 끝난다(05 "종료"의 "남은 태스크는
    취소 신호를 받는다"). Go의 읽기는 끊을 수 없어 줄이나 EOF가 올 때까지 프로그램이 끝나지 않는다. 자식 프로세스 기다림은 Go와 같이 끊지 않는다.
  - 기다리는 `swap`: Rust는 다른 막히는 연산처럼 `block`을 지나 취소되면 바로 E0701로 풀린다. Go의 기다림(`a.mu.Lock`)은 끊을 수 없어 쥔 쪽이 놓을 때까지
    기다린 뒤 E0701로 끝난다. 어느 쪽이든 `f`는 돌지 않아 최종 결과는 같고 시간만 다르다(쥔 쪽이 `f` 안에서 10초 자면 Go는 10초 뒤, Rust는 바로).
    `exit`이나 취소가 다른 태스크의 `f`를 기다리게 되는 쪽보다 낫다고 판단했다(3a Task 5).
  - atom을 쥔 태스크의 `exit`: 진입 파일이 `swap`으로 그 atom을 기다리는 중이면 Go는 대개 진입 파일의 함수를 돌리지 않지만
    가끔 돌린다(기다림이 취소되지 않는 `a.mu.Lock`이고 취소 플래그를 고루틴이 늦게 세운다). Rust는 항상 취소가 이긴다. 실행 비교에서 뺐다
    (`crates/fung-cli/tests/run_parity.rs`, `vm/exit_tests.rs`).
  - 꺼내지 않은 실패의 보고: Rust는 마지막 핸들이 사라지는 순간(이미 실패했으면 그때, 아니면 실패하는 순간) 보고한다. Go는 GC가 핸들을 찾을 때라서
    표준 출력과 섞인 터미널에서는 `task failed:` 줄이 Go보다 앞에 보일 수 있다. 끝에 남은 실패가 여럿이면 Rust는 실패한 순서로 보고하는데,
    태스크가 대개 띄운 순서대로 실패하므로 띄운 순서가 된다. Go는 고루틴이 흔히 띄운 역순으로 돌아 역순으로 나온다. 실행 비교는 보고가 여럿인 사례를 피했다(아래 "형제 태스크가 시작하는 순서").
  - 자기 자신 취소(`cancel(me)`, `me`는 자기 핸들): Rust는 다음 호출에서 바로 E0701로 끝나 `Err({kind: 'cancelled})`(05 "호출마다 검사"). Go는 `context.AfterFunc`가 플래그를 늦게 세워 대개 이어서 돌고 값을 돌려준다(`vm/task_tests.rs`의 `a_task_that_cancels_itself_stops_at_its_next_call`).
  - 형제 태스크가 시작하는 순서: Rust는 실행권 줄(FIFO `Gate`)에 선 순서라 대개 띄운 순서지만, 운영체제 스레드가 줄에 서는 시점에 달려 보장되지 않는다. Go는 흔히 마지막에 띄운 것이 먼저 돈다
    (예: `spawn(() => exit(5)); spawn(() => exit(6)); sleep(50ms)`은 Rust 5, Go 6). 위의 실패 보고 순서도 같은 차이에서 나온다. 순서에 기대는 프로그램은 두 구현 모두에서 경쟁 상태이고,
    **사용자 `test` 블록이 순서에 기대면 두 구현에서 결과가 다를 수 있다.**
  - 띄운 태스크는 띄운 쪽이 차례를 넘기거나(호출 1만 번) 막히기 전에는 돌 수 없다. 그래서 곧바로 끝나는 태스크가 띄운 자식은 한 번도 돌지 못하고
    부모가 끝날 때 취소된다. Go는 그 자식이 가끔 먼저 돈다.
  - 표준 입력 위치: 4 KiB 넘게 읽은 뒤 같은 입력을 이어 읽는 다른 명령이 보는 위치가 Go와 몇 바이트 다를 수 있다. Go `bufio`는 남은 부분 줄을
    버퍼 앞으로 옮기고 나머지만 채워 읽는 크기가 달라진다. Go 내부 동작이라 따라 하지 않는다(첫 4 KiB까지는 같다: `crates/fung-cli/tests/stdin.rs`).
  - 핸들러가 띄운 태스크가 `outside`(표준 입력·자식 프로세스 대기)에 남아 있으면 살아 있고 막히지 않은 것으로 세므로, 뒤 핸들러의 교착을 알아채지 못한다.
  - 자기 자신을 품은 채널·atom(`send(c, c)`, `swap(a, _ => [a])`, 자기 핸들을 결과로 담은 태스크)은 `Rc` 순환이라 3b까지 해제되지 않고 새어 나간다. Go는 GC가 회수한다.
    3b: atom은 없어졌고 `send(c, c)`는 E0705. 남은 것(두 채널, 태스크 결과, 자기 핸들을 품은 에이전트)은 아래 "3b단계에서 알고 남긴 차이와 한계".
- [ ] 3b단계에서 알고 남긴 차이와 한계 (진행 기록 `.superpowers/sdd/2026-10-15-rust-agent-3b/progress.md`, 노트 `docs/notes/R3b-agent.md` 9절).
  - 꼬리 위치의 `swap`: fung으로 쓴 모든 prelude 함수처럼 부른 쪽 프레임을 대신하므로, `f`의 에러가 `swap(`이 아니라 바깥 호출 위치에서 난다(생략된 꼬리 호출은 호출 경로에 셈).
    꼬리 위치의 E0704도 같다(`swap(a, x => swap(a, y => y))`는 바깥 `swap(`). Go도 자기 fung prelude 함수에서 똑같다. atom은 네이티브라 프레임이 남았다.
    안쪽 위치를 얻으려면 꼬리 호출마다 호출 자리를 프레임에 적어야 해서(VM 변경, 모든 꼬리 호출에 저장 하나) 하지 않았다.
  - 에이전트끼리의 `deref`: 에이전트 b의 `f` 안에서 `deref(a)`를 하는데 a가 b를 기다리고 있으면 진짜 교착이고 E0702로 보고된다.
    겹친 `swap(a, x => swap(b, y => deref(a) + 5))`가 이 모양이다(Go atom은 6, Rust는 E0702).
  - 꼬리 위치가 아닌 E0704의 호출 경로: 위치는 안쪽 `swap(`으로 Go와 같지만, 경로는 바깥 `swap`을 부른 쪽 것(`extras/rust/agent.fung swap`, 사용자 프레임)이고 Go에 있는 `<lambda>` 프레임이 없다.
    `f`가 에이전트의 태스크에서 돌기 때문이다. 겉모습만의 차이로 남긴다.
  - **에이전트 하나 = 운영체제 스레드 하나**(3a의 태스크 모델, 2026-10-15 최종 리뷰). 살아 있는 에이전트 수에 운영체제 스레드 한도가 천장이다.
    `map(0..20000, i => agent(i))`는 약 2.5초 뒤 `E0801 cannot start a task`(Go atom은 0.01초), 만들고 `deref`하기 22,000번은 3.58초(Go 0.012초),
    `swap` 한 번은 약 7 µs(Go 0.4 µs). 지금은 기록만 하고(사용자 결정) 그린 스레드(아래 "R7 스케줄러")로 푼다. 노트 `docs/notes/R3b-agent.md` 2절.
  - `f`는 부른 태스크를 따른다(2026-10-15 사용자 결정, 최종 리뷰 뒤): 부른 태스크가 `exit`·교착 보고·`cancel`로 취소되면 그 `f`도 취소되고 갱신은 반영되지 않는다.
    `f`의 취소 노드는 에이전트 노드와 부른 태스크 노드 양쪽 아래에 걸린다(`CancelNode::also_under`). 꺼내기 전에 부른 쪽이 취소된 요청은 건너뛴다.
    `f`가 띄운 태스크도 부른 태스크와 함께 끝난다(atom과 같음). 처음 스펙의 "꺼낸 요청은 반영"은 이 결정으로 바뀌었다(노트 9절 (7)).
  - 도구를 옮길 때(5단계 `fung test`·REPL, 늦어도 6단계 전환 전): 핸들러를 돌린 뒤 `Runtime::end_background()`를 불러야 한다(`crates/fung-cli/src/run.rs`의 `execute` 순서:
    핸들러 → `end_background` → `wait` → 꺼내지 않은 실패 보고). 빠뜨리면 에이전트가 테스트 파일·REPL 입력 사이에 남거나 `Runtime`이 버려질 때에야 취소된다.
  - 막히지 않는 배경 태스크(끝없이 도는 것)는 다른 태스크들 사이의 진짜 교착을 가린다. 끝없이 도는 보통 태스크도 이미 그렇다.
  - 끝까지 남는 에이전트: 상태가 자기 핸들을 품었거나(`swap(a, _ => a)`), 두 에이전트가 서로의 핸들을 품었거나, 배경 태스크(다른 에이전트의 `f`)가 부른 `f`가 끝나지 않는 에이전트는 프로그램이 끝날 때까지 남는다.
    배경 태스크라 프로그램을 멈추게 하지는 않고 `at_exit` 뒤에 취소된다.
  - 배경 태스크가 띄운 태스크는 배경 종류를 물려받는다(세기·수명). 그래서 에이전트의 `f`가 띄운 태스크가 끝없이 돌아도 프로그램 끝을 막지 않는다. 실패 보고는 남는다.
  - E0705가 훑지 않는 곳: 다른 채널의 버퍼(두 채널 순환), 모듈·클로저의 전역(런타임 끝에 `World::release_values`가 끊음), 태스크 핸들(결과를 보면 에러가 시점에 달림).
  - `send` 비용이 보내는 값의 크기에 비례한다: 정수 100만 개 리스트 한 번에 약 0.75 ms, 원소 하나짜리 리스트 100만 개는 약 4.75 ms(릴리스). 에이전트 내부 요청은 해당 없음.
    같은 영속 벡터를 공유하는 여러 보기는 보기마다 자기 범위를 다시 훑는다(일부러 만든 입력에서는 지수적으로 커질 수 있음).
  - 에이전트는 값이 아니라 `<agent>`로 출력된다(값을 보이려면 에이전트에 물어야 해서 출력이 막히는 연산이 된다). atom은 `<atom 1>`.
  - ~~겹친 `swap`의 취소 노드 누수~~ 해결 (4a Task 0, 2026-10-15): 연결 부모(`also_under`)가 마지막 연결 자식이 떠날 때 끝나지 않아
    `swap(a, x => x + swap(b, fn(y): spawn(() => 1); y + 1 ;;))`마다 노드가 남았다. `CancelNode::leave`가 연결 부모에도 주 부모와 같은 검사를 한다. 노트 `docs/notes/R4a-fs.md` 6절.
- [ ] Rust 첫 릴리스(0.2.0)에서 바뀌는 것 (6단계 CHANGELOG·예제·매뉴얼의 근거, 2026-10-15). 사용자에게 보이는 호환성 깨짐과 새 동작:
  - `atom` → `agent`. 옮겨 쓰기는 `atom(` → `agent(`이고 `swap`·`deref`는 이름 그대로. 타입 표기도 `atom` → `agent`(`fn f(a: agent)`), `type_of`는 `'agent`.
  - `swap(a, f)`의 `f`는 부른 태스크가 아니라 **에이전트의 태스크**에서 돈다. 요청은 받은 순서대로 하나씩. `f`의 실패는 상태를 그대로 두고 부른 쪽 `swap(`에서 같은 에러로 난다.
    `f` 안의 `exit(n)`은 부른 쪽의 `exit(n)`이 되고 에이전트는 계속 일한다. `f` 안(과 `f`가 띄운 태스크)의 `deref`는 바꾸기 전 값.
    부른 태스크가 취소되면(`exit`, 교착 보고, `cancel`) 그 `f`도 취소되고 갱신은 반영되지 않는다(atom과 같은 결과).
  - **수천 개의 살아 있는 칸에는 `atom(` → `agent(`이 그대로 대체되지 않는다.** 에이전트마다 운영체제 스레드가 하나라서, 2만 개를 살려 두면 스레드 한도에 걸려
    `E0801 cannot start a task`로 끝나고, 만들기·`swap`도 atom보다 수십 배 느리다(위 3b 목록의 수치). 값마다 칸을 두던 프로그램은 에이전트 하나에 맵으로 모은다.
    그린 스레드가 들어오면(R7, 6단계 뒤) 풀릴 한계라서 그때 이 항목을 고친다. 0.2.0 릴리스에는 이 한계가 그대로 들어간다.
  - 다른 에이전트를 겹쳐 쓰는 `f`: `swap(a, x => swap(b, y => deref(a) + 5))`는 atom에서 6이었지만 E0702(교착)다. `f` 안의 `deref`가 기다리지 않는 것은 같은 에이전트에서만.
  - 서로의 핸들을 품은 두 에이전트(`b = agent(a)`, `swap(a, _ => b)`)는 핸들이 모두 사라져도 끝나지 않고 프로그램 끝까지 남는다(배경 태스크라 프로그램 끝은 막지 않음). atom에는 해당 없던 일(GC가 회수).
  - 출력은 `<agent>`(atom은 `<atom 1>`처럼 값을 보였다).
  - 새 에러: E0704 `agent called from inside its own swap`(atom의 재진입 에러를 이음), E0705 `channel sent into itself`.
  - 핸들이 모두 사라진 에이전트는 실행 중에 끝난다. 에이전트는 프로그램이 끝나기를 막지 않고 `at_exit` 핸들러에서 쓸 수 있다.
  - 고칠 것: `examples/api.fung`(요청 카운터)·`examples/serve.fung`, `docs/builtins.html`(`atom`·`swap`·`deref` 항목과 서명), `tests/lang`의 `# only: go` 파일(`atoms`·`err_atom_reentry`·`conc_atoms`·`http_serve`)과
    dump 대조의 `ATOM_EXAMPLES`(`crates/fung-cli/tests/parity.rs`). 6단계에서 Go를 지우면 `# only:` 표시도 정리한다.
  - `http` (4d, 06 "fung HTTP 약속"; 약속 밖의 차이):
    - 응답 본문 상한 **64 MiB**(사용자 결정, 2026-10-16). 넘으면 `Err 'network`. Go에는 상한이 없다. 더 큰 응답은 나중에 요청별 `max_body` 같은 옵션으로.
    - 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축 없음. 서버는 연결마다 요청 하나라서 HTTP/1.1 응답에 늘 `Connection: close`가 붙고,
      HTTP/1.0 keep-alive 요청에 `Connection: keep-alive`로 답하지 않는다. 2 KB 넘는 응답 본문도 `Content-Length`로 보낸다(Go는 조각 전송; 클라이언트가 읽는 결과는 같음).
    - 바이너리 본문: UTF-8이 아니면 바이트마다 U+FFFD(4a와 같은 뿌리).
    - SOCKS 프록시(`socks5://`·`socks5h://`)와 `https://` 프록시 너머의 https는 지원하지 않는다(`proxyconnect tcp: socks5 proxies are not supported`,
      `proxyconnect tcp: https through an https proxy is not supported`). Go는 둘 다 지원한다.
    - 서버가 끝내는 중에 읽은 요청: Go는 답 없이 닫고 Rust는 503으로 답한다.
    - Go가 덧붙이는 그 밖의 헤더, 흔하지 않은 에러 문구(아래 "4d단계(http)에서 알고 남긴 차이").
- [ ] Go 동등을 위한 과도기 잔가지 → **이식 중에는 Go와 맞추고, Go를 지운 뒤 따로 걷어낸다** (2026-10-15).
  Go 구현은 빠르게 해 본 첫 시도로 본다. 아래는 언어 설계가 아니라 Go 구현의 우연을 바이트 단위로 따라 하려고 붙인 것들이다.
  걷어낼 때 각 항목의 바른 동작을 설계 문서와 `tests/lang`에 정한 뒤 지운다. 이식 중 새로 붙이는 것도 여기에 더한다.
  - 유니코드 판정 표: Go가 생성하는 `spec/unicode.txt`(`tests/spec_test.go`의 `unicodeSpec`)와 `crates/fung-front/src/unicode.rs`. 식별자 규칙은 그때 정한다(Rust 표준 판정 또는 UAX #31).
  - Go 런타임 메시지 흉내: 2^44개 넘는 범위의 `E0801 ... makeslice: cap out of range`(`crates/fung-vm/src/native/nat.rs`의 `MAX_SLICE`, `prelude.rs`의 `map`), `repeat` 넘침의 `strings: Repeat output length overflow`(`prelude/strings.rs`).
  - `channel(n)`의 Go 할당 한도 흉내: `n`이 2^44 − 6 이상이면 `E0801 ... makechan: size out of range`(`crates/fung-vm/src/conc.rs`의 `MAX_CHANNEL_BUFFER`). Rust는 버퍼를 미리 잡지 않는다.
  - 표준 입력 읽기 단위 4 KiB(`crates/fung-vm/src/vm/stdin.rs`의 `BUFFER`, 명령의 `raw_stdin` 버퍼): Go `bufio`의 기본 크기라서, `read_line` 한 번 뒤 파일 위치가 Go와 같다
    (`crates/fung-cli/tests/stdin.rs`). 필요할 때만 읽는 것은 설계이고, 크기는 Go의 우연이다.
  - 채널 순회는 Go `nat.Items`처럼 **닫힐 때까지 다 받은 뒤** 함수를 부른다(`native/nat.rs`의 `check_items`·`each_until`·`items`). `each(ch, f)`가 값이 올 때마다 `f`를 부르는 편이 자연스럽고, `map(ch, 3)`처럼 함수가 틀려도 먼저 채널을 비우는 것도 Go의 우연이다.
  - Rust의 내장 함수 차이 목록(`crates/fung-front/src/builtins.rs`의 `RUST_PRELUDE_NATIVES_REMOVED`·`RUST_PRELUDE_AGENT`·`RUST_TYPE_RENAMES`·`RUST_MODULE_MEMBERS`, 3b):
    `spec/builtins.txt`는 Go가 만들어 atom을 담으므로, Rust는 차이(네이티브 `atom`·`swap`·`deref` 빠짐, `extras/rust/agent.fung`의 공개 이름 더함, 타입 이름 `atom` → `agent`,
    `_native.conc`의 Rust 전용 멤버)를 이 한 곳에 두고 앞단·부팅·명세 테스트가 함께 읽는다. 6단계에서 명세에 합치고 목록을 지운다. `extras/rust/`를 따로 둔 것(Go의 `embed`가 하위 디렉터리를 넣지 않음)도 그때 정리한다.
  - Go 방식 그대로: 16진 실수 파싱(`prelude/convert.rs`의 `parse_hex_float`), 대소문자 변환의 Go 단순 매핑(`prelude/strings.rs`의 `upper_rune`·`lower_rune`), 실행 파일 형식 사전 검사로 만드는 `exec format error`(`native/process.rs`).
  - 운영체제 에러를 Go 문구로(4a): errno → Go `syscall.Errno.Error()` 표(`crates/fung-vm/src/native/oserr.rs`, macOS·Linux 따로, arm64 한 줄씩 더함)와
    Go `os`의 여러 시스템 호출 함수를 단계 그대로 옮긴 것(`native/fs.rs`의 `mkdir_all`·`remove`·`remove_all`·`rename`·`make_temp_dir`, NUL 바이트 경로의 EINVAL).
    Rust `io::Error`의 문구 대신 Go 문구를 내려고 붙였다. `native/process.rs`도 같은 `reason`을 쓴다.
  - Go `filepath` 규칙과 디렉터리 순서(4a): `Clean`·`Join`·`Base`·`Dir`·`Ext`를 바이트 단위로 옮긴 것(`native/path.rs`; Rust `std::path`는 `..`를 지우지 않고 `.hidden`을 확장자로 보지 않음),
    `list`·`walk`의 이름 바이트 순서 정렬(Go `os.ReadDir`)과 `walk`의 `WalkDir` 동작(루트는 `lstat`, 읽지 못하는 첫 디렉터리에서 시작 경로로 에러), `glob` 결과의 전체 재정렬(`native/fs.rs`).
    경로 정리 규칙과 순서는 그때 언어 규칙으로 정한다.
  - `glob`의 Go `filepath.Match`·`Glob` 우연(4a, `native/glob.rs`): 잘못된 패턴은 매칭이 그 조각에 닿을 때만 드러난다(`a*[`는 `a`로 시작하는 이름이 있어야 에러),
    디렉터리 부분 재귀 한도 10,000, `\` 이스케이프, `!`는 부정이 아님, 읽지 못하는 디렉터리는 조용히 건너뜀. 패턴 문법과 에러 시점은 그때 언어 규칙으로 정한다.
  - 파일 핸들 쓰기 버퍼(4a, `native/file.rs`의 `GoWriter`): Go `bufio.Writer`의 4096바이트 버퍼와 비우는 시점(넘치면 채워서 비움, 버퍼가 비었으면 바로 씀, 첫 쓰기 에러가 남음)을
    단계 그대로 옮겼다. `close` 전에 같은 파일을 읽는 프로그램이 Go와 같은 바이트를 보게 하려는 것이다. 핸들의 표시 `<file "...">`가 fung 문자열 표기가 아니라
    Go `strconv.Quote`(`\x01`, `\u00a0`)인 것도 같다(`value/format.rs`가 앞단의 `fung_front::diag::quote`를 그대로 쓴다). 버퍼 크기·비우는 시점과 표시 형식은 그때 정한다.
  - Go 시간대 탐색 순서(4b, `crates/fung-vm/src/native/tz.rs`): Go `initLocal`·`loadLocation` 그대로. `TZ` 없음 → `/etc/localtime`(이름 "Local"), `""`·`UTC`·`:` → UTC(`Etc/UTC`는 파일을 읽음),
    앞의 `:` 떼기, 상대 이름의 `..`를 막지 않음, `platformZoneSources` 디렉터리 넷(`/usr/share/zoneinfo/`, `/usr/share/lib/zoneinfo/`, `/usr/lib/locale/TZ/`, `/etc/zoneinfo`), 실패하면 조용히 UTC,
    파일 10 MB 한도, `Location` 이름(fung에서는 보이지 않음). `tzset`의 해 구간 근사(윤년에도 `365*day`, `Date`만 씀)와 터무니없는 순간에서의 Go식 감싸기 산술도 같다.
    `Date`의 없는/두 번 있는 벽시계 시각 선택(먼저 추측한 오프셋, 구간을 벗어나면 다시 찾기)은 의미 규칙이지만 Go가 정한 것이다. 시간대를 찾는 규칙은 그때 언어 규칙으로 정한다.
  - 시각 값의 Go 표현(4b, `value/time.rs`): 서기 1년부터의 초(Go `ext`) + 나노초 + 위치, Go `Add`의 붙기(±(2^63-1))·`Sub`의 자르기·`Unix()`의 감싸기를 그대로.
    `time - time`이 정확히 i64 끝값이면 E0503(Go `Sub`의 자른 값을 넘침으로 봄), `time - Dur(i64 최솟값)`이 E0503(Go가 먼저 부호를 뒤집음).
    ISO 출력이 오프셋의 초를 버리고 1분 미만 오프셋을 `+00:00`으로 쓰는 것(Go `appendFormatRFC3339`). Go를 지우면 `i128` 나노초 같은 단순한 표현과 명확한 넘침 규칙으로 바꿀 수 있다.
  - Go 시간 서식·파싱 세부(4b, `native/timefmt.rs`): `format`의 Go `pad(strconv.Itoa(n))`(연도 -1이 `00-1`, `YY`가 부호를 유지), 이름 토큰 에러의 `strconv.Quote` 앞 3바이트(`"ab\xc3"`),
    서식 없는 `parse`의 Go `time.Parse(time.RFC3339Nano)` 일반 경로(느슨한 형태: 시 한 자리, 소수점 대신 쉼표, `+24:00`·`:60`까지의 오프셋, 9자리 넘는 소수 버림),
    Go `time.ParseError` 문구(Go 레이아웃 `2006-01-02T15:04:05.999999999Z07:00`, `time.quote`의 `\xNN`), 숫자 오프셋이 지역 시간대와 같으면 지역 시간대를 붙이는 Go `setLoc` 규칙.
    fung만의 RFC 3339 규칙과 에러 문구는 그때 정한다.
  - Go 1.27 `encoding/json` → `jsontext` 경로(4b, `native/json.rs`): Go 1.27은 `jsonv2` 실험이 기본이라 `Decoder.Token()`이 `jsontext`와 v1 호환 층(`v2_stream.go`)으로 돈다.
    그 토큰 읽기·상태 기계·스캐너 에러 문구, 호환 층의 문구 치환(`transformSyntacticError`, "(expecting…" 자르기)·옛 위치 계산(`hadPeeked`/`hadEOF`)·에러 뒤에도 참인 `More`,
    깊이 한도 10,000, 에러 위치까지의 바이트를 글자 수로 세는 열(BOM·`é`), 공백 뒤 구분자 에러가 앞의 `,` 위치에 나는 것을 옮겼다.
    **읽기 버퍼가 커지는 과정까지 흉내 낸다**(64바이트에서 4096까지 두 배, 3/4 규칙, `invalidatePreviousRead`). 에러 문구가 그 순간 버퍼에 든 바이트로 만들어지므로
    읽기 경계에 걸친 글자가 `'\xc3'`처럼 잘려 보이는 것까지 같게 하려는 것이다. 이 모두가 Go 1.27의 기본 경로에 묶여 있어서, Go를 올리거나 `GOEXPERIMENT=nojsonv2`로 빌드하면
    Go의 출력이 바뀔 수 있고 parity 테스트(`crates/fung-cli/tests/json_parity.rs`)가 그것을 알려 준다. 언어 규칙으로는 "입력 전체를 보고 에러를 말한다"가 자연스럽다.
  - Go json 이스케이프와 출력(4b, `native/json.rs`): `jsontext.AppendQuote`(HTML 이스케이프 끔, JS 이스케이프 켬: `<>&`는 그대로, U+2028·U+2029는 `\u2028`·`\u2029`,
    `\b \f \n \r \t`, 그 밖의 제어 문자는 `\u00XX`, DEL은 그대로),
    (`NaN`·무한대 실수는 지금 만들 수 없다: 넘침은 E0504, `0.0/0.0`은 E0502, `float("nan")`은 거부. 만들어지면 fung 서식 그대로 써서 틀린 JSON이 된다.)
    아주 큰 들여쓰기의 Go `strings.Repeat`·`makeslice` 패닉 문구(2^48 넘으면 `E0801 runtime error: makeslice: len out of range`).
  - Go 정규식의 유니코드 표(4c): Go가 생성하는 `spec/regex_unicode.txt`(`tests/spec_test.go`의 `regexUnicodeSpec`; `unicode.Categories`·`Scripts`·`FoldCategory`·`FoldScript`·`CategoryAliases`와
    모든 `SimpleFold` 궤도, Unicode 17)를 `native/regex_go.rs`가 읽는다(`\pL`·`\p{Greek}`, `(?i)`, `$name`의 이름 글자 판정 `go_letter_or_digit`). 크레이트의 표(`regex-syntax` 0.8.11은 Unicode 16)와
    받아들이는 속성 이름이 다르다. Go를 지우면 유니코드 판정 표(맨 위 항목)와 함께 어디서 표를 얻을지 정한다. 옮긴 파서 자체는 잔가지가 아니다(위 "정규식 문법").
  - Go 정규식 에러 문구(4c, `native/regex_go.rs`): Go `syntax.ErrorCode` 문구와 Go가 고르는 문제 부분(`Expr`), 여러 에러 중 Go가 먼저 내는 것, 노드 재사용으로 낡는 한도 캐시까지.
    엔진의 "너무 큼"은 Go의 `expression too large`로, 그 밖의 엔진 거부는 Go의 `regexp/syntax: internal error`로 바꾼다. fung의 에러 문구는 그때 정한다.
  - Go `regexp`의 반복 규칙(4c, `native/regex.rs`): `Expand`·`extract`(`$1a`는 이름, 앞자리 0이면 이름, 1억 이상 자름, 잘못된 참조는 `$`를 남김), `allMatches`의 빈 매치 규칙(`go_all`),
    `replaceAll`의 다른 빈 매치·전진 규칙, `Split`의 우연(빈 주어에 빈 패턴이면 `[]`, 꼬리 규칙 `end != len(s)`), 패턴 캐시 256개를 통째로 비우기. 빈 매치와 바꿈 문자열 규칙은 그때 언어 규칙으로 정한다.
  - 정규식 기대값 생성기(4c): `tests/regex_fixtures_test.go`(`go test ./tests -run TestRegexFixtures -update`)가 Go로 `crates/fung-vm/src/native/regex_go_fixtures.rs`를 만든다
    (손으로 고른 컴파일 334개·매치 173쌍, PCG(2026, 1016) 무작위 2,000개 + 겹친 반복 600개). Go를 지우면 생성기는 함께 지우고 만들어진 파일은 고정된 회귀 데이터로 남긴다.
  - SIGPIPE의 Go 런타임 세부(4c, `crates/fung-vm/src/sigpipe.rs`): 부모가 SIGPIPE를 무시로 둔 채 띄워도 기본 동작으로 되돌려 신호 13으로 죽는 것(C 프로그램이라면 EPIPE),
    신호로도 안 죽으면 `exit(141)`하는 대비책(Go `dieFromSignal`의 `exit(128+sig)`). 닫힌 표준 출력·에러에서 신호로 죽는 것 자체는 유닉스의 보통 동작이라 남긴다.
  - Go `net` 문구와 주소 규칙(4d, `native/net.rs`·`sock.rs`): `OpError` 형식(`dial tcp …: connect: …`, `read tcp L->R: read: …`, `i/o timeout`,
    `use of closed network connection`, 닫힌 연결의 기한 설정은 `set tcp <local>: …`), `SplitHostPort`·`parsePort` 문구와 `lookup tcp/<x>: unknown port`,
    `getaddrinfo` 결과 → Go 문구(`no such host`), 에러의 `accept4`(Linux)/`accept`(macOS), 빈 listen 호스트 = `[::]` 이중 스택, 빈 dial 호스트 = 로컬 시스템,
    `io.ReadFull`의 `EOF`/`unexpected EOF`, 두 번째 `close`가 에러인 것.
  - Go TLS 문구와 순서(4d, `native/tls.rs`): `x509: …`·`tls: …` 문구, Go `alert.String()` 표 34개(`remote error: …`), `local error: tls: bad record MAC`,
    `GoOrder`의 검사 순서(날짜, 이름, 사슬)와 이름 에러 문구(IP 호스트는 IP SAN), Go 형식의 RFC 3339 시각, `Records`의 Go 끝 규칙(레코드 사이 EOF는 끝, 안쪽은 `unexpected EOF`)과
    첫 레코드 검사, 핸드셰이크 시간 초과의 `context deadline exceeded`, 세션 재개 없음, close_notify는 `close`에서만.
  - Go HTTP 클라이언트 흉내(4d, `extras/rust/http.fung`): `User-Agent: Go-http-client/1.1`(사용자 값이 덮고, 빈 값이면 보내지 않음), `POST`·`PUT`·`PATCH`는 빈 본문에도 `Content-Length`,
    사용자 헤더의 `CanonicalMIMEHeaderKey`·정렬, `url.Error` 형식(`Op "URL": `, 비밀번호 `***`), "transport connection broken" 접두사, TCP 연결 30초 상한,
    **TLS 핸드셰이크 10초 상한과 그 문구 `net/http: TLS handshake timeout`**(`'network`), Go `ReadResponse`·`textproto.ReadMIMEHeader`·`parseTransferEncoding`·`fixLength`·`fixTrailer`·
    `shouldClose`·`fixPragmaCacheControl` 규칙과 문구, 101 밖의 1xx 건너뛰기, 헤더 예산 10 MiB와 문구, Go `chunkedReader`(4096바이트 줄, 16진 규칙, 16 KiB "excess" 검사, 트레일러 규칙),
    `net/url` 해석과 에러 문구의 부분집합, 사용자 정보의 `Authorization: Basic`.
  - Go 리다이렉트·프록시 흉내(4d): 10번 한도와 그 문구(원래 `Location`을 에러 URL로), `Referer` 규칙, 민감한 헤더·본문 헤더 목록과 "첫 호스트나 그 하위 도메인, 한 번 버리면 계속" 규칙,
    홉마다의 Basic 인증, `failed to parse Location header` 문구, **리다이렉트 본문을 2 KB까지 읽고 버리기**(Go가 연결 재사용을 위해 하는 일, 재사용이 없는 Rust에는 필요 없음; 멈춘 본문에서 Go처럼 기다림),
    `Location` 해석 → 본문 버리기 → 한도 검사 순서, 프록시 변수 우선순위와 빈 값 건너뛰기, CGI 안전장치 문구, `parseProxy`의 `http://` 재시도, `localhost`를 쓴 그대로 비교,
    `NO_PROXY` 문법(Go의 CIDR v4/v6 구분, `IP.Equal`), `proxyconnect tcp: ` 접두사, `CONNECT` 요청 바이트(Go `User-Agent` 포함), 이유 문구 에러와 `unknown status code`,
    `CONNECT` 60초 기한, 프록시 http 요청의 `Proxy-Authorization`을 맨 뒤에.
  - Go HTTP 서버 흉내(4d): Go `StatusText` 표와 `NNN status code NNN`, 헤더 순서·형식(정렬·정규화 뒤 `Date`, `Content-Length`, `Connection: close`; 잘못된 이름 버림, CR/LF는 공백),
    **`Date`의 Go `http.TimeFormat` 형식**, 본문 없는 상태(1xx 먼저 보내기, 204·304), `http.Error` 형식(500·503·413·400의 `X-Content-Type-Options: nosniff`와 끝 `\n`),
    Go가 스스로 내는 답(400, `400 Bad Request: <이유>`, 505, 501, 431)과 EOF·시간 초과·읽기 에러의 침묵, 요청 읽기 규칙(`parseRequestLine`, `ValidHostHeader`, 머리 1 MB + 4 KB·값 500개),
    `url.ParseQuery`(1만 쌍 한도, `;`·잘못된 이스케이프 건너뜀), `Expect: 100-continue`·417, `OPTIONS *`를 서버가 답하기, 413·431 뒤 500 ms 읽고 버리기, `accept` 재시도 5 ms → 1 s,
    핸들러 실패 줄 `http: handler failed: …`와 내부 에러 줄 `http: internal error: …`.
  - 바로 위 "2단계에서 알고 남긴 차이"(U+FFFD·환경 순서; 닫힌 파이프 141은 4c에서 해결)와 "3a단계에서 알고 남긴 차이"는 반대 방향(Rust가 Go를 못 따라가거나 일부러 따르지 않은 것)이라 걷어낼 대상은 아니고, 그때 어느 쪽을 언어 규칙으로 삼을지 정한다.
- [ ] 4a단계(fs)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-15-rust-fs-4a/progress.md`).
  - 닫지 않은 핸들: Rust는 마지막 사본이 사라지는 순간 파일을 닫는다(참조 계수). Go는 GC가 거둘 때까지 열려 있을 수 있다. 그래서 핸들 수천 개를 버려도 디스크립터가 새지 않는다.
    두 구현 모두 버리는 핸들의 버퍼는 **비우지 않는다**(Go 동등; `close`만 비운다). 닫지 않고 버린 핸들에 쓴 마지막 4 KiB 미만은 사라진다. 버릴 때 비울지는 그때 언어 규칙으로 정한다.
  - `close(2)`의 에러: `fs.close`·`write`·`append`·`copy` 모두 Rust는 파일을 닫을 때의 에러를 보고하지 않는다(Go는 보고). NFS 같은 파일 시스템에서만 드러난다.
  - 마지막 사본이 사라질 때의 `close(2)`는 실행권을 쥔 채 돈다(`Drop`은 실행권 안에서 일어남). 로컬 파일은 순식간이지만 NFS 같은 곳에서 멈추면 그동안 다른 태스크도 멈춘다.
    고친다면 닫을 파일을 배경 닫기 스레드에 넘긴다. Go는 GC의 finalizer가 따로 닫아서 해당 없음.
  - `remove_all`: 디렉터리 안쪽은 `std::fs::remove_dir_all`에 맡겨서, 깊은 곳에 실패가 여럿 섞이면 보고하는 첫 errno가 Go `removeAllFrom`과 다를 수 있다. 첫 errno 하나만 보고하는 것은 같다.
  - Go 문자열은 잘못된 UTF-8 바이트를 그대로 품어 핸들 표시에서 `\xff`로 보이지만, Rust 경로 문자열은 U+FFFD로 바뀌어 있어 그대로 보인다(2단계 U+FFFD 차이와 같은 뿌리).
  - `fs.read`·`read_lines`·`read_line(h)`는 파일 내용의 잘못된 UTF-8 바이트를 바이트마다 U+FFFD로 바꾼다. Go는 원래 바이트를 그대로 보존한다(같은 뿌리).
- [ ] 4b단계(time·json)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-16-rust-time-json-4b/progress.md`, 노트 `docs/notes/R4b-time-json.md` 7절).
  - Go `loadLocation`의 마지막 수단 `$GOROOT/lib/time/zoneinfo.zip`을 옮기지 않았다. 시스템 zoneinfo 디렉터리에 없는 이름만 해당한다.
    릴리스 Go 바이너리는 `-trimpath`라 GOROOT가 비어 Go도 쓰지 않지만, `-trimpath` 없이 빌드한 parity용 Go는 Rust가 못 찾는 시간대를 찾을 수 있다.
  - Go `Location`의 "지금" 구간 캐시를 옮기지 않았다. 캐시가 맞으면 Go `lookup`은 계산한 구간 대신 캐시 구간을 돌려주는데, 그 구간은 `Date`의 오프셋 재확인에만 쓰인다.
    무작위 6만 건 대조에서 차이는 없었지만, 올해 안에 마지막 전환이 있는 특이한 tzdata에서는 이론상 `Date`가 갈라질 수 있다(벽시계에 따라 달라지는 차이).
  - UTF-8이 아닌 시간대 약어·POSIX 규칙의 이름은 출력할 때 U+FFFD로 바뀐다(Go는 바이트 그대로). 해석은 바이트로 하므로 받아들이고 거부하는 것은 같다. 실제 tzdata는 ASCII다.
  - `now()`에 단조 시계가 없다. Go는 `now()`끼리의 `-`·`==`·`<`에 단조 시계를 쓰므로, 두 `now()` 사이에 시스템 시각이 바뀌면(NTP 보정 등) 결과가 다르다. 경과 시간 재기가 그때 틀릴 수 있다.
  - 값 출력(`value/format.rs`, `print`·`str`)은 아직 재귀다. 2026-10-16에 재 보니 JSON이 만들 수 있는 가장 깊은 값(10,000겹 `[`·`{"a":`, `[{"a":` 5,000번)은
    디버그·릴리스 모두 진입 스레드(4 GiB 스택)와 `spawn`한 태스크(1 GiB 스택)에서 정상 출력된다. `fold`로 만든 백만 겹 리스트도 진입 스레드에서 출력된다(디버그 80초, 릴리스 75초).
    5백만 겹 리스트는 만들고 버리는 것은 되지만 `str`하면 디버그 진입 스레드에서 스택이 넘쳐 프로세스가 죽는다(종료 코드 134). 큰 스택에 기대는 것이므로 출력을 반복으로 바꿀지는 스택 크기를 줄일 때(그린 스레드, R7) 다시 본다.
  - 잘못된 UTF-8 입력: Rust 문자열에 들어올 때 이미 U+FFFD라서 `json.parse` 에러가 Go의 `'\xNN'` 대신 `'�'`를 보인다(4a와 같은 뿌리). 출력은 둘 다 U+FFFD.
  - 2^30~2^48 사이의 들여쓰기: 두 구현 모두 메모리를 잡으려다 죽지만 죽는 출력이 다르다.
  - Rust에만 있는 종류(`agent`, 잡은 에러)는 `stringify`에서 E0513 `cannot convert agent to JSON`·`cannot convert error to JSON`(Go가 채널·함수에 쓰는 기본 분기와 같은 규칙).
- [ ] 4c단계(regex·SIGPIPE)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-16-rust-regex-crates-4c/progress.md`, 노트 `docs/notes/R4c-regex-crates.md` 6절).
  - 아주 큰 패턴: Go의 명령 수 예산(3,355,443)에는 들어가지만 Rust NFA의 크기 한도(128 MiB)를 넘는 패턴(`[a-z]{1000}` × 3,355, `\pL`을 약 5,000번 넘게 반복)은 Go가 받는데 Rust는
    Go 문구 `` error parsing regexp: expression too large: `<패턴>` ``로 실패한다(E0515). 유니코드 클래스마다 UTF-8 오토마타를 만들기 때문이다.
    실패까지 시간이 걸린다(`\pL` × 5,000이 릴리스 1.3초, × 20,000이 릴리스 약 3초·디버그 약 30초). 필요하면 더 싼 사전 추정을 붙인다.
    컴파일은 실행권 밖에서 돈다(4c 최종 수정, `native/regex.rs`의 `compile_pattern`이 `outside`로): 그동안 기다리는 것은 그 패턴을 쓰는 태스크뿐이고 다른 태스크는 계속 돈다.
    Go는 캐시 뮤텍스를 쥔 채 컴파일해서 다른 정규식 호출자도 기다리지만, Rust는 아무도 기다리지 않고 같은 패턴을 동시에 놓친 두 태스크가 각자 컴파일한다(캐시에는 먼저 넣은 것이 남음).
    `regex_tests.rs`의 `other_tasks_run_while_a_pattern_compiles`가 컴파일을 관문에 세워 두고 다른 태스크가 그 관문을 여는지로 확인한다.
  - 깊은 중첩: 약 999단계에서 `regex` 크레이트의 컴파일러가 디버그 빌드의 2 MB 스택을 넘친다. fung의 명령 스레드(4 GiB)·태스크 스레드(1 GiB)는 괜찮고, 테스트는 1 GiB 스레드에서 돈다.
    스택 크기를 줄일 때(그린 스레드, R7) 다시 본다.
  - 엔진이 크기 밖의 이유로 옮긴 패턴을 거부하면 Go의 `regexp/syntax: internal error` 문구가 나온다. Go는 이 에러를 내지 않으며, 테스트로 닿는 경우는 없다.
  - ~~내부 에러 보고(`crates/fung-vm/src/vm/task.rs`의 `eprintln!`)는 표준 에러가 닫혀 있으면 SIGPIPE로 죽지 않고 패닉한다.~~ 해결 (4c 최종 수정): 결과를 버리는 `writeln!`으로 바꿔 표준 에러가 닫혀 있어도 `abort`까지 간다.
  - 4c 보류 (최종 리뷰에서 머지 전 수정 불필요로 분류한 사소한 항목):
    - `native/regex_go.rs`의 HIR 왕복 확인은 `debug_assert!`라서, 옮긴 패턴을 다시 읽은 결과가 다르면 디버그 빌드에서만 패닉한다(릴리스는 그대로 진행). 무작위 패턴 4만 7천 개에서 어긋난 적은 없다.
    - 무작위 기대값(`regex_go_fixtures.rs`)이 Go `math/rand/v2`의 PCG와 `IntN` 결과에 기댄다. Go가 바뀌어 같은 시드에서 다른 패턴이 나오면 `-update`가 파일 전체를 바꾼다(회귀 검사 자체는 고정 파일이라 영향 없음).
    - `crates/fung-cli/tests/sigpipe.rs`의 fifo 시험에서 읽는 쪽 스레드가 시험이 실패하는 길에서는 거둬지지 않고 남는다(시험 프로세스가 끝나면 사라짐).
- [ ] 4d단계(http)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-16-rust-net-http-4d/progress.md`, 노트 `docs/notes/R4d-net-http.md` 8절).
  약속 밖이라 사용자에게 보이는 것은 위 "0.2.0에서 바뀌는 것"의 `http` 항목에 있다. 아래는 그 밖의 세부다.
  - 소켓(Task 1):
    - `set_deadline`은 이미 막힌 호출에 닿지 않는다. 막힌 `poll`은 시작할 때의 기한을 들고 있고, 새 기한은 다음 기다림부터다(Go는 막힌 호출도 깨운다).
    - 호스트 없음: Linux의 Go(순수 Go 해석기)는 `lookup x on <서버>: no such host`, Rust는 시스템 해석기의 `lookup x: no such host`.
    - 이름 붙은 포트(`:http`)를 찾지 않는다(`lookup tcp/http: unknown port`).
    - Go는 두 번째 주소 계열을 300 ms 뒤에 경주시키고(Happy Eyeballs) 시간 제한을 주소마다 나눈다. Rust는 한 기한 안에서 차례로 시도한다. 첫 에러를 보고하는 것은 같다.
    - ~~IPv6 영역 표시: Go `%en0`, Rust `SocketAddr`의 `%4`.~~ 해결 (4d 최종 수정): 영역이 붙은 IPv6 글자는 Go `parseIPZone`처럼 직접 읽어 scope(인터페이스 이름, 아니면 숫자,
      둘 다 아니면 0)를 싣고, 연결·듣기 문구는 쓴 그대로(`dial tcp [::1%lo0]:P`), 소켓에서 얻은 주소는 인터페이스 이름(`[fe80::1%lo0]:P`)으로 Go와 같다.
    - `close`는 바로 `shutdown`하고 디스크립터는 진행 중인 연산이 놓을 때 닫는다. 상대가 보는 동작은 같다.
    - 한 연결에 읽는 쪽 하나, 쓰는 쪽 하나(노트 2절). 두 번째로 기다리는 쪽은 잠금에서 기다리며 취소되지 않는다.
  - TLS(Task 2):
    - Linux의 Go는 `SSL_CERT_FILE`만 있어도 기본 디렉터리(`/etc/ssl/certs` 등)를 함께 읽는다. `rustls-native-certs`는 변수가 하나라도 있으면 변수의 위치만 읽는다(macOS는 같음).
    - 변수 없는 macOS의 Go는 Apple 검증기를 써서 문구가 다르다(100년짜리 인증서는 `not standards compliant`, 825/398일 한도). Rust는 키체인의 루트로 WebPKI 검증, Go의 Linux 문구.
    - 첫 레코드 검사(`first record does not look like a TLS handshake`)가 Go보다 좁다.
    - 치명적 경고를 받은 뒤 쓰기 에러가 Go처럼 "끈적"(sticky)하게 남지 않는다.
    - 이름 불일치 문구의 이름 목록은 webpki의 `Debug` 출력(`DnsName("…")`·`IpAddress(…)`, rustls-webpki 0.103)을 해석해 얻는다. 형식이 바뀌면 `a_name_mismatch_reads_like_go`가 알린다.
    - Go의 "legacy Common Name" 문구를 내지 않는다(webpki는 CN을 보지 않음). 중간·루트 인증서의 만료는 문구가 다를 수 있다(잎 인증서의 날짜만 정확히 맞춤).
      그 밖의 rustls 에러는 `tls: <rustls 문구>`, Go의 `SystemRootsError`는 없다(루트가 하나도 없으면 unknown authority).
    - DNS 이름으로 맞지 않는 호스트(`_` 등)는 `tls: invalid server name <host>`(Go는 보내고 검증에서 실패).
    - close_notify를 기다리지 않고 보낸다(소켓 버퍼가 차 있으면 버려짐). Go는 5초까지 기다린다. ALPN을 보내지 않는다(Go는 `h2, http/1.1`).
  - 클라이언트(Task 3):
    - 인자 에러의 호출 경로에 `extras/rust/http.fung` 프레임이 보인다. Go의 함수는 네이티브라 없다(06은 extras 프레임을 보이게 둔다).
    - 사용자 함수가 `http.*`를 꼬리 위치에서 부르면 그 프레임이 사라져, 에러가 부른 쪽의 부른 쪽 위치를 가리키고 경로에 `get (1 tail calls elided)`가 보인다. fung으로 쓴 모든 라이브러리 함수와 같다(3b `swap`과 같은 뿌리).
    - TCP 연결과 핸드셰이크를 `min(10초, 남은 시간)` 기한 하나로 묶는다(Go는 30초 + 10초).
    - URL: IDN·퓨니코드 없음, 호스트의 `%XX`(0x80 이상) 풀지 않음(그래서 에러 URL의 `%` → `%25` 바꿈도 괄호 IPv6만이 아니라 모든 호스트에 적용되어
      `http://%C3%A9:1/` 같은 입력의 문구가 Go와 다르다. IDN이 없어 어느 쪽이든 연결은 실패하므로 문구 하나의 차이로 둔다), 괄호 IPv6는 "IPv4인가 아닌가"만 검사(잘못된 IPv6는 연결에서 다른 문구로 실패, IPv4 앞자리 0 문구도),
      에러 URL의 조각(fragment)은 다시 이스케이프하지 않음, 출력할 수 없는 비ASCII 글자의 `%q`(Go는 `\u`).
    - 사용자 정보에 비ASCII가 있으면 가린 에러 URL이 `%C3%A9`로 보인다(Go는 `é`).
    - 101 응답은 본문 없음(Go는 연결을 넘겨줌).
    - 헤더 예산을 "줄 바이트 + 2"로 센다(Go는 소켓에서 읽은 양). 경계에서만 다를 수 있다.
    - 서버가 일찍 닫아 쓰기가 실패하면 쓰기 에러를 보고한다(Go는 답을 읽을 수 있으면 읽음).
    - 헤더 폭탄의 비용: 접히거나 끝없는 머리를 fung으로 해석하므로 10 MiB 예산 안에서 40만 줄에 릴리스 약 1초.
    - `need_str_map`(Go의 "map of strings" E0403)은 HTTP와 무관한 인자 검사인데 `_native.net`에 있다. 자리는 6단계 내장 함수 배치 검토 때 다시 본다.
    - ~~IPv6 영역(`%25en0`)이 연결할 때 떨어진다.~~ 해결 (4d 최종 수정): 위 소켓 목록의 영역 항목.
    - 작은 청크의 비용: 크기 줄은 한 번에 읽지만(`read_raw_line`, 4d 최종 수정) 청크마다 fung 해석과 네이티브 호출 세 번이 남아 1바이트 청크 10만 개에 릴리스 약 0.6초(Go는 거의 0).
  - 리다이렉트·프록시(Task 4):
    - 알 수 없는 스킴의 프록시 URL에 포트가 없으면 Rust는 80(Go는 빈 포트로 연결).
    - IDN·퓨니코드 없음(`NO_PROXY`, 같은 사이트 판정).
    - `NO_PROXY`의 공백 다듬기는 ASCII(공백·탭·CR·LF)만. Go `TrimSpace`는 유니코드 공백 전체.
    - `CONNECT` 답은 상태 줄과 헤더 줄만 검사한다. Go `ReadResponse`는 그 답의 `Content-Length`·`Transfer-Encoding` 틀도 거부한다.
    - 프록시 요청의 `ValidHostHeader` 검사(`http: invalid Host header`)가 없다.
    - `CONNECT` 60초 기한을 넘기면 요청 시간 초과 문구(`request to … timed out`)로 보고한다.
    - 프록시는 요청마다 환경에서 고른다(Go는 프로세스마다 한 번). fung 프로그램은 자기 환경을 바꿀 수 없어 보이지 않는다.
  - 서버(Task 5):
    - 이름이 대소문자만 다른 응답 헤더(`{"x-a": .., "X-A": ..}`): Go는 맵 순서라 아무것이나, Rust는 뒤의 것.
    - 핸들러가 넣은 틀 헤더는 쓴 그대로: `Content-Length`는 자동 값을 끄지만 본문은 달라도 다 쓴다(Go는 거부하거나 자르고 기록), `Transfer-Encoding`은 해석하지 않음, 101 전환의 `Connection: upgrade` 세부는 없음.
    - 413: Go는 16 MiB + 1을 먼저 읽고, Rust는 선언된 길이가 넘으면 바로 답한 뒤 500 ms 읽고 버린다(선 위의 바이트는 같고 시점이 다름). 431에서 Go는 FIN을 보내고 기다리지만 Rust는 반쯤 닫기 원시 기능이 없어 읽기만 한다.
    - 431 경계: Go는 bufio 읽기 양, Rust는 줄 바이트 + 2(클라이언트와 같음).
    - 핸들러가 띄운 태스크는 핸들러 아래에 걸려 핸들러와 함께, 서버와 함께 취소되지만 배경 종류다. 프로그램을 붙잡지 않고, 교착 집계에 따로 들며, 꺼내지 않은 실패는 보고된다.
    - `serve` 인자 에러의 호출 경로에 extras 프레임이 보이고, `ready` 에러에는 `_ready_check`·`serve` 두 프레임이 더 있다(5단계 Task 0에서 포매터 결함을 고쳐 `_serve` 감싸개 프레임은 없어짐).
    - 따라 하지 않은 것: `PRI * HTTP/2.0`, `CONNECT`의 2xx 답, 요청 컨텍스트를 취소하는 배경 읽기.
    - 본문 중간에 멈춘 클라이언트는 닫을 때까지 연결 태스크의 스레드 하나를 붙잡는다(Go도 `ReadTimeout`이 없어 붙잡지만 고루틴이라 쌈). R7에서 푼다.
    - 헤더 폭탄의 비용(서버): 요청 머리(1 MB + 4 KB, 값 500개)를 fung으로 해석하므로 꽉 찬 머리 하나에 요청마다 약 0.1~0.25초(Go는 거의 0).
    - 연결 태스크를 띄우지 못하면(스레드 천장, `E0801 cannot start a task`) 그 연결을 답 없이 닫고 표준 에러에 `http: cannot start a connection task: …`를 쓴 뒤
      받기 재시도처럼 물러났다가 계속 받는다(4d 최종 수정; 그 전에는 서버 전체가 멈췄다). Go에는 이런 경우가 없다(고루틴은 늘 뜬다).
  - 테스트: `http_parity`는 프로필마다 약 45초 걸린다. 헤더 10초 한도 시험(`a_slow_client_is_cut_off_after_ten_seconds`)은 시간을 주입하지 않고 실제로 10초를 기다린다.
  - 4d 보류 (시험 부채; 최종 리뷰에서 머지 전 수정 불필요로 분류):
    - `read_all` 쪽 취소 시험은 실패(RED)를 먼저 확인하지 않았다(`read_line` 쪽만 확인).
    - 교착 없음 시험의 `free_port`: 닫은 포트를 다른 프로세스가 그 사이에 잡을 수 있다(드묾).
    - `crates/fung-cli/tests/tls.rs`: 실패하는 길에서 Go TLS 서버 프로세스와 임시 디렉터리를 거두는 정리 장치(`Drop` guard)가 없다(성공하는 길에서만 치움).
    - 시험 없음: `SSL_CERT_FILE`의 깨진 PEM, `SSL_CERT_DIR`만 있는 경우, TLS 읽기 도중의 취소.
- [x] Go 포매터 결함 (M9, 4d Task 5에서 발견): 매개변수에 `fn` 타입 표기가 있는 함수의 블록 몸통을 Go `fung fmt`가 내어쓴다(`fn` 타입을 람다로 읽음). 최소 재현:
  `fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;`의 몸통이 들여쓰기를 잃는다. 4d에서는 `extras/rust/http.fung`를 `serve = _serve(...)` 한 줄 감싸개로 피했다.
  → 5단계 Task 0에서 해결 (2026-10-17, 사용자 승인으로 Go 수정): 들여쓰기 상태 기계(`internal/format/layout.go`)가 타입 이름 `fn`도 `:`를 기다리는 블록 키워드로
  쌓아, 괄호가 닫힌 뒤의 블록 `:`가 그 짝을 찾지 못했다. 이제 `fn` 뒤가 이름이나 `(`일 때만 함수 머리로 본다(`fnHead`). 감싸개를 걷어 `serve`가 본래 함수다.
- [ ] **R7 스케줄러** (6단계 Go 삭제 뒤의 별도 단계, 4d 스펙 결정 2026-10-16). 언어 의미를 바꾸지 않는 내부 교체다. 막히는 곳은 `vm/block.rs`의 `block()`과
  소켓의 `vm/watch.rs`(`Watch`)로 모여 있어 그 자리를 갈아끼운다. 그때 함께 판단할 것:
  - 그린 스레드: 태스크를 운영체제 스레드가 아니라 VM이 주차·재개하는 단위로. 이벤트 루프 크레이트 `mio`를 들일지(4d에서는 들이지 않음).
  - 태스크 생성 비용: 지금 태스크 하나에 약 55 µs(Go 약 2 µs, 약 25배; `docs/notes/R3a-runtime.md` 9절). 채널 주고받기 약 2.5 µs(Go 약 3배).
  - 스레드 천장: 살아 있는 태스크·에이전트마다 운영체제 스레드 하나라서 프로세스 스레드 한도(macOS `kern.num_taskthreads` 8192)가 천장이다.
    `map(0..20000, i => agent(i))`는 `E0801 cannot start a task`, 에이전트 `swap` 한 번은 약 7 µs(Go atom 0.4 µs; 위 3b 목록).
    HTTP 서버는 천장에 닿아도 죽지 않지만(4d 최종 수정), 스레드가 바닥난 동안 새 연결은 운영체제 대기열에서 거절되거나 답 없이 바로 닫힌다.
  - 람다를 부르는 고차 내장 함수(`map`·`fold`·`sort_by`·`each`)의 구현 언어: Rust 안에서 람다를 부르면 그 사이에 태스크를 멈출 수 없고 Rust 스택을 쓴다(위 "구현 언어 지침").
  - 스택 크기: 지금은 진입 스레드 4 GiB, 태스크 스레드 1 GiB 예약에 기댄다(깊은 정규식 컴파일, 깊은 값 출력). 그린 스레드는 작은 스택이 전제라 다시 정한다.
  - 값 출력의 재귀(`value/format.rs`, 위 4b 목록): 스택을 줄이면 반복으로 바꿔야 할 수 있다.
  - 멈춘 HTTP 클라이언트 하나 = 스레드 하나(위 4d 서버 목록).
  - 막히는 기다림마다 깨우개 파이프 하나(디스크립터 2개, 막힐 때만 만들고 호출이 끝나면 닫음). 이벤트 루프가 생기면 하나로 모을 수 있다.
  - 이름 조회 스레드: `getaddrinfo`는 끊을 수 없어 스레드에서 돈다. 취소되거나 시간이 다 된 `connect`는 바로 돌아오지만 조회 스레드는 해석기가 답할 때까지 남는다.
    시간으로는 해석기 제한에 묶이지만 개수로는 묶이지 않는다(응답 없는 해석기에 취소된 `connect`를 많이 하면 그만큼 남음).
- [ ] 3단계(런타임)가 바꿔야 할 Rust 구조 (2단계 최종 리뷰, 2026-10-15):
  - 해결 (3a): 런타임을 `World`(공유, 실행권 아래)와 `Task`(태스크별)로 나눴고, 기본값은 `Lit`, 내장 함수 표는 프로세스에 하나(`OnceLock`)다. 아래는 그 전의 기록.
  - 런타임 빌림: `Task<'rt>`가 `&'rt mut Runtime`을 쥔다(`crates/fung-vm/src/vm.rs`). 태스크 여럿이 주차된 스케줄러에서는 각자 배타적으로 빌릴 수 없으므로
    `Runtime`을 공유 부분(스트림, 종료 핸들러, 종료 코드, 모듈)과 태스크별 부분으로 나눈다. 내장 함수는 이미 `t.stdout()`·`t.at_exit`·`t.call`로만 런타임을 쓰므로 고칠 범위는 그 안에 머문다.
  - 워커 하나(먼저 할 것)는 지금 구조로 된다. 여러 코어는 그 다음 단계이고, 그때 세 가지를 바꾼다.
    - 내장 함수는 `&'static`(스레드 지역, 누수로 만든 것)이고 `NativeParam.default`가 `Rc`를 품은 `Value`다. 기본값을 `Send`인 것(리터럴 enum 등)으로 바꾼다.
    - `RtProto.consts`에 코드와 링크된 `Rc` 값이 섞여 있다. 코드와 링크된 상수를 나누지 않으면 태스크 경계를 넘는 클로저 복사가 코드까지 깊이 복사한다.
    - 클로저·내장 함수·모듈은 포인터로 비교한다(`value/compare.rs`). 워커 사이에 복사한 뒤에도 `print == print` 같은 동일성이 유지되도록 다시 짝지어 준다.
- [x] 배포 → **플랫폼별 CI 러너**(TLS 암호 부분의 C 코드 때문에 대상용 C 컴파일러가 필요; Linux x86_64·ARM64, macOS ARM64·x86_64 러너 각자 빌드),
  Linux는 musl 정적 바이너리, CI 컴파일 캐시. 릴리스 파일 이름·`SHA256SUMS`·빌드 증명·설치 스크립트·CHANGELOG 릴리스 노트는 그대로.
  `go install` 경로는 사라지고 `cargo install`이 생긴다.
- [x] 버전 → Rust 첫 릴리스는 `0.2.0`(`atom` 제거가 호환성 깨짐). 1.0은 이식 뒤에 정한다.
- [ ] 일정: 정하지 않음. 지금은 공부를 겸한 검토 단계.

