# R 이식 기록: "Rust 이전" 절

이 노트는 `docs/design/open-questions.md`의 "Rust 이전" 절에 있던 이식 기록을 6c-7(2026-10-18)에 그대로 옮긴 것이다.
Go로 먼저 만든 fung을 Rust로 옮기며(1~6c단계) 내린 결정, 단계별 완료 기록, Go와의 대조 수치, 일부러 갈라진 것과 해결된 항목이 들어 있다.
지금도 유효한 규칙과 남은 일, 알려진 한계는 open-questions의 "Rust 구현: 정한 것과 남은 일" 절로 옮겨 fung의 말로 다시 썼다. 이 노트는 기록이라 고치지 않는다.
아래 본문의 "아래"·"위" 같은 가리킴은 옮기기 전의 절 안을 가리키고, 설계 문서 06의 파일 이름은 지금 `06-native-and-stdlib.md`다.

## Rust 이전 (검토 중, 2026-10-14)

**6b(2026-10-17)에 Go 구현을 지웠고 Rust가 유일한 구현이다.** 아래는 이식의 결정과 기록, 그리고 남은 일(6c 할 일, R7 스케줄러)이다.
처음 결정(2026-10-14): 장기 목표로 fung을 Rust로 다시 구현한다. 그때의 Go 구현에는 집착하지 않는다. 문법과 사용자에게 보이는 동작이 핵심이고, 내부는 바뀌어도 된다.
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
    | `regex` | 1.13.1 (2026-07-15) | MIT OR Apache-2.0 | `fung-vm` 직접, 기본 기능 끄고 `std`·`perf`·`unicode`(6c-4: 문법이 이 크레이트의 것이고 유니코드 클래스를 쓴다; 릴리스 실행 파일 +490 KB, 약 9%) | 선형 시간 엔진(여러 엔진과 최적화)은 크고 틀리기 쉽다. RE2 계열이라 선형 시간·역참조 없음의 약속이 이어진다. 직접 구현(Thompson NFA)은 공부 과제로 남김 | `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr` |
    | `regex-syntax` | 0.8.11 (2026-06-09) | MIT OR Apache-2.0 | `fung-vm` 직접(`std`·`unicode`) + `regex`·`regex-automata`가 끌고 옴 | 잘못된 패턴의 에러 종류를 한 줄로 쓰려고 패턴을 먼저 파싱한다(`native/regex.rs`). 어차피 `regex`가 끌고 오는 크레이트라 새 코드가 늘지 않는다 | 없음 |
    | `regex-automata` | 0.4.18 (2026-08-04) | MIT OR Apache-2.0 | `regex`가 끌고 옴 | `regex`의 실제 엔진들 | `aho-corasick`, `memchr`, `regex-syntax` |
    | `aho-corasick` | 1.1.5 (2026-08-03) | Unlicense OR MIT | `regex`·`regex-automata`가 끌고 옴 | 여러 글자열 동시 찾기(패턴의 고정 글자열을 먼저 찾는 가속) | `memchr` |
    | `memchr` | 2.8.3 (2026-07-08) | Unlicense OR MIT | `regex`·`regex-automata`·`aho-corasick`가 끌고 옴 | 빠른 바이트 찾기 | 없음 |
    | `libc` | 0.2.189 (2026-07-21; 0.2.190은 2주 미만이라 고정) | MIT OR Apache-2.0 | `fung-vm` 직접, 유닉스 대상만 | `signal`·`raise`·`pthread_sigmask`(SIGPIPE로 죽기)는 표준 라이브러리에 없고, `sigset_t` 배치가 운영체제마다 달라 손으로 선언하면 틀리기 쉽다. 4d부터 소켓(`native/sock.rs`: 비차단·close-on-exec·`SO_NOSIGPIPE`·`poll`·`getaddrinfo`)에도 쓴다 | 없음 |
    | `rustls` | 0.23.45 (2026-09-14; 공지 아래 참고) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접(4d), 기본 기능 끄고 `ring`·`std`·`tls12` | TLS(암호와 프로토콜 상태 기계)는 직접 짜면 위험하다(의존성 정책). 소켓을 만지지 않는 상태 기계라 비차단 소켓 + `poll` + 취소 깨우개 위에서 돈다 | `ring`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `zeroize`, `once_cell` |
    | `rustls-native-certs` | 0.8.4 (2026-06-01; 0.8.5는 2주 미만) | Apache-2.0 OR ISC OR MIT | `fung-vm` 직접(4d) | 운영체제 루트 인증서(macOS 키체인, Linux 인증서 파일)와 `SSL_CERT_FILE`·`SSL_CERT_DIR`을 읽는다. 운영체제마다 다른 저장소를 직접 읽는 것은 플랫폼 코드다 | `rustls-pki-types`, `openssl-probe`(macOS 밖 유닉스), `security-framework`(macOS), `schannel`(Windows) |
    | `rustls-pki-types` | 1.15.1 (2026-07-23) | MIT OR Apache-2.0 | `rustls`·`rustls-webpki`·`rustls-native-certs`가 끌고 옴 | 인증서·키·서버 이름 타입 | `zeroize` |
    | `rustls-webpki` | 0.103.15 (2026-08-21) | ISC | `rustls`가 끌고 옴 | 인증서 사슬·이름 검증(rustls의 WebPKI 검증기, `native/tls.rs`) | `ring`, `rustls-pki-types`, `untrusted` |
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
- [x] 정규식 문법 → **다시 판단 (6c-4, 2026-10-18 사용자): `regex` 크레이트의 문법이 fung 정규식 문법이다**(RE2 계열, 유니코드 클래스). 아래는 2026-10-16의 첫 결정(Go 파서를 옮김)의 기록이다.
  첫 결정: Go `regexp/syntax` 파서를 옮긴 것이 fung 정규식 문법의 정의 (사용자 결정, 2026-10-16).
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
    Rust 골든 131개, 경로 입력 38개·에러 11개, errno 표 161개(그때 Go 프로그램을 실행해 대조), 무작위 `Match` 30만 쌍, 핸들 프로그램 14개가 Go와 같음.
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
    fung으로 쓴 `extras/rust/http.fung`. 06에 "fung HTTP 약속"을 적고 그 안에서 Go와 같다. 지원 안 함(E0805) 장치와 `run_parity_skip.txt`를 지웠다: 그때부터 6b까지 구현마다 갈라지는 파일은 `# only:` 표시뿐이었다.
    실행 비교 189건(`tests/lang` 143개 중 표시 없는 133개, 빼는 파일 없음), Rust 골든 139개(`http_serve_rust` 포함), `http_parity.rs` 14개(디버그 13개; 클라이언트 프로그램 61개·연결 없는 경우 30개·인자 에러 13개,
    리다이렉트·프록시, `NO_PROXY` 표 23줄, 서버 원시 요청 88개를 Go·Rust 서버에 보내 바이트 비교, 헤더 10초 한도, 바인드 실패, 예제), `tls.rs`(Go TLS 서버로 인증서 문구 비교, macOS·Linux).
    스펙 `docs/superpowers/specs/2026-10-16-rust-net-http-4d-design.md`, 계획 `docs/plans/2026-10-16-rust-net-http-4d.md`, 노트 `docs/notes/R4d-net-http.md`.
  - 5단계(도구: `check`·`test`·`fmt`·REPL·`version`·`help`) 완료 (2026-10-17, 커밋 `02765ea..2a5472a`와 문서): Rust CLI가 Go CLI의 명령 전체를 같은 출력·종료 코드로 제공한다.
    경고 W0101~W0104(`resolve/warn.rs`), `fung test`(파일마다 런타임, `Runtime::hold`·`release`), 포매터(`crates/fung-front/src/format.rs` + `format/`), REPL(`crates/fung-cli/src/repl/`, 터미널 도우미 `fung_vm::term`).
    먼저 Go 포매터를 고쳤다(아래 "Go 포매터 결함" 두 항목). Go 대조 `crates/fung-cli/tests/tools_parity.rs` 20개(저장소 `.fung` 전부 × 흐트러뜨린 변형 21가지의 `fmt`, `check`·`test` 고정 파일과 저장소,
    파이프 REPL 세션 33개 + 잘못된 바이트 하나, 경로 순회), 가짜 터미널 `repl_tty.rs` 4개(편집기 화면 바이트·기록 파일을 Go와 비교), Rust `fung fmt --check`로 저장소를 검사하는 `fmt_repo.rs`.
    일부러 갈라진 것과 남은 차이는 아래 "5단계" 항목. 스펙 `docs/superpowers/specs/2026-10-17-rust-tools-5-design.md`, 계획 `docs/plans/2026-10-17-rust-tools-5.md`, 노트 `docs/notes/R5-tools.md`.
  - 6a단계(릴리스 전환 준비, Go는 정답지로 남김) 완료 (2026-10-17, 커밋 `8088d72..`와 문서): `fan_in`·`pmap`·`retry`를 Rust에서 `conc` 모듈로,
    예제·매뉴얼을 Rust(0.2.0) 기준으로(매뉴얼 함수 참조를 Rust의 매개변수 표와 비교하는 `crates/fung-cli/tests/manual.rs`, 매뉴얼 예제는 Rust 혼자 `check`·`fmt --check`·실행),
    Rust 릴리스 파이프라인(Linux musl 정적, 플랫폼별 러너, 게시 없는 시험 실행, 태그 푸시에서만 빌드 증명, 아키텍처 검사), CHANGELOG `[Unreleased]`, Go·Rust 벤치마크(`bench/README.md`).
    결정과 6b 할 일은 아래 "6a단계" 항목. 스펙 `docs/superpowers/specs/2026-10-17-rust-release-prep-6a-design.md`, 계획 `docs/plans/2026-10-17-rust-release-prep-6a.md`, 노트 `docs/notes/R6a-release-prep.md`.
  - 6b단계(Go 삭제) 완료 (2026-10-17, 커밋 `fb3d326..`와 문서): Go 구현(`cmd/`·`internal/`·`go.mod`·Go 테스트·CI의 Go 작업)을 지웠고 Rust가 유일한 구현이다.
    Go와 대조하던 테스트는 **기대 출력 파일**(`crates/fung-cli/tests/expected/`, `crates/fung-vm/tests/expected/`, 사례 3,570개)로 얼렸고, 명세 파일은 Rust 표에서 생성한다.
    `# only:` 표시·`extras/rust/`·Rust 전용 내장 함수 차이 목록·`FUNG_REQUIRE_PARITY`가 없어졌다. 결정과 남은 것은 아래 "6b단계" 항목.
    스펙 `docs/superpowers/specs/2026-10-17-rust-go-removal-6b-design.md`, 계획 `docs/plans/2026-10-17-rust-go-removal-6b.md`, 노트 `docs/notes/R6b-go-removal.md`.
    다음은 **6c**(아래 "6c 할 일": Go 흉내 동작과 이름을 걷어내고 기대 출력을 갱신), 그 뒤 0.2.0 릴리스.
- [ ] 1단계(앞단) 후속, 보류한 사소한 항목 (최종 리뷰에서 머지 전 수정 불필요로 분류):
  - ~~스택 4 GiB 예약이 거부되는 환경(엄격한 `ulimit -v` 등)에서는 시작 시 패닉~~ 해결 (6a 마무리, 2026-10-17): 진입 스레드 4 GiB → 1 GiB → 256 MiB → 64 MiB, 태스크 1 GiB → 256 MiB → 64 MiB로 줄여 다시 시도한다(`fung_vm::stack`). 모두 거부되면 진입은 `error: cannot start the fung command thread: …` 한 줄과 종료 코드 1, 태스크는 E0801. 디버그 빌드는 약 80만 단계 체인에서 여전히 넘침(릴리스는 200만까지).
  - ~~유니코드 근사~~ 해결 (2026-10-15, 2단계 Task 12): `is_print`와 식별자 판정도 아래 `spec/unicode.txt` 표를 쓴다.
  - 비UTF-8 디렉터리 아래 모듈 경로는 Go와 달리 U+FFFD로 표시. 비UTF-8 파일 이름 복원은 단위 테스트만(macOS에서 만들 수 없음).
  - 견고성: `desugar`·`compiler`의 `lit_str`/`lit_index`가 잘못된 리터럴 종류에 빈 값을 돌려줌(Go는 패닉, 사용자 입력으로는 도달 불가), 카탈로그 파싱이 깨진 줄을 조용히 받아들임, `SourceMap::add`의 `as u32`.
  - 성능: 맵 리터럴·매개변수 중복 검사가 O(n²), 렉서가 토큰마다 `String`을 여러 번 만듦, `Display for Node`의 임시 문자열.
  - 테스트 다듬기: 명세 테스트의 맵 순회 순서·`MkdirAll`·읽기 에러 문구, 정답지 비교 실패 이름표가 `-e`에서 잘림, 이른 실패 시 임시 파일 정리, `has_source` 테스트 도우미, Go 테스트 일부의 약한 단언.
  - 2단계 이후: ~~REPL용 `resolve::Options`의 `Globals`·`Redefine`(Go의 `Redefine` 분기 포함)~~ 해결 (5단계 Task 4, 2026-10-17: `globals`·`redefine`, 진입 파일에만), 깊은 입력 테스트를 실제 CLI 스택 크기와 `MAX_DEPTH`에서.
- [ ] 2단계(값·VM)에서 알고 남긴 차이와 결정 (진행 기록 `.superpowers/sdd/2026-10-15-rust-vm/progress.md`):
  - 비UTF-8 프로세스 출력·환경 변수 값은 잘못된 부분마다 U+FFFD 하나로 바뀐다(6c-3: 유니코드 권장 방식). Rust 문자열은 UTF-8, Go 문자열은 바이트라서. 문자열을 바이트 기반으로 바꿀 때 다시 본다.
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
  - ~~도구를 옮길 때(5단계 `fung test`·REPL, 늦어도 6단계 전환 전): 핸들러를 돌린 뒤 `Runtime::end_background()`를 불러야 한다(`crates/fung-cli/src/run.rs`의 `execute` 순서:
    핸들러 → `end_background` → `wait` → 꺼내지 않은 실패 보고). 빠뜨리면 에이전트가 테스트 파일·REPL 입력 사이에 남거나 `Runtime`이 버려질 때에야 취소된다.~~
    해결 (5단계, 2026-10-17): `fung test`는 **파일마다**, REPL은 **세션 끝**(`:quit`, 입력 끝, `exit(n)`)에 한 번 같은 순서로 부른다.
    REPL을 입력마다가 아니라 세션 끝으로 한 것은 사용자 결정이다. 입력 사이에 정의가 남으므로 전역에 담은 에이전트도 남아야 하고,
    입력마다 부르면 배경 루트가 취소되어 다음 입력부터 모든 에이전트가 처음부터 취소된 채 시작한다. 아무도 붙잡지 않은 에이전트는 핸들이 사라질 때 끝난다(아래 "5단계" 항목).
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
    → 6a에서 예제·매뉴얼·CHANGELOG를 고쳤고 `ATOM_EXAMPLES`는 `RUST_ONLY_EXAMPLES`(`crates/fung-cli/tests/common/mod.rs`)로 바뀌었다. `# only:` 정리는 6b에서 했다(표시와 장치를 지움).
  - `fan_in`·`pmap`·`retry`는 prelude가 아니라 `conc` 모듈(6a, 아래 "6a단계" 항목). 이름 그대로 부르면 E0204.
  - `http` (4d, 06 "fung HTTP 약속"; 약속 밖의 차이):
    - 응답 본문 상한 **64 MiB**(사용자 결정, 2026-10-16). 넘으면 `Err 'network`. Go에는 상한이 없다. 더 큰 응답은 나중에 요청별 `max_body` 같은 옵션으로.
    - 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축 없음. 서버는 연결마다 요청 하나라서 HTTP/1.1 응답에 늘 `Connection: close`가 붙고,
      HTTP/1.0 keep-alive 요청에 `Connection: keep-alive`로 답하지 않는다. 2 KB 넘는 응답 본문도 `Content-Length`로 보낸다(Go는 조각 전송; 클라이언트가 읽는 결과는 같음).
    - 바이너리 본문: UTF-8이 아니면 잘못된 부분마다 U+FFFD(4a와 같은 뿌리).
    - SOCKS 프록시(`socks5://`·`socks5h://`)와 `https://` 프록시 너머의 https는 지원하지 않는다(``proxy `host:port`: socks5 proxies are not supported``,
      ``proxy `host:port`: https through an https proxy is not supported``). Go는 둘 다 지원한다.
    - 서버가 끝내는 중에 읽은 요청: Go는 답 없이 닫고 Rust는 503으로 답한다.
    - Go가 덧붙이는 그 밖의 헤더, 흔하지 않은 에러 문구(아래 "4d단계(http)에서 알고 남긴 차이").
  - 도구 (5단계, 2026-10-17 사용자 결정; 08 `fung test`·REPL 절, 아래 "5단계" 항목):
    - `fung test`: 교착이 보고되거나 띄운 태스크가 `exit(n)`을 부르면 그 파일의 나머지 블록을 건너뛴다(직접 `exit(n)`과 같음). Go는 남은 블록을 돌린다.
    - REPL: 한 입력 안의 교착은 그 입력만 실패시키고 세션은 계속된다(종료 코드 0으로 끝날 수 있음). 띄운 태스크의 `exit(n)`은 세션을 n으로 끝낸다.
      다음 입력을 기다리는 태스크는 사람이 천천히 쳐도 교착이 아니다. 에이전트는 세션 끝에 끝난다.
- [ ] **6c 할 일: 이식 잔가지 걷어내기** (Go 동등을 위한 과도기 잔가지; 2026-10-15 "이식 중에는 Go와 맞추고, Go를 지운 뒤 따로 걷어낸다", 6b 스펙에서 6c로 정함).
  Go 구현은 빠르게 해 본 첫 시도로 본다. 아래는 언어 설계가 아니라 Go 구현의 우연을 바이트 단위로 따라 하려고 붙인 것들이다. 5단계(도구)의 잔가지는 아래 "5단계" 항목에 있다.
  걷어낼 때 각 항목의 바른 동작을 설계 문서와 `tests/lang`에 정하고 코드를 바꾼 뒤, 바뀐 기대 출력을 `FUNG_EXPECTED=bless cargo test -p <크레이트> --test <이름>`으로
  다시 쓰고 diff를 검토한다(6b 스펙: 사용자가 없으므로 호환성은 고려하지 않는다). **완료 기준: 코드와 현재 문서에서 "Go"가 검색되지 않는다.**
  6b에서 남긴 이름·주석은 맨 아래 "6c: 이름과 주석" 항목.
  - ~~유니코드 판정 표와 식별자 규칙~~ → **해결 (6c-1, 2026-10-17)**: 식별자는 UAX #31(01 "식별자"), 태그는 Lu, "보이는 글자"는 L·M·N·P·S와 U+0020.
    `spec/unicode.txt`는 `crates/fung-front/src/unicode/generate.rs`가 유니코드 문자 데이터베이스(17.0.0)에서 만든다(`FUNG_UCD`, 01에 명령).
  - ~~런타임 한도 문구(`makeslice`·`Repeat`)·`channel(n)` 한도~~ → **해결 (6c-1)**: 원소 1억 개·글자 1억 개의 `E0806`, 인자 검사가 먼저, `channel(n)` 상한 없음(02 "크기 한도").
  - ~~표준 입력 읽기 단위 4 KiB~~ → **해결 (6c-1)**: 64 KiB(06 `read_line`).
  - ~~채널 순회가 닫힐 때까지 받은 뒤 함수를 부름~~ → **해결 (6c-1)**: 값이 올 때마다 부르고, 일찍 멈추면 나머지는 채널에 남는다(05).
  - Rust의 내장 함수 차이 목록(`crates/fung-front/src/builtins.rs`의 `RUST_PRELUDE_NATIVES_REMOVED`·`RUST_PRELUDE_AGENT`·`RUST_TYPE_RENAMES`·`RUST_MODULE_MEMBERS`, 3b):
    `spec/builtins.txt`는 Go가 만들어 atom을 담으므로, Rust는 차이(네이티브 `atom`·`swap`·`deref` 빠짐, `extras/rust/agent.fung`의 공개 이름 더함, 타입 이름 `atom` → `agent`,
    `_native.conc`의 Rust 전용 멤버)를 이 한 곳에 두고 앞단·부팅·명세 테스트가 함께 읽는다. 6a에서 `RUST_PRELUDE_FUNG_MOVED`(prelude에서 `conc`로 옮긴 `fan_in`·`pmap`·`retry`)와
    `RUST_FUNG_MODULE_MEMBERS`(`conc` 모듈의 서명)를 더했다. 6단계에서 명세에 합치고 목록을 지운다. `extras/rust/`를 따로 둔 것(Go의 `embed`가 하위 디렉터리를 넣지 않음)도 그때 정리한다.
    → **해결 (6b)**: `spec/builtins.txt`를 Rust 표에서 생성하면서(`crates/fung-vm/src/boot/spec_file.rs`, `FUNG_UPDATE_SPEC=1 cargo test spec_file`) 차이 목록을 모두 지웠고(Task 3·6),
    `extras/rust/*.fung`는 `extras/`로 옮겼다. 옮긴 이름의 안내는 `resolve::MOVED_TO_MODULE`. `spec/errors.txt`도 Rust 표(`diag/catalog.rs`)에서 생성한다.
  - ~~16진 실수 파싱, 대소문자 단순 매핑~~ → **해결 (6c-1)**: `float`은 리터럴 문법(06), `upper`·`lower`는 전체 매핑. 정렬도 안정 병합 정렬로 바꿨다.
  - ~~실행 파일 형식 사전 검사의 문구~~ → **해결 (6c-2)**: 거부는 유지, 문구는 `not an executable file (run scripts with sh)`(06 명령 실행).
  - ~~운영체제 에러를 Go 문구로, `os` 함수의 단계 옮김~~ → **해결 (6c-2)**: 흔한 에러는 fung 문구(종류별, 플랫폼마다 같음), 드문 것은 운영체제 문구(06).
    `mkdir`·`remove`·`remove_all`·`move`는 fung 규칙으로 단순하게(06 `fs` 규칙), 명령 실행 문구도 fung 문구로.
  - ~~경로 규칙과 디렉터리 순서~~ → **해결 (6c-2)**: `..` 정리·`join`(이어 붙임)·이름 바이트 순서는 규칙으로 남기고, 점으로 시작하는 이름은 확장자 없음, `walk` 에러는 실패한 디렉터리.
  - ~~`glob`의 우연~~ → **해결 (6c-2)**: 패턴 전체를 먼저 검사(잘못된 패턴은 항상 `'invalid`), 깊이 한도 없음. `\`·`!`·건너뛰기·정렬은 규칙으로 남김.
  - ~~파일 핸들 쓰기 버퍼와 표시 형식~~ → **해결 (6c-2)**: 표준 버퍼 64 KiB, 버린 핸들도 비운다, 표시와 메시지의 따옴표는 fung 문자열 리터럴.
  - ~~시간대 탐색, 시각 값의 표현, 시간 서식·파싱 세부~~ → **해결 (6c-3)**: C 라이브러리의 `TZ` 규칙(+`TZDIR`, POSIX 문자열), 유닉스 초 + 나노초로 0000~9999년,
    서머타임 경계의 명시적 규칙(없는 시각은 빈틈만큼 뒤로, 두 번 있는 시각은 이른 순간), 오프셋의 초, 숫자 오프셋은 고정, 엄격한 RFC 3339와 fung 문구(06 시간 형식).
  - ~~JSON의 `jsontext` 경로·읽기 버퍼 흉내, 이스케이프~~ → **해결 (6c-3)**: 입력 전체를 보는 반복 파서, 문제 글자의 위치와 fung 문구, 같은 키·짝 없는 서로게이트는 에러,
    필요한 것만 이스케이프, 결과에 글자 한도(06 JSON).
  - ~~정규식의 유니코드 표·에러 문구·반복 규칙·고정 기대값~~ → **해결 (6c-4)**: 문법은 `regex` 크레이트의 것(위 "정규식 문법"), 유니코드 클래스,
    fung 에러 문구, 크레이트의 반복·`$` 규칙, `split`은 끝의 빈 매치로 자르지 않음(06 정규식). 옮긴 파서·고정 데이터·`spec/regex_unicode.txt`는 지웠다.
  - ~~SIGPIPE의 런타임 세부~~ → **해결 (6c-5, 2026-10-18)**: 동작은 그대로(무시로 띄워도 신호 13, 대비책 141)이고 fung 규칙으로 적었다(06 "에러 규칙"의 닫힌 표준 출력·에러).
  - ~~네트워크 문구와 주소 규칙~~ → **해결 (6c-5)**: 네이티브는 짧은 이유(`connection refused`, `no such host`, `timed out`, `connection is closed`, `listener is closed`,
    `connection closed after N of M bytes`, ``invalid port `x` ``, ``invalid address `x` (…)``)만 주고 `http`가 무엇을 하다 났는지 붙인다. `accept4`/`accept` 구분 없음.
    빈 listen 호스트 `[::]`, 빈 dial 호스트 로컬, 두 번째 `close`가 에러인 것은 규칙으로 남겼다.
  - ~~TLS 문구와 순서~~ → **해결 (6c-5)**: rustls의 WebPKI 검증(사슬 다음 이름)을 그대로 쓰고(`GoOrder` 삭제), 에러 종류를 fung 문구로(`unknown certificate authority`,
    ``certificate is not valid for `host` ``, `certificate has expired (valid until <ISO 시각>)`, `TLS alert: <IANA 이름>`, `the server did not answer with TLS`,
    `TLS handshake timed out`, `connection closed inside a TLS record`, 그 밖은 `TLS error: <rustls 문구>`). 레코드 사이의 TCP 끝을 끝으로 보는 것은 규칙으로 남겼다(06 HTTP).
  - ~~HTTP 클라이언트·리다이렉트·프록시 흉내~~ → **해결 (6c-5)**: RFC 9110·9112·3986의 규칙은 남기고, `User-Agent: fung/<버전>`(`CONNECT`에도), 사용자 헤더는 맵 순서,
    `timeout` 하나가 연결·핸드셰이크·`CONNECT`·응답을 덮음(30초·10초·60초 상한 삭제), 리다이렉트 본문을 읽지 않음, 리다이렉트 10번까지(11번째가 에러),
    문구는 이유만(`Get "url": `·transport·`proxyconnect tcp: ` 접두사 없음, ``proxy `host:port`: …``, ``redirected to `url`: …``, `invalid URL: …`)(06 HTTP).
  - ~~HTTP 서버 흉내~~ → **해결 (6c-5)**: 표준 이유 문구(RFC 9110)·IMF-fixdate `Date`·204/304/1xx·`nosniff`·413/431 뒤 읽고 버리기·`accept` 재시도는 규칙으로 남기고,
    모르는 코드는 빈 이유 문구, 서버가 스스로 내는 본문은 `<코드> <이유 문구>`, 요청 머리 1 MiB(06 HTTP).
  - 바로 위 "2단계에서 알고 남긴 차이"(U+FFFD·환경 순서; 닫힌 파이프 141은 4c에서 해결)와 "3a단계에서 알고 남긴 차이"는 반대 방향(Rust가 Go를 못 따라가거나 일부러 따르지 않은 것)이라 걷어낼 대상은 아니고, 그때 어느 쪽을 언어 규칙으로 삼을지 정한다.
  - **6c: 이름과 주석** (6b에서 일부러 남긴 것, 동작과 무관). 6b는 Go 삭제와 대조 장치의 이름표 떼기까지만 했다.
    - 코드 주석과 이름: `crates/` 아래 `.rs` 파일 약 126개에 "Go …"를 가리키는 주석(예: "Go `register`", "Go's …", "(Go `internal/vm/...`)"), 파일·타입·함수 이름
      (`native/regex_go.rs`·`regex_go_fixtures.rs`·`regex_go_tests.rs`, `GoWriter`, `GoOrder`, `decode_go`·`compile_go`·`go_all`·`go_error`·`go_letter_or_digit`·`go_rune_width`,
      `*_like_go`·`*_matches_go`·`*_go_cases` 같은 테스트 이름). 대부분은 위 흉내 항목을 걷어낼 때 함께 바뀐다.
    - 그 밖의 파일: `deny.toml`(주석 2줄), `scripts/build-release.sh`(주석 2줄), `extras/*.fung`의 주석(`http.fung` 약 150줄, `prelude`·`json`·`regex`), `bench/README.md`(v0.1.0·Go HEAD 측정 열, 기록이라 남길지 정함),
      `scripts/logo/render.html`(1줄).
    - ~~테스트 전용 코드가 라이브러리에 들어 있음(`tls_test_server.rs`)~~ → **해결 (6c-5)**: `crates/fung-cli/tests/common/tls_server.rs`로 옮기고 rustls를 fung-cli의
      dev-dependency로(fung-vm과 같은 요구·기능, `deps.rs`가 이 하나만 허용).
    - ~~기대 출력의 `User-Agent: Go-http-client/1.1`~~ → **해결 (6c-5)**: `fung/0.0.0-dev`로 bless.
    - 현재 문서에 남긴 Go 흉내 서술: 06 "정규식"(`$1` 치환의 Go 규칙)(6c-5에서 "HTTP" 절과 매뉴얼 `User-Agent` 줄은 정리함),
      매뉴얼 `docs/internals.html`의 0.1 측정 출처(영속 컬렉션 절). 설계 문서 06의 파일 이름 `06-go-interop-and-stdlib.md`(제목은 6b에서 "네이티브 연동"으로 바꿈; 링크가 많아 이름은 6c).
    - 언어 설계로 남는 비교(걷어낼 대상 아님): 문법의 기준으로서의 Go("겉은 Go, 속은 Lisp", 00·background·매뉴얼), 05의 용어 비교(`select`·`context`), 01의 리터럴 비교, 이름의 유래(**g**o).
      완료 기준의 "검색되지 않는다"에서 이것들을 뺄지, 표현을 바꿀지는 6c에서 사용자와 정한다.
- [ ] 4a단계(fs)에서 알고 남긴 차이 (진행 기록 `.superpowers/sdd/2026-10-15-rust-fs-4a/progress.md`).
  - 닫지 않은 핸들: Rust는 마지막 사본이 사라지는 순간 파일을 닫는다(참조 계수). Go는 GC가 거둘 때까지 열려 있을 수 있다. 그래서 핸들 수천 개를 버려도 디스크립터가 새지 않는다.
    버리는 핸들의 버퍼는 마지막 사본이 사라질 때 비운다(6c-2; 그때의 쓰기 에러는 보고하지 않는다).
  - `close(2)`의 에러: `fs.close`·`write`·`append`·`copy` 모두 Rust는 파일을 닫을 때의 에러를 보고하지 않는다(Go는 보고). NFS 같은 파일 시스템에서만 드러난다.
  - 마지막 사본이 사라질 때의 `close(2)`는 실행권을 쥔 채 돈다(`Drop`은 실행권 안에서 일어남). 로컬 파일은 순식간이지만 NFS 같은 곳에서 멈추면 그동안 다른 태스크도 멈춘다.
    고친다면 닫을 파일을 배경 닫기 스레드에 넘긴다. Go는 GC의 finalizer가 따로 닫아서 해당 없음.
  - `remove_all`: 디렉터리 안쪽은 `std::fs::remove_dir_all`에 맡겨서, 깊은 곳에 실패가 여럿 섞이면 보고하는 첫 errno가 Go `removeAllFrom`과 다를 수 있다. 첫 errno 하나만 보고하는 것은 같다.
  - Go 문자열은 잘못된 UTF-8 바이트를 그대로 품어 핸들 표시에서 `\xff`로 보이지만, Rust 경로 문자열은 U+FFFD로 바뀌어 있어 그대로 보인다(2단계 U+FFFD 차이와 같은 뿌리).
  - `fs.read`·`read_lines`·`read_line(h)`는 파일 내용의 잘못된 UTF-8을 잘못된 부분마다 U+FFFD 하나로 바꾼다(6c-3). Go는 원래 바이트를 그대로 보존한다(같은 뿌리).
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
  - ~~깊은 중첩의 스택 넘침~~ → 6c-4부터 크레이트의 중첩 한도(250단계)가 그 전에 E0515로 거절한다.
  - ~~엔진이 크기 밖의 이유로 거부하는 패턴의 Go 문구~~ → 6c-4: 파서가 먼저 거르므로 닿지 않고, 닿으면 `invalid pattern`.
  - ~~내부 에러 보고(`crates/fung-vm/src/vm/task.rs`의 `eprintln!`)는 표준 에러가 닫혀 있으면 SIGPIPE로 죽지 않고 패닉한다.~~ 해결 (4c 최종 수정): 결과를 버리는 `writeln!`으로 바꿔 표준 에러가 닫혀 있어도 `abort`까지 간다.
  - 4c 보류 (최종 리뷰에서 머지 전 수정 불필요로 분류한 사소한 항목):
    - ~~옮긴 Go 파서의 HIR 왕복 확인, 무작위 기대값의 PCG 의존~~ → 6c-4에서 파서와 고정 데이터를 지워 해당 없음.
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
    - ~~첫 레코드 검사가 Go보다 좁다.~~ → 6c-5: 검사는 fung 규칙(경고·핸드셰이크가 아니거나 있을 수 없는 판), 문구 `the server did not answer with TLS`.
    - 치명적 경고를 받은 뒤 쓰기 에러가 Go처럼 "끈적"(sticky)하게 남지 않는다.
    - ~~이름 불일치 문구의 이름 목록을 webpki의 `Debug` 출력에서 해석한다.~~ → 6c-5: 문구는 ``certificate is not valid for `host` ``(인증서의 이름 목록은 쓰지 않음).
    - ~~Go의 "legacy Common Name"·`SystemRootsError` 문구, 그 밖의 rustls 에러의 `tls: …`~~ → 6c-5: rustls가 판단한 종류의 fung 문구, 그 밖은 `TLS error: <rustls 문구>`
      (루트가 하나도 없으면 `unknown certificate authority`).
    - DNS 이름으로 맞지 않는 호스트(`_` 등)는 연결 전에 ``invalid TLS server name `host` ``(6c-5 문구).
    - close_notify를 기다리지 않고 보낸다(소켓 버퍼가 차 있으면 버려짐). Go는 5초까지 기다린다. ALPN을 보내지 않는다(Go는 `h2, http/1.1`).
  - 클라이언트(Task 3):
    - 인자 에러의 호출 경로에 `extras/rust/http.fung` 프레임이 보인다. Go의 함수는 네이티브라 없다(06은 extras 프레임을 보이게 둔다).
    - 사용자 함수가 `http.*`를 꼬리 위치에서 부르면 그 프레임이 사라져, 에러가 부른 쪽의 부른 쪽 위치를 가리키고 경로에 `get (1 tail calls elided)`가 보인다. fung으로 쓴 모든 라이브러리 함수와 같다(3b `swap`과 같은 뿌리).
    - ~~TCP 연결과 핸드셰이크를 `min(10초, 남은 시간)` 기한 하나로 묶는다.~~ → 6c-5: 요청의 `timeout` 하나가 덮는다.
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
    - ~~`CONNECT` 60초 기한~~ → 6c-5: 요청의 `timeout`이 덮고, 넘기면 `request to … timed out`.
    - 프록시는 요청마다 환경에서 고른다(Go는 프로세스마다 한 번). fung 프로그램은 자기 환경을 바꿀 수 없어 보이지 않는다.
  - 서버(Task 5):
    - 이름이 대소문자만 다른 응답 헤더(`{"x-a": .., "X-A": ..}`): Go는 맵 순서라 아무것이나, Rust는 뒤의 것.
    - 핸들러가 넣은 틀 헤더는 쓴 그대로: `Content-Length`는 자동 값을 끄지만 본문은 달라도 다 쓴다(Go는 거부하거나 자르고 기록), `Transfer-Encoding`은 해석하지 않음, 101 전환의 `Connection: upgrade` 세부는 없음.
    - 413: Go는 16 MiB + 1을 먼저 읽고, Rust는 선언된 길이가 넘으면 바로 답한 뒤 500 ms 읽고 버린다(선 위의 바이트는 같고 시점이 다름). 431에서 Go는 FIN을 보내고 기다리지만 Rust는 반쯤 닫기 원시 기능이 없어 읽기만 한다.
    - ~~431 경계~~ → 6c-5: 요청 머리 1 MiB를 줄 바이트 + CRLF로 센다(빈 줄 포함, 06 HTTP).
    - 핸들러가 띄운 태스크는 핸들러 아래에 걸려 핸들러와 함께, 서버와 함께 취소되지만 배경 종류다. 프로그램을 붙잡지 않고, 교착 집계에 따로 들며, 꺼내지 않은 실패는 보고된다.
    - `serve` 인자 에러의 호출 경로에 extras 프레임이 보이고, `ready` 에러에는 `_ready_check`·`serve` 두 프레임이 더 있다(5단계 Task 0에서 포매터 결함을 고쳐 `_serve` 감싸개 프레임은 없어짐).
    - 따라 하지 않은 것: `PRI * HTTP/2.0`, `CONNECT`의 2xx 답, 요청 컨텍스트를 취소하는 배경 읽기.
    - 본문 중간에 멈춘 클라이언트는 닫을 때까지 연결 태스크의 스레드 하나를 붙잡는다(Go도 `ReadTimeout`이 없어 붙잡지만 고루틴이라 쌈). R7에서 푼다.
    - 헤더 폭탄의 비용(서버): 요청 머리(1 MiB, 값 500개)를 fung으로 해석하므로 꽉 찬 머리 하나에 요청마다 약 0.1~0.25초(Go는 거의 0).
    - 연결 태스크를 띄우지 못하면(스레드 천장, `E0801 cannot start a task`) 그 연결을 답 없이 닫고 표준 에러에 `http: cannot start a connection task: …`를 쓴 뒤
      받기 재시도처럼 물러났다가 계속 받는다(4d 최종 수정; 그 전에는 서버 전체가 멈췄다). Go에는 이런 경우가 없다(고루틴은 늘 뜬다).
  - 테스트: `http_parity`(6b부터 `http.rs`)는 프로필마다 약 45초 걸렸다. 헤더 10초 한도 시험(`a_slow_client_is_cut_off_after_ten_seconds`)은 시간을 주입하지 않고 실제로 10초를 기다린다.
  - 4d 보류 (시험 부채; 최종 리뷰에서 머지 전 수정 불필요로 분류):
    - `read_all` 쪽 취소 시험은 실패(RED)를 먼저 확인하지 않았다(`read_line` 쪽만 확인).
    - 교착 없음 시험의 `free_port`: 닫은 포트를 다른 프로세스가 그 사이에 잡을 수 있다(드묾).
    - ~~`crates/fung-cli/tests/tls.rs`: 실패하는 길에서 Go TLS 서버 프로세스와 임시 디렉터리를 거두는 정리 장치(`Drop` guard)가 없다(성공하는 길에서만 치움).~~ 해결 (6b): TLS 서버는 테스트 프로세스 안의 스레드(`fung_vm::tls_test_server`)이고, 자식 프로세스는 모두 `common::Reaped`가 거둔다.
    - 시험 없음: `SSL_CERT_FILE`의 깨진 PEM, `SSL_CERT_DIR`만 있는 경우, TLS 읽기 도중의 취소.
- [ ] 5단계(도구)에서 알고 남긴 차이와 잔가지 (진행 기록 `.superpowers/sdd/2026-10-17-rust-tools-5/progress.md`, 노트 `docs/notes/R5-tools.md`).
  - 일부러 Go와 갈라진 것 (사용자 결정, 2026-10-17; 08에 규칙):
    - `fung test`에서 교착이 보고되거나 띄운 태스크가 `exit(n)`을 부르면 그 파일의 나머지를 건너뛴다(직접 `exit(n)`과 같음, 선택지 B).
      돌던 블록은 자기 줄을 내고, 남은 블록이 있을 때만 메모 한 줄. 건너뛰기는 실패 하나로 센다: 통과한 블록 뒤면 `FAIL P: <rest of file>` + 메모(최종 리뷰).
      최상위가 정상으로 끝났는데 exit나 교착이 걸려 있으면 `FAIL P: <top level>` + 메모. 두 갈래 모두 상태로 순서를 정한 고정 파일(`test_rust/rest_skipped`·`top_pending`)이 지킨다.
      Go는 취소된 루트에서 남은 블록을 돌려, 호출 없는 블록은 통과하고 호출하는 블록은 E0701로 실패한다. 이것은 경쟁이 아니라 호출 여부로 결정적이다(취소 플래그는 호출할 때만 검사).
    - REPL의 교착: 그 입력만 실패(Go와 같은 보고), `Runtime::recover_deadlock`이 앞 입력들이 막힌 채 남긴 태스크까지 끝나기를 기다린 뒤 새 실행 루트를 단다(보고가 건 exit(1)만 지우고, 그 뒤 태스크가 요청한 exit(n)이 있으면 루트를 바꾸지 않고 세션을 n으로 끝낸다). 전역·에이전트는 남고,
      회복한 세션은 종료 코드 0으로 끝날 수 있다. Go는 세션이 취소된 채 남고 끝날 때 1.
    - REPL에서 띄운 태스크의 `exit(n)`: 세션을 n으로 끝낸다(잘린 입력의 E0701과 값은 찍지 않음). 터미널에서는 줄 읽기가 기다리는 중이라 다음 줄을 친 뒤에 끝나고 그 줄은 돌지 않는다.
      Go는 계속 읽고, 새 태스크는 취소를 늦게 본다.
    - REPL의 `end_background`는 세션 끝에 한 번(위 3b 항목). REPL은 세션 내내 `hold`하므로 다음 입력을 기다리는 태스크는 사람이 천천히 쳐도 교착이 아니다.
      Go는 50 ms 유예 안에 다음 줄이 올 때(파이프)만 그렇고, 천천히 치면 입력 사이에서 교착을 보고하고 세션을 취소한다.
  - 흉내가 아닌 장치: `Runtime::hold`·`release`(`Task::driven`, `hold_taken` 단언). 명령이 블록·입력을 더 돌릴 참인 틈을 교착으로 보지 않게 명령 자신을 살아 있는 태스크 하나로 센다.
    Go에서는 50 ms 유예가 우연히 같은 일을 하지만, 리뷰는 이것을 Go 흉내가 아니라 **진짜 의미상 필요**로 판단했다. Go를 지워도 남긴다.
  - ~~Go 흉내(과도기 잔가지)~~ → **해결 (6c-6, 2026-10-18)**, 규칙은 08:
    - `fung test`: 중복 제거 키(소스 이름·줄·열·코드)와 `exit(n)` 규칙은 그대로, 읽지 못한 파일은 `FAIL <load>`(종료 코드 2), 종료 핸들러 실패는 `FAIL <exit handler>`.
    - 경로 순회: `test`·`fmt`가 한 규칙(`_`·`.` 디렉터리 건너뜀, 인자로 준 링크는 따라감, 순회 중의 링크는 따라가지 않음). 파일 에러는 ``cannot read `p`: <이유>``·``cannot write `p`: <이유>``.
    - `check -e CODE 나머지`는 사용 오류. 디렉터리는 ``cannot read `dir`: is a directory``. 표준 입력은 `<stdin>`. `version`의 `(devel)` 처리는 6c-5에서 없앰.
    - 포매터 정렬은 화면 칸 수(UAX #11, `spec/unicode.txt`의 `wide`·`zero`), 잘못된 UTF-8은 부분마다 U+FFFD 하나·1칸. 원래 바이트 보존은 그대로.
    - REPL: 발췌는 코드를 정의한 입력의 글(런타임에도 글을 둠), 전역 한도를 넘긴 입력은 아무것도 묶지 않고 그 입력만 실패,
      줄 편집은 터미널 설정이 있을 때만, 두 읽기 길 모두 LF 하나와 그 앞의 CR 하나를 뗌, 기록 파일은 원래 바이트로 줄임. 종료 코드 규칙은 그대로.
    - W0102: 같은 위치는 이름 순서(08에 규칙).
  - ~~6c-6에서 남긴 줄 편집기의 키 디코딩(바이트마다 U+FFFD)과 커서 너비(글자마다 1칸)~~ → **해결 (6c-7)**: 부분마다 U+FFFD 하나, 커서는 표시 너비(08 REPL).
  - 6c-6 최종 리뷰에서 기록만 한 것: 표시 너비는 글자 단위 UAX #11이라 ZWJ로 이은 이모지(각 글자 2칸)와 VS16(U+FE0F, 0칸) 붙은 글자는
    터미널과 다를 수 있다(코드에 드묾). REPL 런타임이 입력마다 글을 한 벌 더 쥔다(`Runtime::add_session_files`; 세션 `SourceMap`과 `Arc<str>`로 나누면 없어짐).
  - 알려진 차이와 한계:
    - W0103은 구현마다 자기 prelude 목록을 쓴다(Rust의 `atom`·`agent` 차이, 3b).
    - REPL 세션의 `SourceMap`은 입력마다 파일 하나씩 계속 커진다(긴 세션에서 메모리가 조금씩 는다).
    - REPL 꺼내지 않은 실패: 세션 중에 핸들이 버려진 실패를 Rust는 마지막 핸들이 사라질 때, Go는 GC나 끝에서 보고한다(위 3a 항목과 같은 뿌리).
    - 두 구현이 같이 가진 한계: 정렬(`align_runs`)이 묶음의 첫 줄마다 다시 시작해 긴 묶음에서 O(n²). 제자리 `fmt`가 원자적이지 않다(임시 파일 + 이름 바꾸기가 아님, 쓰다 죽으면 파일이 잘림).
      프롬프트에서 밖으로부터 SIGTERM·SIGHUP을 받거나 출력이 EPIPE로 죽으면 터미널이 raw로 남는다(신호 처리기 없음; 고친다면 `tcsetattr`로 되돌리는 처리기 약 40줄).
    - 가짜 터미널 도우미(`fung_vm::term::pty`, `#[doc(hidden)]`)가 라이브러리에 들어 있다. 테스트 전용 feature로 감추려 했으나 `crates/fung-cli/tests/deps.rs`가
      `fung-cli`의 의존성에 `path` 말고는 허락하지 않아(`{ path, features }`도 위반) 울타리를 바꾸지 않았다. Linux에서는 `openpty` 때문에 libutil을 링크한다. 명령은 `pty`를 부르지 않는다.
      6단계 릴리스 대상(musl·정적 링크)에서 libutil 링크가 되는지 확인한다(최종 리뷰). → 6a Task 3에서 해결: musl에는 `libutil`이 없고 `openpty`가 `libc.a`에 있으므로
      `#[link(name = "util")]`를 `target_env = "gnu"`에서만 건다(노트 `docs/notes/R6a-release-prep.md` 3절). musl 링크 자체는 CI `musl` 작업과 릴리스 빌드가 확인한다.
    - 포매터(두 구현 같음, 5단계 이전부터): 함수 머리가 여러 줄이면(`fn f(h:` 다음 줄 `int):`) 몸통을 4칸, `;;`를 2칸에 둔다(최종 리뷰에서 발견).
      이식이 끝난 뒤에 본다(Go와 맞추는 동안은 그대로).
    - `#[allow(dead_code)]`: `fung-cli`의 `mod paths`에 붙였던 것은 `fmt`가 `fung_files`를 쓰게 되어 지웠다. `tools_expected.rs`(옛 `tools_parity.rs`)가 `#[path]`로 `paths.rs`를 끌어오는 곳에만 남는다(테스트가 일부 함수만 씀).
- [x] Go 포매터 결함 (M9, 4d Task 5에서 발견): 매개변수에 `fn` 타입 표기가 있는 함수의 블록 몸통을 Go `fung fmt`가 내어쓴다(`fn` 타입을 람다로 읽음). 최소 재현:
  `fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;`의 몸통이 들여쓰기를 잃는다. 4d에서는 `extras/rust/http.fung`를 `serve = _serve(...)` 한 줄 감싸개로 피했다.
  → 5단계 Task 0에서 해결 (2026-10-17, 사용자 승인으로 Go 수정): 들여쓰기 상태 기계(`internal/format/layout.go`)가 타입 이름 `fn`도 `:`를 기다리는 블록 키워드로
  쌓아, 괄호가 닫힌 뒤의 블록 `:`가 그 짝을 찾지 못했다. 이제 `fn` 뒤가 이름이나 `(`일 때만 함수 머리로 본다(`fnHead`). 감싸개를 걷어 `serve`가 본래 함수다.
- [x] Go 포매터 결함 두 가지 (5단계 Task 3 리뷰에서 발견) → 5단계 Task 3b에서 Go·Rust 함께 해결 (2026-10-17, 사용자 승인으로 Go 수정):
  - BOM: 렉서는 맨 앞 BOM 하나를 건너뛰는데 `format.Format`은 그 오프셋으로 BOM이 남은 소스를 잘라, BOM + 코드 파일을 E0801로 거절하고
    주석만 있는 파일에서는 BOM을 말없이 지웠다. 이제 BOM을 떼고 포맷하며 출력에 BOM은 없다. 두 겹 BOM은 렉서처럼 E0101(두 번째는 코드).
  - `fmt --check -`가 `--check`를 무시하고 포맷 결과를 찍었다. 이제 바뀔 게 있으면 파일처럼 `<stdin>`을 찍고 종료 코드 1, 없으면 아무것도 안 찍고 0.
- [ ] 6a단계(릴리스 전환 준비)에서 정한 것과 6b 할 일 (진행 기록 `.superpowers/sdd/2026-10-17-rust-release-prep-6a/progress.md`, 노트 `docs/notes/R6a-release-prep.md`).
  - [x] 내장 함수 배치 → **"prelude에는 확실한 것만"** (2026-10-17 사용자 결정). Rust에서 `fan_in`·`pmap`·`retry`를 prelude → `conc` 모듈
    (`extras/rust/conc.fung`, `import conc`). 셋 다 정책(동시 실행 수, 재시도 횟수·간격)을 품은 조합이라 확실하지 않다. `until`·`iterate`는 `while`이 없는 언어에서
    반복의 대체라 핵심, `exec`·`sh`는 셸을 대신하는 언어의 핵심이라 prelude에 남긴다. `remove`·`close`와 `fs.remove`·`fs.close`의 이름 겹침은 유지하고 매뉴얼에 한 줄.
  - [x] Linux 바이너리 → **musl 정적 빌드** (2026-10-17 사용자 결정). Go의 "아무 서버에서나 돈다"를 지킨다. glibc 정적 링크를 쓰지 않는 이유: `getaddrinfo`가 NSS로 공유 라이브러리를 실행 중에 읽어 같은 버전의 glibc가 있어야 한다.
    `libutil` 링크는 gnu에서만(위 5단계 항목). musl 할당기 속도는 아직 재지 못했다(아래).
  - ~~Rust 부팅이 숨기는 prelude 사본~~ 해결 (6b Task 6: `prelude.fung`에서 지우고 숨기는 장치와 목록을 없앰). 지난 기록: `extras/prelude.fung`는 Go와 함께 쓰므로 `fan_in`·`pmap`·`retry`의 Go용 사본이 남아 있고, Rust는 앞단(`RUST_PRELUDE_FUNG_MOVED`)과
    부팅(`boot_file(..., hidden)`)에서 숨긴 채 **여전히 컴파일하고 부팅 때 돌린다**. 6b에서 Go를 지우면 `prelude.fung`에서 지우고 숨기는 장치와 목록을 없앤다.
  - 옮긴 이름의 안내 (Rust 전용 차이): `import conc` 없이 `pmap` 등을 쓰면 E0204는 그대로이고 도움말이 "비슷한 이름"보다 먼저
    `help: moved to module conc: import conc, then conc.pmap`을 보여 준다(`resolve.rs`, 6b부터 목록은 `resolve::MOVED_TO_MODULE`).
    0.3 이후 업그레이드 안내가 필요 없어지면 지울지 정한다.
  - [x] 6b 할 일 (스펙 결정) → **완료** (2026-10-17, 아래 "6b단계" 항목). 지난 기록:
    - Go 구현 삭제(`cmd/`·`internal/`·`extras/`의 Go embed·`go.mod`·Go 테스트·CI의 Go 작업·`release.yml`의 Go 테스트), `# only: go` 파일 정리와 `# only:` 표시 정리, `extras/rust/`를 `extras/`로.
    - Go 대조 테스트(`parity`·`run_parity`·`tools_parity`·`http_parity`·`repl_tty`·`sigpipe`·`tls` 등)는 Go 출력을 한 번 기록해 **기대값 파일로 얼리고** 계속 돌린다. 시간에 기대는 것은 Rust 전용 테스트로.
    - `spec/builtins.txt`·`errors.txt`는 Rust 코드에서 생성하고 동기 테스트로 지킨다(Rust 내장 함수 차이 목록은 그때 명세에 합쳐 지운다). `spec/unicode.txt`·`regex_unicode.txt`·
      정규식 대조 데이터(`regex_go_fixtures.rs`)는 얼린 데이터로 남긴다.
    - Go 테스트를 지우기 전에 Go에만 있는 검사를 옮긴다: `tests/install_test.go`(가짜 릴리스에 대고 `install.sh` 실행)와 `tests/changelog_test.go`
      (CHANGELOG `[Unreleased]`/릴리스 절과 매뉴얼 사이드바 버전의 짝)를 Rust 통합 테스트나 CI 셸 스크립트로 옮기고, `ci.yml`이 계속 돌리게 한다.
    - 테스트 도우미의 Go 전용 장치 정리: `go_source`(Go에 넘길 때 `import conc`를 빈 줄로), `RUST_ONLY_EXAMPLES`·`uses_rust_only`, `FUNG_REQUIRE_PARITY`.
    - 위 "과도기 잔가지"를 걷어내는 일은 6단계 뒤(스펙 범위 밖).
  - [ ] **태그 전에 릴리스 워크플로 시험 실행**: `gh workflow run release.yml -f version=0.0.0-dryrun`(사용자가 실행). Linux musl 빌드, `readelf` 정적 검사, ELF `file` 문구 패턴,
    darwin/amd64 교차 빌드, 작업 사이 아티팩트 배치는 CI에서만 돈다(로컬은 macOS arm64뿐). `publish`(증명·게시)는 태그 푸시에서만 돌므로 첫 실제 태그에서야 처음 돈다.
    0.2.0 태그는 6b 뒤 스쿼시 머지 다음이고, 머지·태그 푸시는 그때 사용자 확인.
  - [ ] musl 빌드 벤치마크: `bench/README.md`의 musl 열이 비어 있다(이 기계에서 만들 수 없음). Linux에서 릴리스 스크립트로 만든 바이너리로 같은 방법으로 잰다.
    musl 할당기가 느리면(`map`·`str`) 할당기 교체를 크레이트 허용 목록 결정과 함께 검토한다.
  - [ ] 고치지 않고 남긴 사소한 것 (태스크 리뷰에서 머지 전 수정 불필요로 분류):
    - ~~4 GiB 실행 스레드를 만들지 못하면 아무 메시지 없이 종료 코드 101~~ 해결 (6a 마무리): 예약이 거부되면 더 작은 스택으로 다시 시도한다(진입 4 GiB → 1 GiB → 256 MiB → 64 MiB,
      태스크 1 GiB → 256 MiB → 64 MiB, `crates/fung-vm/src/stack.rs`). 모두 거부되면 진입은 한 줄 에러와 종료 코드 1, 태스크는 E0801. 그런 호스트에서는 아주 깊은 네이티브 재귀
      (fung을 부르는 `map`·`fold`의 깊은 중첩, 깊은 값 출력)의 한도가 낮다. CHANGELOG "알려진 차이".
    - `crates/fung-cli/tests/run_expected.rs`(옛 `run_parity.rs`)의 `CONCURRENCY` 목록이 맨 이름 `pmap`·`fan_in`·`retry`를 아직 담음(무해: 실행하는 파일에 이제 없고 `conc.pmap(`은 걸리지 않음).
      `testutil.rs`·`run_expected.rs` 주석의 맨 `pmap` 표기.
    - `tools_expected.rs`(옛 `tools_parity.rs`)의 `run_limited`는 기다리는 동안 파이프를 비우지 않는다(64 KB 넘게 찍는 매뉴얼 예제는 시간 한도까지 막힌 뒤 죽음; 지금은 없음).
    - `manual.rs`: 모듈 목록 `MODULES`를 손으로 적음, fung 서명을 파서가 아니라 최상위 `fn NAME(`·`NAME = _x.member` 줄을 훑어 읽음(구조 분해 같은 다른 최상위 값은 건너뜀),
      ~~`uses_rust_only`의 `agent(`가 단어 경계를 보지 않음, Rust 전용 예제 수 단언이 약함~~(6b에서 장치째 지움).
    - `builtins.html`의 U+FFFD 글자를 문자 그대로 씀(엔티티가 아님).
    - `check-release.sh`의 ELF 패턴은 GNU `file`의 문구를 가정한다. darwin/amd64는 러너에 Rosetta가 있을 때만 실제로 돌려 본다.
    - 정적 바이너리도 실행 중에 시스템 CA 파일을 읽는다(`rustls-native-certs`). CA 인증서가 없는 `scratch` 컨테이너에서는 https가 실패한다(0.1 Go도 같음).
    - Rust `conc` 모듈(키 `conc`)과 `_native.conc`의 `Module` 이름이 둘 다 "conc"다(키가 달라 충돌은 없음).
    - ~~`docs/design/background.md` 원칙 4("Go는 내부에서 쓰되 겉으로 드러나지 않게")를 매뉴얼처럼 "구현 언어는…"으로 맞추지 않았다~~ 해결 (6b): "구현 언어(Rust)는 내부에서 쓰되 겉으로 드러나지 않게".
- [ ] 6b단계(Go 삭제)에서 정한 것과 남은 것 (진행 기록 `.superpowers/sdd/2026-10-17-rust-go-removal-6b/progress.md`, 노트 `docs/notes/R6b-go-removal.md`).
  - [x] Go 대조 → **기대 출력 파일** (2026-10-17 스펙 결정). 입력마다 출력이 정해지는 대조(`dump`·실행·`check`·`test`·`fmt`·REPL·`json`·`time`·`regex`, `fung-vm`의 prelude·`fs` 프로그램)는
    Go가 있을 때 Go 출력과 Rust 출력이 사례마다 같은지 확인하며 **Go 출력을 기록**했고, 이후는 Rust만 그 파일과 비교한다. Go 서버·클라이언트를 맞붙이던 것(`http`·`tls`·`sigpipe`)은
    요청 원문·응답·신호 13·에러 문구를 같은 형식으로 고정해 Rust 혼자 검사한다(TLS 서버는 `fung_vm::tls_test_server`, 6c-5부터 `crates/fung-cli/tests/common/tls_server.rs`).
    - 파일: `crates/fung-cli/tests/expected/<주제>/*.txt`, `crates/fung-vm/tests/expected/{prelude,fs}/runs.txt`. 형식은 `crates/fung-cli/tests/common/expected.rs` 머리 주석
      (`==== 이름표`, `-- 칸` 뒤 `| 줄`, 제어 문자는 `(escaped)`, 큰 값은 길이와 FNV-1a 해시, 임시 경로·포트·저장소는 `$TMP`·`$PORT`·`$REPO` 따위로 치환).
    - 갱신: 일부러 동작을 바꾼 뒤 `FUNG_EXPECTED=bless cargo test -p <크레이트> --test <이름>`(`fung-vm`은 테스트 이름으로 거름; 각 파일 머리 주석에 명령이 있다), diff 검토 후 커밋.
      테스트마다 사례 수를 상수로 단언하므로 사례를 더하거나 빼면 상수도 고친다. 기록 모드(`FUNG_EXPECTED=record`)와 Go 실행 도구(`common::go_fung`, `FUNG_REQUIRE_PARITY`)는 Go와 함께 지웠다.
    - 호스트 의존(노트 3절): 기록은 macOS에서만 했고 Linux CI 실행으로 확인했다. 그 과정에서 고친 것: 모든 기대 출력 실행에 `TZ=UTC`(`common::TZ`; 처음에는 기록한 기계의 시간대가
      들어갔다), Linux에서만 만들 수 있는 비UTF-8 파일 이름은 얼린 사례에서 빼고 UTF-8 이름과 같은지 비교하는 유도 테스트로, 권한 없는 디렉터리 사례는 root면 건너뜀,
      디렉터리 크기는 `$SIZE`, macOS `/private/tmp`는 `/tmp`로.
  - [x] 명세 파일의 원본 → **Rust 코드** (Task 3·6). `spec/builtins.txt`는 `fung_vm::boot`의 표(`NATIVE_MODULES`, prelude 네이티브, `extras::PRELUDE_FILES`, `conc`·`http` 모듈)에서,
    `spec/errors.txt`는 `fung_front::diag::catalog`의 표에서 만들고, 테스트가 파일과 코드가 같은지 지킨다. 갱신은 `FUNG_UPDATE_SPEC=1 cargo test spec_file`.
    생성기는 파일을 읽지 않고 표에서 시작하므로 새 `_native.*` 모듈을 더해도 손으로 파일을 먼저 고칠 필요가 없다. `unicode.txt`·`regex_unicode.txt`는 고정 데이터(위 6c 할 일의 첫 항목).
  - [x] Go 테스트가 지키던 것: `install.sh`(가짜 릴리스) → `crates/fung-cli/tests/install_script.rs`, CHANGELOG ↔ 매뉴얼 사이드바 버전 → `changelog.rs`. errno 표를 살아 있는 Go와
    대조하던 테스트는 Rust 단독 확인(대표 문구, 모든 칸이 한 줄)으로 바꿨다(`native/oserr.rs`의 표 자체가 기록).
  - [x] Go가 다시 들어오지 않게: `crates/fung-cli/tests/repo_rust_only.rs`가 `.go`·`go.mod`가 없는지, 워크플로·스크립트·크레이트·`CLAUDE.md`·`README.md`·`docs/design`에
    Go 도구 호출이 없는지 본다(학습 노트·계획·스펙·CHANGELOG는 기록이라 뺌).
  - [x] 자식 프로세스 정리 (2026-10-17 사용자 요청): 테스트가 띄우는 서버·클라이언트는 모두 `common::Reaped`가 쥐어, 테스트가 패닉해도 `Drop`에서 죽이고 거둔다
    (`a_panicking_test_leaves_no_server_behind`가 확인). 손자 프로세스는 거두지 않는다(테스트 프로그램이 남기지 않음).
  - 사례 수(6b 끝): dump 644, 실행 196(`tests/lang` 139, 동시성 33은 릴리스에서 50번씩), 골든 `tests/lang` 139개 파일, `check` 저장소 167, `test` 저장소 168, `fmt` 입력 4,404, 파이프 REPL 34,
    http 클라이언트 96·서버 89, regex 쌍 800, json 입력 143, time 168, prelude 357. `# only:` 파일을 합치며 늘었고, 줄어든 것은 지운 Go 전용 파일(Go의 얇은 `extras/http.fung` 래퍼, `# only: go` 다섯 개)뿐이다.
- [ ] **R7 스케줄러** (6단계 Go 삭제 뒤의 별도 단계, 4d 스펙 결정 2026-10-16). 언어 의미를 바꾸지 않는 내부 교체다. 막히는 곳은 `vm/block.rs`의 `block()`과
  소켓의 `vm/watch.rs`(`Watch`)로 모여 있어 그 자리를 갈아끼운다. 그때 함께 판단할 것:
  - 그린 스레드: 태스크를 운영체제 스레드가 아니라 VM이 주차·재개하는 단위로. 이벤트 루프 크레이트 `mio`를 들일지(4d에서는 들이지 않음).
  - 측정 기준선 (2026-10-17, M2 Max·macOS arm64, `bench/` 9회 중앙값, 방법은 `bench/README.md`; musl 열은 이 기계에서 못 만들어 비어 있음):
    v0.1.0/Go HEAD/Rust 초 — fib 0.517/0.207/0.201, loop 0.564/0.241/0.217, map 0.494/0.455/0.361, str 0.066/0.055/0.075,
    chan 0.112/0.091/0.251(Go HEAD의 2.8배), spawn 0.010/0.010/0.126(12.6배). Go HEAD는 v0.1.0 뒤의 최적화가 든 미릴리스 Go(`a06e1f4`).
    계산 위주(fib·loop·map)는 Rust가 같거나 빠르고, 태스크 생성(spawn, 태스크 2,000개)과 채널 전환(chan)만 느리다: 운영체제 스레드 생성·전환 비용이 보인다.
  - 태스크 생성 비용: 지금 태스크 하나에 약 55 µs(Go 약 2 µs, 약 25배; `docs/notes/R3a-runtime.md` 9절). 채널 주고받기 약 2.5 µs(Go 약 3배).
  - 스레드 천장: 살아 있는 태스크·에이전트마다 운영체제 스레드 하나라서 프로세스 스레드 한도(macOS `kern.num_taskthreads` 8192)가 천장이다.
    `map(0..20000, i => agent(i))`는 `E0801 cannot start a task`, 에이전트 `swap` 한 번은 약 7 µs(Go atom 0.4 µs; 위 3b 목록).
    HTTP 서버는 천장에 닿아도 죽지 않지만(4d 최종 수정), 스레드가 바닥난 동안 새 연결은 운영체제 대기열에서 거절되거나 답 없이 바로 닫힌다.
  - 람다를 부르는 고차 내장 함수(`map`·`fold`·`sort_by`·`each`)의 구현 언어: Rust 안에서 람다를 부르면 그 사이에 태스크를 멈출 수 없고 Rust 스택을 쓴다(위 "구현 언어 지침").
  - 스택 크기: 지금은 진입 스레드 4 GiB, 태스크 스레드 1 GiB 예약에 기댄다(깊은 정규식 컴파일, 깊은 값 출력; 거부되면 64 MiB까지 줄여 다시 시도, `fung_vm::stack`). 그린 스레드는 작은 스택이 전제라 다시 정한다.
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
  Go 시절의 설치 경로 대신 `cargo install --locked --git https://github.com/tunarider/fung --tag v0.2.0 fung-cli`가 생긴다(`include_str!` 경로가 모두 저장소 안이라 git에서 빌드된다;
  2026-10-17 `cargo install --locked --path crates/fung-cli`와 로컬 `file://` 저장소의 `--git`으로 확인. 이 길은 `FUNG_BUILD_VERSION`이 없어 `fung version`이 `0.0.0-dev`). README·매뉴얼 설치 절에 적었다.
  - 6a(2026-10-17)에 그대로 만들었다: `x86_64-unknown-linux-musl`·`aarch64-unknown-linux-musl`(C 컴파일러 `musl-gcc`), macOS 두 대상은 Apple 실리콘 러너 하나에서.
    설치 문서의 빌드 길은 `cargo build --release --locked`(저장소에서). CI 컴파일 캐시는 아직 넣지 않았다.
- [x] 버전 → Rust 첫 릴리스는 `0.2.0`(`atom` 제거가 호환성 깨짐). 1.0은 이식 뒤에 정한다.
- [ ] 일정: 정하지 않음. 지금은 공부를 겸한 검토 단계.
