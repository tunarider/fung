# fung — 작업 안내 (Claude용)

fung은 Rust로 구현한 개인용 실용적 함수형 스크립트 언어다 (바이트코드 VM).
설계, 알파(M0~M10), Rust 이식(1~6c단계)을 마치고 0.2.0(첫 Rust 릴리스, 2026-10-18)을 냈다. 동시성 v2(언어 L1~L6, 런타임 R1~R6)를 마치고 0.3.0(2026-10-19)을, 언어·VM 검토와 매뉴얼 개편을 마치고 0.4.0(2026-10-20)을 냈다.

## 먼저 읽을 것

1. `docs/design/README.md` — 설계 문서 목차와 상태
2. `docs/design/background.md` — 배경, 결정 원칙, 기각된 대안
3. `docs/design/09-implementation-plan.md` — 구조, 마일스톤, 테스트 전략
4. `docs/design/open-questions.md` — 아직 정하지 않은 세부 사항

설계 문서가 기준이다. 구현 중 설계와 충돌하거나 문서에 없는 결정이 필요하면,
임의로 정하지 말고 **선택지 + 추천 + 이유**로 사용자에게 묻고, 결정 후 해당 문서를 갱신한다.

## 사용자

- SRE / DevOps 엔지니어. 전문 개발자는 아니고, 개발은 취미·내부 툴·스크립트 수준.
- 언어 개발은 처음이다. "내가 뭘 모르는지조차 모른다"고 했으므로, 고려해야 할 점을 **먼저 짚어서** 물어야 한다.
- 구현은 Claude가 한다. 사용자는 **이론과 "언어를 만들 때 무엇을 고려하는지"**를 알고 싶어 한다.
  단계마다 무엇을 만들었는지, 어떤 원리로 동작하는지 설명한다.
- 사용자의 제안이라도 효율·안정성 측면에서 문제가 있으면 **비판적으로 검토**해서 말한다 (사용자가 요청함).

## 소통 규칙

- 사용자와의 대화는 **한국어**.
- 요청하지 않으면 시각화(도표, 차트 등)하지 않고 텍스트로 설명한다.
- 주제를 섹션/단계 단위로 확실히 닫고 다음으로 넘어간다. 갑자기 다른 섹션으로 넘어가지 않는다.
- 결정이 필요하면 번호 붙인 질문 + 각 항목에 추천안을 붙인다.

## 저장소 규칙

- 설계 문서: 한국어. 코드, 코드 주석, 에러 메시지(카탈로그 기본), 커밋 메시지: 영어.
- 커밋: Conventional Commits, 영어. 섹션/작업 단위마다 커밋.
- git 작성자: `tunarider <tunarider@tunaground.net>` (저장소 로컬 설정). **다른 개인 이메일은 절대 커밋·문서·설정에 쓰지 않는다.**
- 원격 저장소: `origin` = https://github.com/tunarider/fung (공개). 2026-10-14에 기록을 커밋 하나로 합쳐 처음 푸시했다.
  옛 기록(커밋 195개)은 저장소 밖 `~/fung-history-2026-10-14.bundle`에만 있다.
- 의존성: 기본은 **Rust 표준 라이브러리** (REPL 줄 편집 등도 직접 구현). 크레이트는 `deny.toml`의 허용 목록만, 앞단(`fung-front`)은 0개.
  이유와 표는 open-questions "Rust 구현: 정한 것과 남은 일"의 의존성 정책. `crates/fung-cli/tests/deps.rs`·CI `cargo-deny`·`cargo-audit`가 지키고, 2주 넘은 버전만 쓴다.
  크레이트 보안 공지: 우리 설정에 닿지 않으면 2주 규칙을 지키고 `deny.toml`·`.cargo/audit.toml`에 이유와 함께 무시(해제 날짜는 open-questions에서 추적), 닿으면 바로 올린다.
- 구현은 TDD로 한다: 테스트를 먼저 쓰고 실패를 확인한 뒤 구현한다.
- **커밋 전 검사** (CI `ci.yml`도 같은 것을 돌린다):
  - `cargo fmt --check`
  - `cargo clippy --all-targets --locked -- -D warnings`와 같은 명령에 `--release`
  - `cargo test --locked --no-fail-fast`와 같은 명령에 `--release` (릴리스 전용 `perf` 테스트가 컬렉션 누적을 0.3초로 지키고, 동시성 실행 사례를 50번씩 반복한다)
  - `cargo deny check`·`cargo audit` (설치되어 있으면; CI는 버전을 고정해 돌린다)
  `cargo test` 안에 언어 골든 테스트, 기대 출력 비교, `fung fmt --check`(`fmt_repo`: `extras`·`examples`·`tests/lang`(`err_*` 일부 예외)·`crates/fung-cli/tests/data`),
  매뉴얼 생성·검사(`manual_build.rs`·`manual.rs`), `install.sh`·CHANGELOG 검사, 크레이트 허용 목록 검사가 모두 들어 있다.

## 구조와 테스트

- Cargo 작업 공간(`Cargo.toml`): `crates/fung-front`(앞단: 렉서·파서·desugar·resolve·컴파일러·포맷터·에러 카탈로그·로더),
  `crates/fung-vm`(값·VM·런타임·prelude·`_native.*` 다리), `crates/fung-cli`(실행 파일 `fung`: `run`·`-e`·`dump`·`check`·`test`·`fmt`·REPL·`version`·`help`).
  구조 설명은 `docs/design/09-implementation-plan.md`, 매뉴얼의 내부 구조(`manual/vm/internals.md`).
- `extras/*.fung`: fung으로 쓴 내장 모듈(`prelude`·`agent`·`fs`·`path`·`cli`·`conc`·`json`·`time`·`regex`·`http`). `fung-front/src/extras.rs`가 `include_str!`로 넣는다.
- `spec/builtins.txt`·`spec/errors.txt`: Rust 표(`fung_vm::boot`, `fung_front::diag::catalog`)에서 생성하고 테스트가 같은지 지킨다. 갱신은 `FUNG_UPDATE_SPEC=1 cargo test spec_file`.
  `spec/unicode.txt`(식별자·대문자·보이는 글자·표시 너비)·`spec/unicode_nfc.txt`는 유니코드 문자 데이터베이스에서 생성한다(`FUNG_UPDATE_SPEC=1 FUNG_UCD=<dir> cargo test -p fung-front unicode_spec_file`, 01). 에러 번호는 `crates/fung-front/src/diag/catalog.rs`에서 할당하며 한 번 붙이면 바꾸지 않는다.
- 골든 테스트: `tests/lang/*.fung`의 `# expect:`·`# expect-error:` 주석을 `crates/fung-cli/tests/golden.rs`가 실제 실행과 비교한다(`FUNG_GOLDEN_BIN`으로 다른 바이너리).
- 기대 출력 파일: `crates/fung-cli/tests/expected/<주제>/`·`crates/fung-vm/tests/expected/`(dump·실행·도구·json·time·regex·http·tls 등, 형식은 `tests/common/expected.rs` 머리 주석).
  각 테스트가 사례 수를 단언한다. 저장소의 `.fung` 파일(`tests/lang`·`examples`·`extras`)을 더하거나 고치면 dump·run·check/test·fmt 기대 출력과 사례 수 상수(`DUMP_CASES`·`FMT_INPUTS`·`FMT_STDIN_CASES` 등)가 함께 바뀐다.
  일부러 동작을 바꾼 뒤에는 `FUNG_EXPECTED=bless cargo test -p fung-cli --test <이름>`(fung-vm의 기대 출력은 라이브러리 단위 테스트라 이름으로 거른다: `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`)으로 다시 쓰고 **diff를 검토한 뒤** 커밋한다.
  기대 출력 실행은 `TZ=UTC`(`common::TZ`)이고, 호스트에 따라 달라지는 값(임시 경로·포트·디렉터리 크기)은 치환한다. Linux·macOS CI 둘 다 같은 파일로 통과해야 한다.
- 테스트가 띄우는 자식 프로세스(서버·클라이언트)는 `common::Reaped`로 쥐어 패닉해도 죽이고 거둔다(고아 프로세스 금지, 사용자 요청).
- 동시성: 실행 스레드 하나가 모든 태스크를 돈다(M:1, `vm/sched.rs`). 기다리는 태스크는 주차하고, 소켓·외부 명령은 kqueue/epoll 이벤트 루프(`vm/poller.rs`), 파일·이름 조회·정규식 컴파일은 스레드 풀(`vm/pool.rs`)이 맡는다.
  fung 값(`Rc`)은 실행 스레드에만 살고 타입이 지킨다(`Runtime`은 `!Send`, `World`는 `Runtime` 안의 `RefCell`). 다른 스레드는 값이 없는 `Shared`만 본다. 손으로 쓴 `unsafe impl Send`·`Sync`는 두지 않는다(`repo_rust_only`가 지킴). 설명 `docs/notes/R7-scheduler.md`.
  동시성 테스트는 `sleep` 대신 상태로 순서를 정한다(`testutil`의 탐침).
- 저장소가 Cargo만으로 빌드되고, 코드와 현재 문서가 이전 구현의 언어를 부르지 않게(언어 비교 문장만 허용 목록) `crates/fung-cli/tests/repo_rust_only.rs`가 지킨다.

## 현재 상태

- 설계: 00 ~ 09 확정 (2026-10-12~13 전체 재점검). 결정의 기록은 각 설계 문서와 open-questions.
- M0~M10 완료 (2026-10-13~14, 첫 구현을 뒤에 Rust로 옮김): 계산기·변수와 흐름·함수(모델 C: 함수는 만들어질 때 보이는 변수를 복사)·자료구조와 패턴·모듈과 stdlib·
  개발 도구(알파 기준)·동시성·네 층 구조·웹(`json`·`time`·`regex`·`http`)·포맷터·영속 컬렉션. 계획 `docs/plans/`, 설명 `docs/notes/M<n>-*.md`.
- Rust 이식 1~6c단계 완료 (2026-10-15~18): 1 앞단, 2 값·VM, 3a 동시성(스레드 + 실행권), 3b `atom` → `agent`, 4a `fs`·`path`, 4b `time`·`json`, 4c `regex`·크레이트·SIGPIPE,
  4d 소켓·TLS·fung으로 쓴 HTTP, 5 도구, 6a 릴리스 준비(`conc` 모듈, musl 정적 릴리스, CHANGELOG), 6b 첫 구현 삭제(대조를 기대 출력으로 얼림, 명세를 Rust에서 생성), 6c 이식 잔가지 정리(흉내 동작을 fung 규칙으로, 2026-10-17~18).
  설명 `docs/notes/R<n>-*.md`(이식 전체의 기록 `R-port-record.md`, 6c 정리 `R6c-cleanup.md`), 스펙 `docs/superpowers/specs/`, 결정·남은 일·알려진 한계는 open-questions "Rust 구현: 정한 것과 남은 일".
- 0.2.0 릴리스 (2026-10-18, 첫 Rust 구현).
- 동시성 v2 언어 단계 L1~L6 완료 (2026-10-18, main에 스쿼시 `3f99a5c`, 아직 미릴리스): L1 `scope`·`supervise`·`spawn(s, f)`, L2 `timeout(d, f)`, L3 `with(acquire, use, release)`(`at_exit` 삭제),
  L5 엄격한 `atom`(`agent` 삭제), L4 채널 두 끝 `(tx, rx)`와 자동 닫힘, L6 `stop_signal`·`request_stop`·SIGINT·SIGTERM·`http.serve`의 `drain`·`reuse_port`·취소된 외부 명령 끝내기·배경 태스크 삭제.
  스펙 `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md`(구속력 있음), 설명 `docs/notes/R7-language.md`, 계획 `docs/plans/2026-10-18-concurrency-v2-l*.md`.
- 동시성 v2 런타임 단계 R1~R6 완료(스펙 10·11·15절): 두 런타임을 함께 두지 않고 제자리에서 바꾸며, L단계 시험이 정답지다. 설명 `docs/notes/R7-scheduler.md`, 계획 `docs/plans/2026-10-1*-concurrency-v2-r*.md`.
  R2(이어 가는 내장 함수)·R1a(주차 요청, offload)·R1b(M:1 스케줄러)·R3(스레드 풀)·R4a(이벤트 루프와 소켓)·R4b(외부 명령, REPL 두 번째 Ctrl-C)·R5(옛 런타임 삭제)·R6(측정과 마무리, 10.4 목표 모두 됨) 끝(2026-10-19, 작업 브랜치). main에 스쿼시(`5a0061f`), 0.3.0 릴리스(2026-10-19). **다음: rustls 0.23.46(2026-10-28 이후, 0.4.1)**.
  rustls는 0.23.45로 짰고 2026-10-28 이후 0.23.46으로 버전만 올린다. 릴리스 태그와 시험 실행은 사용자 확인 뒤.
  tree-sitter·LSP·패키지 관리는 별도 세션에서 필요한 시점에. 보류 목록은 open-questions. 사용자는 언어 완성도를 편의 기능보다 우선한다.
- 언어·VM 검토 (2026-10-20): 고칠 문제 목록을 `docs/plans/2026-10-20-language-vm-review.md`에 모았다(항목마다 재현·명세 근거·원인 위치·고치는 법, 결정 항목은 선택지와 추천).
  결정 없는 버그 묶음(앞단 F1~F12, VM·값 V1~V7·V9, 동시성 C1~C7)은 고쳐 main에 스쿼시했다(2026-10-20).
  이어서 나머지 항목도 모두 처리했다(2026-10-20, main에 스쿼시, 0.4.0에 포함): 결정 L1~L11·T1~T4·V8·V10, 성능 P1·P3~P7, 문서 D1~D9.
  큰 언어 변화: 버려지는 `Err`는 E0606, 값 자리의 `else` 없는 `if`는 E0223, 변수 자리의 대문자 이름은 E0225, `map` 람다의 `?`는 E0226(`try_map`),
  파이프의 `_`는 E0227, 심볼과 문자열 비교는 E0517, 지역 변수는 마지막 읽기 뒤에 놓음(`CLEARLOCAL`), 한 번만 대입되는 최상위 이름은 최상위 `fn`의 상수,
  맵 키 자리 식별자는 쓴 그대로, 태스크 실패 값은 `{kind, code, message, at}`. P2와 P7의 맵·튜플은 open-questions "알려진 한계".
  **다음**: 검토 문서 12절의 남은 의심, open-questions의 L1·L3·L7·L9 후속 한계.
- 보수 작업 (2026-10-24, 작업 브랜치, 미릴리스): open-questions를 열린 항목만 남게 정리(닫은 것은 `docs/notes/open-questions-closed.md`), 매뉴얼 찾은 문제 F-1~F-4·F-6~F-10·F-13·D-1·D-4를 고침.
  큰 것: 시간 초과·취소·닫힘 `Err`에 `message`(호환 깨짐), 파이프 끝 `?`·`!`는 E0229, `get`은 내장 함수(E0506), `take`·`drop`이 문자열, `conc.pmap`은 작업자 n개, 상수 표 해시 색인. 남은 결정은 open-questions "남은 일".
- 작업 흐름: 이식 단계는 작업 브랜치에 쌓고 사용자가 요청할 때 main에 스쿼시 머지한다. 스쿼시 머지 뒤에는 곧바로 브랜치를 main 위로 다시 올린다(`git rebase --onto`).
- 매뉴얼: 원고는 `manual/`(목차 `toc.txt`, 마크다운 부분집합 + 날 HTML, 형식은 `docs/plans/2026-10-20-manual-1-skeleton.md` "원고 형식"), 생성 결과는 `docs/latest/`(커밋한다).
  GitHub Pages("Deploy from branch → /docs"). **문법·내장 함수·내부 구조를 바꾸면 원고를 고치고 `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`로 다시 만든 뒤 diff를 검토한다.**
  생성기(`crates/fung-cli/tests/manual_site/`)가 `fung run`·`check`·`dump` 펜스를 검사·실행하고(`run`은 두 번 돌려 출력이 같아야 한다) 실제 출력을 페이지에 넣으며, 깨진 내부 링크를 막는다.
  함수 레퍼런스는 매개변수 표와 같아야 한다(`manual.rs`). 설계 `docs/superpowers/specs/2026-10-20-manual-structure-design.md`(네 부분 모두 씀: 배우기·예제 모음·레퍼런스·VM). 원고 작성 지침 `docs/plans/2026-10-20-manual-2-content.md`, 쓰며 찾은 언어 문제 `docs/plans/2026-10-20-manual-findings.md`.
  버전: `docs/versions.js`, 옛 주소(`docs/index.html` 등)는 `latest`로 넘겨 주는 생성물이다. 0.4.0부터 릴리스 때 버전을 얼린다(`docs/notes/release.md` 7절).
- 배포: 버전은 빌드 때의 `FUNG_BUILD_VERSION` → `0.0.0-dev`, `scripts/build-release.sh`(Linux musl 정적·macOS, `SHA256SUMS`), `scripts/check-release.sh`, `install.sh`,
  `.github/workflows/release.yml`(`v*` 태그만 게시). **사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`에 적는다. 릴리스 때 그 절을 고치고 매뉴얼을 얼린다**(`changelog.rs`).
  설명 `docs/notes/release.md`, 보류는 open-questions "배포 후속". 로고는 `docs/assets/logo/`(규격 `README.md`, 스크립트 `scripts/logo/`).
- 학습용 설명은 `docs/notes/`에 한국어로 남긴다(마일스톤 `M<n>-<주제>.md`, 이식 단계 `R<n>-<주제>.md`). 무엇을 만들었고 왜 그렇게 했는지 쓴다.
