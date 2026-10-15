# fung — 작업 안내 (Claude용)

fung은 Go로 구현하는 개인용 실용적 함수형 스크립트 언어다 (바이트코드 VM).
설계는 끝났고, 구현 단계로 넘어가는 시점이다.

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
- Go 모듈 `github.com/tunarider/fung`, Go 1.27, **Go 표준 라이브러리만 사용** (REPL 줄 편집 등도 직접 구현).
- 원격 저장소: `origin` = https://github.com/tunarider/fung (공개). 2026-10-14에 기록을 커밋 하나로 합쳐 처음 푸시했다.
  옛 기록(커밋 195개)은 저장소 밖 `~/fung-history-2026-10-14.bundle`에만 있다.
- Rust 이식(진행 중): 루트 Cargo 워크스페이스, `crates/fung-front`(앞단)·`crates/fung-vm`(값·VM)·`crates/fung-cli`(`fung dump`·`run`·`-e`). 크레이트 의존성은 허용 목록(open-questions "Rust 이전")만,
  앞단·VM은 0개. `spec/builtins.txt`·`spec/errors.txt`·`spec/unicode.txt`(Go `unicode` 글자 범주 표)는 Go가 생성(`go test ./tests -run TestSpecFiles -update`). 커밋 전 `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `FUNG_REQUIRE_PARITY=1 cargo test`, `FUNG_REQUIRE_PARITY=1 cargo test --release`도 통과해야 한다(`parity`·`run_parity` 테스트가 Go 버전과
  dump·실행 출력을 비교, 이 변수가 없으면 `go`가 없을 때 비교를 조용히 건너뜀; 릴리스 전용 `perf` 테스트가 컬렉션 누적을 0.3초로 지킴).
  동시성(3a): fung 값(`Rc`)은 실행권을 쥔 스레드만 만지고, `unsafe impl Send`는 `// SAFETY:` 근거와 디버그 단언으로 지킨다(`docs/notes/R3a-runtime.md` 3절). 동시성 테스트는 `sleep` 대신 상태로 순서를 정한다(`testutil`의 탐침).
  갈라지는 테스트(3b): Go와 일부러 다른 `tests/lang` 파일은 맨 위에 `# only: go|rust`. Rust 골든 실행기(`crates/fung-cli/tests/golden.rs`)가 `# only: go`가 아닌 파일을 Go 없이 파일의 기대값과 비교하고, 실행 비교는 표시된 파일을 건너뛴다.

## 현재 상태

- 설계: 00 ~ 09 확정. 2026-10-12~13에 전체 재점검 완료 (01에 EBNF, 09에 앞단·뒷단 구조 설계 추가).
- M0 준비 완료: `go.mod`, `cmd/fung` 뼈대(하위 명령 분기, 종료 코드), `internal/diag`(종료 코드), `internal/golden` + `tests/golden_test.go`(골든 테스트 실행기).
- M1 계산기 완료: 렉서(글자 규칙 전부), Pratt 파서, 균일 트리, desugar, resolve, 컴파일러, 스택 VM, `fung -e`/`run`/`dump`, 골든 테스트 13개. 계획은 `docs/plans/2026-10-13-m1-calculator.md`, 설명은 `docs/notes/M1-calculator.md`.
- M2 변수와 흐름 완료: 대입(전역/슬롯), 블록 스코프와 E0205·E0206, `if`/`then`/`else`, 문자열·보간·심볼·기간·nil, 소스 발췌 에러 출력, `fail`, `internal/frontend`. 계획 `docs/plans/2026-10-13-m2-variables-and-flow.md`, 설명 `docs/notes/M2-variables-and-flow.md`.
- 2026-10-13 스코프 재논의: 모델 C ("함수는 만들어질 때 보이는 변수를 복사한다", 예외 없음) 확정. 03 참고. 배경 문서에 원칙 7(예외 조항 최소화) 추가.
- M3 함수 완료: `fn`·람다·익명 함수, 호출 규약(위치·이름 인자, 기본값), 프레임·클로저(값 복사)·`LOADSELF`, 끌어올린 최상위 fn, `return`, TCO, 타입 표기 검사, 스택 한도 100,000, `at_exit`, 호출 경로 출력. 계획 `docs/plans/2026-10-13-m3-functions.md`, 설명 `docs/notes/M3-functions.md`.
- M4 자료구조와 패턴 완료: 리스트·맵·튜플·태그·범위, 인덱스·슬라이스, `match`·`MATCH` 명령(패턴 상수 트리), 구조 분해(대입 규칙), guard, `!` `?` `??`, `|>`, `_`, `for`(→ `map`), `in`, `type` 별칭, `...rest`, `len`·`map`·`type_of`. 계획 `docs/plans/2026-10-13-m4-data-and-patterns.md`, 설명 `docs/notes/M4-data-and-patterns.md`.
- M5 모듈과 실사용 완료: `import`(로더가 그래프를 먼저 컴파일), 모듈 값·모듈별 전역, `_native.*` 층 + embed된 `fs.fung`·`path.fung`·`prelude.fung`, 내장 함수 호출 규약 통일, prelude(컬렉션·문자열·변환·결과값·입출력·프로세스), 파일 핸들·`with_open`, `exit` 신호, `examples/rename.fung`. 계획 `docs/plans/2026-10-13-m5-modules-and-stdlib.md`, 설명 `docs/notes/M5-modules-and-stdlib.md`.
- M6 개발 도구 완료: `test` 블록(함수로 컴파일, `fung run`은 버림)·`assert`(E0807, 양쪽 값 표시), `fung test`(파일마다 Runtime, 테스트 없는 파일은 건너뜀), `fung check`, 경고 W0101~W0103(`check`·`test`만), REPL(`internal/repl`, raw 모드 편집기 + 파이프 fallback, 재정의 허용), `dbg`, 소스 구간(`Token.Off/End`, `Node.Off/End`). 계획 `docs/plans/2026-10-13-m6-dev-tools.md`, 설명 `docs/notes/M6-dev-tools.md`.
- **알파 기준 도달** (M0~M6). 샘플 스크립트 4개(`examples/`: rename, logsum, tidy, envcheck — 각각 로직 모듈 + `test` 블록)와 그 경험에서 나온 조정(대소문자 무관 맵 키, `lines`, 매개변수 패턴, `cli` 모듈)까지 끝.
- M7 동시성 완료: `spawn`/`await`/`cancel`(context 트리, 호출마다 플래그 검사), 채널(닫지 않는 Go 채널 + `done` 신호, 순회 가능), `atom`(재진입 E0703), `sleep`, 어디서든 `exit`, 아무도 꺼내지 않은 실패는 핸들이 GC될 때나 끝날 때 보고 + 종료 코드 1(2026-10-14 개정, 05·M7 노트 13절), 교착은 50ms 감시자(E0702), 잠금 writer, `fan_in`/`pmap`/`retry`는 fung. 동시성 값·내장 함수는 `internal/vm`(`vm.Concurrency()`). 테스트는 `-race`도 통과. 계획 `docs/plans/2026-10-13-m7-concurrency.md`, 설명 `docs/notes/M7-concurrency.md`.
- M7.5 구조 완료: 네 층(06) — core(`internal/prelude`: Go 값 연산 41개 + vm의 동시성 11개), native(`internal/native`: `_native.fs`·`path`·`process` 다리, `Bridges()`), 내장 extras(`extras/`: `prelude.fung`·`fs`·`path`·`cli`, `extras.Default()` → `frontend.Registry`), 공용 도우미 `internal/nat`. 조립은 `cmd/fung`의 `registry()`만. `internal/`은 `extras/`를 import하지 않는다(`tests/layers_test.go`). 조합 함수 21개(`take`·`drop`·`lines`·`is_ok`…)는 fung으로; `flatten`은 측정 결과 Go에 남김. fung extras의 인자 검사는 `fail`, 내장 파일 안의 에러 위치는 사용자 프레임(`Proto.Builtin`). 계획 `docs/plans/2026-10-13-m7b-structure.md`, 설명 `docs/notes/M7b-structure.md`.
- M8 웹 완료 (2026-10-14): core 타입 `value.Time`(`==` `<` `± dur` `time - time`, ISO 출력), `value.Context`에 `Done()`·`RunTask()`,
  네 모듈 전부 `_native.*` 다리 + 얇은 fung(`extras/time.fung`·`json`·`regex`·`http`; `json`도 성능 때문에 Go — 06 "Go native를 쓰는 때").
  `time`(토큰 10개 직접 구현, 정규화 차단), `json`(순서 유지 토큰 디코더, int/float 구분, E0513), `regex`(룬 인덱스, 캐시, E0515,
  `is_match` — `match`는 키워드), `http`(404도 `Ok`, 소문자 헤더, `serve(addr, handler, ready)`는 요청마다 새 태스크, 취소로 종료).
  예제 `examples/serve.fung`+`api.fung`, `checkurls.fung`+`urlcheck.fung`. 계획 `docs/plans/2026-10-14-m8-web.md`, 스펙
  `docs/superpowers/specs/2026-10-14-m8-web-design.md`, 설명 `docs/notes/M8-web.md`.
- 2026-10-14 미결 정리 완료: 예약어를 이름 자리(`.` 뒤·맵 키·패턴 키)에서 허용, `{a, b}` 축약, 파일 핸들 뮤텍스, `fs.list full`, W0104,
  `fung test`의 `<load>` 집계와 에러 중복 제거, REPL 기록 파일, `parse_dur`, `MMM`·`ddd`, `http.serve` 한도. 보류 목록은 open-questions
  ("M8 후속", REPL 폭 2 글자). 노트 `docs/notes/M8-web.md` 7절.
- M9 포맷터 완료 (2026-10-14): `internal/format`(토큰 재배치 — 줄 묶기·들여쓰기 상태 기계·공백 표·정렬, 출력 토큰 열 보존 안전장치),
  `fung fmt [--check] <path>...|-`, 저장소 `.fung` 전부 포맷, `tests/fmt_test.go`가 지킴. 스펙 `docs/superpowers/specs/2026-10-14-formatter-design.md`,
  계획 `docs/plans/2026-10-14-m9-formatter.md`, 설명 `docs/notes/M9-formatter.md`. **커밋 전 `fung fmt --check extras examples tests/lang`도 통과해야 한다**(테스트가 검사).
- 매뉴얼 (2026-10-14): `docs/index.html`(기조·빠른 시작), `syntax.html`, `builtins.html`(128개 함수, 서명은 실제 매개변수 표에서), `internals.html`,
  공용 `docs/assets/`. GitHub Pages는 "Deploy from branch → /docs"로 서빙(`docs/.nojekyll`). **문법·내장 함수·내부 구조를 바꾸면 매뉴얼도 같이 고친다.**
  매뉴얼의 코드 예제(`frag`·`plain`·`core` 클래스가 아닌 `<pre>`)는 `fung check`·`fmt --check`·실행을 통과해야 한다.
- M10 영속 컬렉션 완료 (2026-10-14): 리스트는 32개 초과 시 영속 벡터(`pvec.go`) 위의 보기 `{vec, lo, hi}`, 맵은 16개 초과 시
  항목 벡터 + HAMT 색인(`omap.go`·`hamt.go`). 값 층 밖은 `Len`/`At`/`All`/`Slice`/`Append`/`Put`과 `ListBuilder`·`MapBuilder`만 쓴다.
  누적·꼬리 재귀가 선형(`tests/perf_test.go`가 0.3초로 지킴). 스펙 `docs/superpowers/specs/2026-10-14-persistent-collections-design.md`,
  계획 `docs/plans/2026-10-14-m10-persistent-collections.md`, 설명 `docs/notes/M10-persistent-collections.md`, 매뉴얼 `internals.html#collections`.
- 배포 준비 (2026-10-14): MIT `LICENSE`, 버전은 `cmd/fung/version.go`(`-X main.linkedVersion` → 모듈 버전 → `0.0.0-dev`),
  `scripts/build-release.sh <ver>`(Linux·macOS × amd64·arm64 tar.gz + `SHA256SUMS`), `install.sh`(체크섬 검증, `~/.local/bin`,
  `tests/install_test.go`가 가짜 릴리스로 검사), `.github/workflows/ci.yml`·`release.yml`(`v*` 태그 → 테스트·빌드·빌드 증명·릴리스).
  첫 버전 `v0.1.0`. **사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`에 적는다. 릴리스 때 그 절과 매뉴얼 사이드바 버전을 함께 고친다**(`tests/changelog_test.go`가 짝을 검사). 설명 `docs/notes/release.md`, 보류는 open-questions "배포 후속".
- 로고 (2026-10-14): `docs/assets/logo/` — IBM Plex Mono Medium 500을 도형으로 바꾼 `fung;;`(`;;` 간격 좁힘, 강조색 `#198f7b`),
  가로형·아이콘 밝은/어두운 판, 파비콘, 소셜 미리보기 1280×640. 규격과 다시 만드는 법은 `docs/assets/logo/README.md`, 스크립트 `scripts/logo/`.
- 성능 1·2단계 (2026-10-14): 산술 디스패치를 배열로, 매개변수 표를 제자리에서 읽기, fung 함수 호출 빠른 길(할당 0), 태스크별 인자 버퍼,
  `fold`·`map`·`each`·`filter`는 범위를 펼치지 않고(`nat.Each`) 인자 배열 재사용. fib·loop 약 2.5배. `internal/vm/alloc_test.go`가 할당 수를 지킨다.
  측정 스크립트 `bench/`, 기록과 3단계 이후 계획은 open-questions "성능".
- Rust 이식 1단계(앞단) 완료 (2026-10-15): `fung dump ast|core|bytecode`와 컴파일 에러가 Go와 글자 단위로 같다(parity 561건). 노트 `docs/notes/R1-front-end.md`. 다음은 2단계(값·VM).
- Rust 이식 2단계(값·VM) 완료 (2026-10-15): `fung run`/`-e`가 동시성·네이티브 다리 모듈(`fs`·`path`·`json`·`time`·`regex`·`http`) 밖의 프로그램을 Go와 같은 출력·종료 코드로 실행한다(dump parity 561건 이상, 실행 비교 152건, 제외 18개는 `crates/fung-cli/tests/run_parity_skip.txt`). 동시성 내장 함수는 E0805. 계획 `docs/plans/2026-10-15-rust-vm.md`, 노트 `docs/notes/R2-vm.md`. 다음은 3단계(런타임·동시성).
- Rust 이식 3a단계(동시성, Go와 같은 의미) 완료 (2026-10-15): 태스크마다 운영체제 스레드 + 실행권 잠금 하나로 `spawn`·채널·취소·`atom`·교착 감지·꺼내지 않은 실패·어디서든 `exit`가 Go와 같은 출력·종료 코드(실행 비교 187건, 제외는 네이티브 모듈 9개; 동시성 31건은 릴리스에서 Rust·Go 각 50번 반복). Go와 남은 차이는 open-questions "3a단계에서 알고 남긴 차이". 스펙 `docs/superpowers/specs/2026-10-15-rust-runtime-3a-design.md`, 계획 `docs/plans/2026-10-15-rust-runtime-3a.md`, 노트 `docs/notes/R3a-runtime.md`. 다음은 3b(`atom` 제거 등 Go와 일부러 달라지는 언어 변경).
- Rust 이식 3b단계(Go와 일부러 갈라지는 언어 변경) 완료 (2026-10-15): Rust만 `atom` → `agent`(`extras/rust/agent.fung`, 배경 태스크 + 닫개 핸들), E0704·E0705, `# only:` 표시와 Rust 골든 실행기(130개 확인). Go와의 내장 함수 차이는 `crates/fung-front/src/builtins.rs`의 목록. 0.2.0에서 바뀌는 것과 남은 한계는 open-questions "Rust 이전". 스펙 `docs/superpowers/specs/2026-10-15-rust-agent-3b-design.md`, 계획 `docs/plans/2026-10-15-rust-agent-3b.md`, 노트 `docs/notes/R3b-agent.md`.
- 다음 작업: tree-sitter·LSP·패키지 관리는 별도 세션에서 필요한 시점에(기존 결정 유지). 보류 목록은 open-questions. 사용자는 언어 완성도를 편의 기능보다 우선한다.
- 에러 번호는 `internal/diag/catalog.go`에서 할당하며 한 번 붙이면 바꾸지 않는다.
- 학습용 설명은 `docs/notes/M<n>-<주제>.md`에 한국어로 남긴다. 마일스톤마다 무엇을 만들었고 왜 그렇게 했는지 쓴다.
- 구현은 TDD로 한다: 테스트를 먼저 쓰고 실패를 확인한 뒤 구현한다. 커밋 전에 `gofmt -l .`, `go vet ./...`, `go test ./...`를 통과해야 한다.
