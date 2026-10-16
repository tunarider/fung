# Rust 이식 4d단계: 소켓·TLS·fung으로 쓴 HTTP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `_native.net`(TCP·TLS 원시 기능)을 Rust로, HTTP/1.1 클라이언트·서버를 fung(`extras/rust/http.fung`)으로 만들어 `http` 모듈이 "fung HTTP 약속" 안에서 Go와 같게 돈다. 실행 비교 제외 파일이 없어진다.

**Architecture:** 지금의 태스크당 스레드 런타임 위에서 막히는 소켓 호출을 실행권 밖(`outside`)에서 한다. 취소되면 막힌 호출이 깨어난다. TLS는 rustls(+ring) + 운영체제 루트 인증서. 프로토콜(URL·메시지·조각 전송·리다이렉트·프록시·서버 루프)은 fung.

**Tech Stack:** Rust 1.94, 크레이트 `rustls`(ring), `rustls-native-certs`(+ 딸려 오는 것). fung.

**Spec:** `docs/superpowers/specs/2026-10-16-rust-net-http-4d-design.md`

## Global Constraints

- 크레이트는 `deny.toml` 허용 목록만, 새 의존성은 `crates/fung-vm/Cargo.toml`에만, 나온 지 2주 넘은 판. `fung-front` 0개, `fung-cli`는 작업 공간 크레이트만(`tests/deps.rs`). Go 구현(`cmd/`, `internal/`, `extras/*.fung`)은 수정하지 않는다(`extras/rust/`는 Rust 전용).
- 커밋 전: `gofmt -l .`(빈 출력), `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(디버그와 `--release`),
  `FUNG_REQUIRE_PARITY=1 cargo test --locked`(디버그와 `--release`), 크레이트를 바꿨으면 `cargo-deny check`·`cargo-audit`(스크래치패드의 `*/bin/`).
- `fung fmt --check`를 통과하는 `.fung`(`extras/rust/*.fung`, 새 `tests/lang` 파일). Rust 쪽 포매터가 없으면 Go `fung fmt --check`로 검사.
- 코드·주석·에러 문구·커밋 메시지는 영어, 학습 노트·설계 문서는 한국어. Conventional Commits, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 다른 이메일 금지.
- **실행권 규칙**: fung 값(`Rc`)은 실행권을 쥔 스레드만. 소켓 입출력은 실행권 밖에서, 실행권 밖으로는 Rust 값만. `unsafe`는 `// SAFETY:`.
- 동시성 테스트는 `sleep` 대신 상태로 순서를 정한다(`testutil` 탐침).
- 표시 없는 `tests/lang` 파일은 Go 대조에서 Go와 바이트 단위로 같다. 새 에러 번호를 만들지 않는다.
- Go를 흉내 내려고만 붙이는 것은 보고서에 적고 코드에 open-questions "Go 동등을 위한 과도기 잔가지"를 가리키는 주석.

## Review Focus

1. 취소: `accept`·`read_*`·`connect`·TLS 핸드셰이크에서 막힌 태스크를 `cancel`하거나 `exit`하면 바로 끝나고, 스레드·소켓이 새지 않는다(서버 태스크 취소 시 처리 중인 핸들러 태스크 포함) → Task 1, Task 5.
2. 악의적이거나 망가진 상대: 헤더를 천천히 보내는 클라이언트(10초 한도), 끝없는 헤더 줄·본문, 잘못된 조각 크기, `Content-Length`와 다른 실제 길이, 중간에 끊긴 연결에서 패닉·무한 대기·무한 메모리가 없다 → Task 3, Task 5.
3. 한글 등 여러 바이트 글자가 든 본문·헤더에서 `Content-Length`(바이트)와 조각 크기가 맞다 → Task 3, Task 5.
4. `NO_PROXY` 패턴(`*`, `.example.com`, `example.com`, 포트 붙은 것, IP), 대소문자 환경변수 우선순위가 Go와 같다 → Task 4.
5. 한 서버에 동시에 여러 요청이 와도 각자 태스크에서 돌고, 느린 핸들러 하나가 다른 요청을 막지 않는다 → Task 5.

---

### Task 1: `_native.net` TCP 원시 기능

**Files:** Create `crates/fung-vm/src/native/net.rs`(+ 테스트), Modify `value.rs`(연결·리스너 값 종류: 출력·`type_of`·같음은 파일 핸들(`Value::File`)의 방식을 따름), `native.rs`, `boot.rs`(`_native.net` 등록 — 사용자 모듈로는 아직 노출하지 않음), 취소 연결부(`vm/`의 취소 노드·`outside`)
- Go 참조: `internal/native/http.go`의 에러 분류, Go `net` 에러 문구(`dial tcp <addr>: connect: connection refused`, `lookup <host>: no such host`, `listen tcp <addr>: bind: address already in use`), 4a의 `native/oserr.rs`·`native/file.rs`(핸들 수명·잠금·`outside` 패턴)

**Interfaces:**
- Produces(`_native.net`, fung에서 부름): `connect(host, port, tls = false, timeout = 0s)` → `Ok(conn)`/`Err({kind: 'network | 'timeout, message})`; `listen(addr)` → `Ok(listener)`/`Err`; `local_addr(l)` → 문자열(`"127.0.0.1:53121"`); `accept(l)` → `Ok(conn)`/`Err`; `read_line(c, limit)` → `Ok(줄)`(줄 끝 제거, `limit` 바이트 넘으면 `Err`) / `Ok(nil)`(EOF); `read_bytes(c, n)` → `Ok(문자열)`(정확히 n바이트, 모자라면 `Err`); `read_all(c, limit)` → `Ok(문자열)`; `write(c, s)` → `Ok(nil)`; `close(c)` → `Ok(nil)`; `set_deadline(c, dur)`(0이면 없음, 넘으면 `Err 'timeout`); `byte_len(s)` → 정수. 문자열은 UTF-8이 아니면 U+FFFD.
- Task 2가 `connect`의 `tls = true`를 채운다(이 작업에서는 `tls = true`면 `Err`).

- [ ] **Step 1: 실패하는 테스트** — 로컬 리스너로: 주고받기, 부분 도착(쓰는 쪽이 나눠 써도 `read_line`·`read_bytes`가 모아 읽음), `limit` 초과, EOF, 닫힌 연결에 쓰기, 연결 거부·호스트 없음·바인드 실패 문구(Go와 대조: 스크래치패드의 작은 Go 프로그램으로 문구 확인), `byte_len("한a") == 4`, 기한 초과, 핸들 마지막 사본에서 닫힘(상대가 EOF를 봄). **취소**: `accept`·`read_line`·`connect`(응답 없는 주소 대신 상태로 제어 가능한 방법)에서 막힌 태스크를 `cancel`하면 E0701로 곧 끝나고 소켓·스레드가 남지 않는다(Review Focus 1).
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm net` / Expected: FAIL.
- [ ] **Step 3: 구현** — 블로킹 `std::net` + `outside`. 취소 깨우기는 취소 노드에 거는 깨우개(소켓 `shutdown(Both)` 또는 리스너는 자기 연결 등)로; 방법과 경쟁 조건 근거를 주석과 보고서에.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): add TCP primitives for the net bridge`

---

### Task 2: TLS (`rustls`)

**Files:** Modify `crates/fung-vm/Cargo.toml`, `deny.toml`(크레이트·라이선스), `Cargo.lock`, `native/net.rs`(`tls = true`), Create `crates/fung-cli/tests/data/tls/`(테스트 CA·서버 인증서·키, 긴 유효 기간) + 생성 스크립트 `scripts/gen-test-certs.sh`(openssl), Test `native/net_tests.rs`·`crates/fung-cli/tests/tls.rs`
- Go 참조: Go `crypto/tls` 클라이언트 에러 문구(`x509: certificate signed by unknown authority`, `x509: certificate is valid for X, not Y`), `crypto/x509` 루트 탐색(`SSL_CERT_FILE`·`SSL_CERT_DIR`)

**Interfaces:**
- Consumes: Task 1 `connect`.
- Produces: `connect(host, port, tls = true, …)`이 SNI와 운영체제 루트(또는 `SSL_CERT_FILE`·`SSL_CERT_DIR`)로 검증한 TLS 연결을 준다. 나머지 원시 기능은 TLS 연결에서도 같게 동작한다. 루트 저장소는 프로세스당 한 번 읽는다(Rust 값만).

- [ ] **Step 1: 실패하는 테스트** — 테스트 안에서 rustls 서버(테스트 인증서)를 띄워: 신뢰(`SSL_CERT_FILE`) 시 주고받기 성공, 신뢰 안 함 → `Err 'network` + Go 문구, 이름 불일치 → Go 문구, 핸드셰이크 중 취소 → E0701. Go 대조는 Linux에서만(`#[cfg(target_os = "linux")]`), macOS는 Rust만.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm net` / Expected: FAIL.
- [ ] **Step 3: 구현** — `rustls`(기본 기능 끄고 `ring`·`std`·`tls12`·로깅 없음), `rustls-native-certs`. `cargo-deny check`·`cargo-audit`.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): add TLS client connections with rustls`

---

### Task 3: HTTP 클라이언트(fung) — 메시지·본문·시간 초과·에러

**Files:** Create `extras/rust/http.fung`(+ 필요하면 `extras/rust/url.fung`, 사용자에게 보이지 않는 이름은 `_` 접두사 등 기존 관례), Modify Rust 부팅(`http`를 지원 안 함 목록에서 빼고 `extras/rust/http.fung`를 `http`로, `agent.fung`을 읽는 방식과 같게), `crates/fung-front/src/builtins.rs`의 Rust 전용 차이 목록(필요하면), `boot/tests.rs`(매개변수 표가 `spec/builtins.txt`의 `http`와 같음), Create `crates/fung-cli/tests/http_parity.rs`(가짜 서버 + Go·Rust 클라이언트 대조)
- Go 참조: `internal/native/http.go`(`doRequest`·`requestError`·`headerMap`·`requestArgs`), Go `net/http` 클라이언트 동작

**Interfaces:**
- Consumes: `_native.net`.
- Produces: `http.get(url, headers = {}, timeout = 30s)`, `http.post(url, body, headers = {}, timeout = 30s)`, `http.request(method, url, body = nil, headers = {}, timeout = 30s)` — 스펙 2절 "클라이언트"(리다이렉트·프록시 제외). `serve`는 Task 5까지 E0805 또는 미정의가 아니라 **아직 없음을 알리는 실패**로 두지 말고, 이 작업에서는 `serve`를 등록하지 않아도 되는지 확인하고 판단을 보고서에.

- [ ] **Step 1: 실패하는 테스트** — Go 대조(가짜 서버는 Rust 테스트 코드, 같은 서버에 Go fung·Rust fung 클라이언트): 상태·헤더(소문자, 같은 이름 `", "`)·본문, `Content-Length`/조각 전송/연결 닫힘까지 읽기, `HEAD`·204·304, 한글 본문(Review Focus 3), 사용자 헤더가 기본 헤더를 덮음, 가짜 서버가 받은 요청 원문(시작 줄·`Host`·`User-Agent`·`Content-Length`) 비교, 연결 거부·잘못된 URL·호스트 없음 문구, 시간 초과(가짜 서버가 응답을 미룸, 상태로 제어), 인자 타입 에러. 망가진 응답(잘못된 상태 줄, 잘못된 조각 크기, 실제보다 큰 `Content-Length`, 끝없는 헤더 줄) → 패닉·무한 대기 없이 Go와 같은 종류의 `Err`(Review Focus 2).
- [ ] **Step 2: 실패 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli --test http_parity` / Expected: FAIL.
- [ ] **Step 3: 구현** — fung으로. 성능상 꼭 필요한 것만 `_native.net`에 더하고(더하면 이유를 보고서에), Go 문구 흉내는 표시.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): write the HTTP client in fung`

---

### Task 4: 리다이렉트와 프록시

**Files:** Modify `extras/rust/http.fung`, `crates/fung-cli/tests/http_parity.rs`
- Go 참조: Go `net/http/client.go`(`redirectBehavior`, `shouldCopyHeaderOnRedirect`, 10번 한도와 문구), `net/http/transport.go`·`golang.org/x/net/http/httpproxy`(`ProxyFromEnvironment`, `NO_PROXY` 규칙, 대소문자 변수 우선순위, `CONNECT`)

**Interfaces:**
- Consumes: Task 3 클라이언트, Task 2 TLS.
- Produces: 스펙 2절의 리다이렉트·프록시 규칙.

- [ ] **Step 1: 실패하는 테스트** — Go 대조: 301·302·303(POST → GET, 본문 버림, `HEAD` 유지)·307·308(유지), 상대 `Location`, 다른 호스트로 넘어갈 때 `Authorization`·`Cookie` 버림(가짜 서버가 받은 헤더 비교), 10번 초과 문구, 프록시(가짜 프록시: http 절대 URL 요청, https `CONNECT` 후 TLS — https는 Linux에서 Go 대조), `NO_PROXY` 패턴 표(Review Focus 4), 루프백은 프록시 안 거침, 대소문자 변수 우선순위. 프록시 환경변수는 자식 프로세스 `Command`에만 설정.
- [ ] **Step 2: 실패 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli --test http_parity` / Expected: FAIL.
- [ ] **Step 3: 구현**
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): follow redirects and proxies like Go`

---

### Task 5: HTTP 서버(fung)

**Files:** Modify `extras/rust/http.fung`(`serve`), Create `tests/lang/http_serve_rust.fung`(`# only: rust`, `http_serve.fung`와 같은 내용을 `agent`로), Modify `crates/fung-cli/tests/run_parity_skip.txt`(마지막 줄 삭제; 파일·장치를 지울지 판단해 보고서에), 최소 건수, `http_parity.rs`(서버 대조), 지원 안 함(E0805) 장치가 비면 그 처리
- Go 참조: `internal/native/http.go`(`httpServeFn`·`serveOne`·`requestValue`·`responseParts`, 한도 상수), Go `net/http` 서버(`Date` 형식, 400·413 응답 본문, `Shutdown`), `url.ParseQuery`

**Interfaces:**
- Consumes: `_native.net`(`listen`·`accept`·`set_deadline`), `spawn`.
- Produces: `http.serve(addr, handler, ready = nil)` — 스펙 2절 "서버".

- [ ] **Step 1: 실패하는 테스트** — Go 대조(같은 원시 요청을 Go fung 서버와 Rust fung 서버에 보내 응답 비교, `Date` 값만 가림): 문자열·맵 응답, 기본 `content-type`, `Content-Length`(한글 본문), 쿼리 디코딩(`%`·`+`·여러 값 중 첫 값), 헤더 소문자, 조각 전송 요청 본문, 400(형식 오류), 413(16MB 초과, 핸들러 안 불림), 500 + stderr 한 줄, 503(핸들러 안 `exit`·취소), 바인드 실패. 헤더 10초 한도는 시간을 주입할 수 없으면 Rust만 따로(느린 클라이언트를 상태로 제어). **동시성**: 느린 핸들러가 다른 요청을 막지 않음(Review Focus 5), 서버 태스크 `cancel` → 처리 중 요청 최대 2초 기다린 뒤 E0701, 스레드·소켓 남지 않음(Review Focus 1). 골든 `http_serve_rust`가 Rust 골든 실행기에서 통과. 예제 `examples/serve.fung`·`api.fung`·`checkurls.fung`·`urlcheck.fung`·`static.fung`이 Rust로 도는지 확인(`atom`이 들어 있으면 그 예제는 보고서에 적고 6단계로).
- [ ] **Step 2: 실패 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli` / Expected: FAIL.
- [ ] **Step 3: 구현**
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): serve HTTP from fung`

---

### Task 6: 문서

**Files:** Create `docs/notes/R4d-net-http.md`, Modify `docs/design/open-questions.md`, `docs/design/06-*.md`(HTTP 절), `CLAUDE.md`

- [ ] **Step 1: 학습 노트**(한국어, `R4c-regex-crates.md`의 문체): 1. 무엇을 만들었나 2. 소켓과 막히는 입출력, 실행권 밖에서 기다리기, 취소로 깨우기 3. TLS와 인증서 신뢰 4. HTTP/1.1 메시지 구조와 본문 길이 세 가지 5. 리다이렉트·프록시·터널 6. fung으로 프로토콜을 쓴 경험(편했던 것·모자랐던 것) 7. 그린 스레드를 미룬 이유 8. 실제로 부딪힌 함정(진행 기록·보고서·리뷰에 있는 것만).
- [ ] **Step 2: open-questions "Rust 이전"** — 4d 완료 한 줄, 허용 목록 표에 TLS 크레이트, **"R7 스케줄러" 항목**(그린 스레드·`mio`·태스크 생성 비용·고차 함수 구현 언어·스택 크기·출력 재귀), 과도기 잔가지(`User-Agent`, Go 에러 문구, Go `Date` 형식), 알려진 차이, "0.2.0에서 바뀌는 것"에 약속 밖 차이.
- [ ] **Step 3: 06 HTTP 절** — "리다이렉트·https는 Go 기본" 대신 "fung HTTP 약속"(스펙 2절). 공유 상태 예시의 `atom`은 6단계 몫이므로 그대로 두되 Rust에서는 `agent`라는 메모.
- [ ] **Step 4: CLAUDE.md** — 현재 상태에 4d 한 줄(다음은 5단계 도구), 크레이트 규칙 문구 확인.
- [ ] **Step 5: 전체 검사와 커밋** — `docs: record Rust port stage 4d`
