# Rust 이식 6c-5 (네트워크·HTTP) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-5 결정표(N1~N4, H1~H8)대로 네트워크·TLS·HTTP의 Go 흉내 문구와 동작을 fung 규칙으로 바꾸고, Go 이름·주석을 걷어낸다.

**Architecture:** `_native.net`은 사용자가 직접 쓸 수 없고 `extras/http.fung`만 쓴다. 그래서 네이티브 쪽(`native/net.rs`·`sock.rs`·`tls.rs`)은 **짧은 이유만** 돌려주고, `http.fung`이 ``cannot request `url`: <이유>``·``cannot listen on `addr`: <이유>``로 감싼다. HTTP 규칙(RFC 9110·9112)은 남기고 문구·`User-Agent`·시간 한도·리다이렉트 본문 처리·서버의 스스로 내는 답만 바꾼다.

**Tech Stack:** Rust 표준 라이브러리, rustls(이미 허용), fung(`extras/http.fung`).

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` (4절 "6c-5 네트워크·HTTP")

## Global Constraints

- 앞 단계들의 Global Constraints를 따른다. bless: `.superpowers/sdd/6c-tools/bless.sh http tls sigpipe`, fung-vm은 `runs_match_the_expected_output`. `http`는 22초쯤 걸린다.
- 네트워크 테스트의 자식 프로세스는 `common::Reaped`로 거둔다.
- 이유 문구 표(네이티브가 돌려주는 것):
  - 연결: `connection refused`(그 밖은 6c-2 OS 문구), 이름: `no such host`, 시간: `timed out`, 닫힌 연결: `connection is closed`,
  - 정확한 길이 읽기: `connection closed after N of M bytes`, 줄 한도: `line longer than N bytes`(그대로),
  - 포트·주소: ``invalid port `x` ``, ``invalid address `x` ``(`missing port` 등은 이유를 괄호로),
  - TLS: `certificate has expired`, `certificate is not valid yet`, `unknown certificate authority`, ``certificate is not valid for `host` ``, `TLS alert: <IANA 이름>`, `TLS handshake timed out`, `the server did not answer with TLS`, `TLS error: <rustls 문구>`.
- HTTP 문구(`http.fung`): 안쪽의 `Get "url": `·`dial tcp …: connect: `·`net/http: HTTP/1.x transport connection broken: `·`proxyconnect tcp: ` 접두사를 없앤다. 프록시를 거친 실패는 ``proxy `host:port`: <이유>``. URL 에러는 ``invalid URL: <이유>``, 스킴은 ``unsupported scheme `ftp` ``, 리다이렉트는 `too many redirects (limit 10)`·``invalid Location header `…` ``.

## Review Focus

- `timeout`이 연결·TLS·응답 전체를 덮는다: 느린 TLS 서버에 `timeout: 1s`면 1초쯤에 `Err('timeout)`(예전처럼 10초 상한이 아니라). → Task 4 테스트.
- 리다이렉트 본문을 읽지 않으므로, 본문을 끝없이 보내는 서버의 30x도 곧바로 다음 홉으로 간다. → Task 5 테스트.
- 사용자 헤더 순서: `{"X-B": 1, "X-A": 2}`는 그 순서로 나간다(정규화는 그대로). → Task 4.
- 모르는 상태 코드(`599`)의 서버 응답 줄은 `HTTP/1.1 599 \r\n`(빈 이유 문구). → Task 6.
- `bad5`가 결정적이다: 같은 테스트를 여러 번 돌려도 같은 결과. → Task 7.

---

### Task 1: 네트워크 이유 문구 (N1, N2)

**Files:** `crates/fung-vm/src/native/net.rs`, `sock.rs`, `net_tests.rs`; `extras/http.fung`에서 네이티브 문구를 다시 감싸던 곳.

- [ ] **Step 1: 실패하는 테스트**(`net_tests.rs`): 닫힌 포트 connect의 메시지가 `connection refused`, 없는 이름이 `no such host`, 기한 넘김이 `timed out`(kind `'timeout`), 닫은 연결의 read가 `connection is closed`, `read_exact` 중간 끊김이 `connection closed after 3 of 10 bytes`, 잘못된 포트가 ``invalid port `abc` ``, 두 번째 close가 `connection is closed`. 기존 Go 문구 단언은 새 문구로 고친다.
- [ ] **Step 2: 실패 확인**. **Step 3: 구현**(OpError 꼴 `format!`을 위 표로; `accept4`/`accept` 구분 삭제). **Step 4:** vm lib, `bless.sh http tls`(HTTP 문구는 Task 4에서 다시 정리하므로 여기서는 diff가 이유 부분만 바뀌는지 본다). **Step 5: 커밋** `refactor(vm): give network errors short fung reasons`.

### Task 2: TLS (N3)

**Files:** `native/tls.rs`(`GoOrder` 삭제 → `ClientConfig::with_root_certificates`, `reason`·`x509`·`hostname_error`·`rfc3339` 다시 쓰기, 알림 이름 표는 IANA 이름이라 남김), `tls_tests.rs`, `crates/fung-cli/tests/tls.rs`.

- [ ] **Step 1: 실패하는 테스트**(`tls_tests.rs`): 만료 인증서 → `certificate has expired`, 다른 이름 → ``certificate is not valid for `other.test` ``, 모르는 발급자 → `unknown certificate authority`, 받은 알림 → `TLS alert: handshake failure`, 핸드셰이크 기한 → `TLS handshake timed out`(kind `'timeout`).
- [ ] **Step 2~4:** 실패 확인, 구현, vm lib·`bless.sh tls http`. 검사 순서가 바뀌어 "다른 이름 + 모르는 발급자"는 이제 `unknown certificate authority`다(rustls 순서).
- [ ] **Step 5: 커밋** `refactor(vm): verify TLS with rustls's own verifier and fung's texts`.

### Task 3: SIGPIPE 주석과 테스트 TLS 서버 (N4, H7)

**Files:** `crates/fung-vm/src/sigpipe.rs`(주석), `crates/fung-vm/src/tls_test_server.rs` → `crates/fung-cli/tests/common/tls_server.rs`, `crates/fung-vm/src/lib.rs`(모듈 삭제), `crates/fung-cli/Cargo.toml`(dev-dependency `rustls`·필요한 것), `crates/fung-cli/tests/deps.rs`(허용), `crates/fung-cli/tests/{http,tls}.rs`(경로).

- [ ] **Step 1:** 옮기고 `cargo test -q -p fung-cli --test tls --test http --test deps`가 통과하는지 본다(동작 불변 이동이라 RED 없음; 진행 기록에 판단으로 적는다).
- [ ] **Step 2: 커밋** `refactor: move the test TLS server into the fung-cli tests`.

### Task 4: HTTP 클라이언트 (H1, H2, H3)

**Files:** `extras/http.fung`(요청 쓰기, `User-Agent`, 헤더 순서, 시간 한도, 에러 조립 `_failed`·`_err`·`_broken`·`op`, URL 에러), `crates/fung-cli/tests/http.rs`의 기대(문구가 든 단언).

- [ ] **Step 1: 실패하는 테스트**(`http.rs` 사례): 요청 원문의 `User-Agent: fung/0.0.0-dev`(테스트 빌드의 버전), 헤더 순서 `X-B` 다음 `X-A`, 닫힌 포트의 메시지 ``cannot request `http://127.0.0.1:$P/x`: connection refused``, 느린 TLS 서버 + `timeout: 1s`가 `'timeout`.
- [ ] **Step 2~4:** 실패 확인, 구현, `bless.sh http tls`(diff: `User-Agent`, 문구, 헤더 순서).
- [ ] **Step 5: 커밋** `feat(http): name fung in User-Agent, keep header order, one timeout`.

### Task 5: 리다이렉트와 프록시 (H4, H5)

- [ ] **Step 1: 실패하는 테스트:** 끝없는 본문을 보내는 302 뒤 목적지에 1초 안에 닿는다, 11번째 리다이렉트가 `too many redirects (limit 10)`, 프록시 연결 실패가 ``proxy `127.0.0.1:$P`: connection refused``.
- [ ] **Step 2~4:** 실패 확인, 구현(본문 버리기 삭제, 문구, `CONNECT`의 `User-Agent`·기한), `bless.sh http`.
- [ ] **Step 5: 커밋** `feat(http): close redirect bodies unread and word redirect and proxy errors`.

### Task 6: HTTP 서버 (H6)

- [ ] **Step 1: 실패하는 테스트:** 상태 599 응답 줄이 `HTTP/1.1 599 \r\n`, 서버가 스스로 내는 400 본문이 `400 Bad Request\n`, 요청 머리 1 MiB 한도(1 MiB + 1바이트는 431).
- [ ] **Step 2~4:** 실패 확인, 구현, `bless.sh http`.
- [ ] **Step 5: 커밋** `refactor(http): word the server's own answers in fung's terms`.

### Task 7: 결정적인 `bad5` (H8)

- [ ] **Step 1:** `crates/fung-cli/tests/http.rs`의 `refused_port()`를 "테스트가 쥔 채 `accept`하지 않고 닫아 둔 포트"에서, **테스트가 쥔 리스너가 연결을 받자마자 닫는 서버**로 바꿔 결과(`connection closed …` 또는 해당 이유)가 매번 같게 한다. 같은 테스트를 5번 돌려 diff가 없는지 본다.
- [ ] **Step 2: 커밋** `test(http): make the refused redirect target deterministic`.

### Task 8: 정리

- [ ] **Step 1:** `extras/http.fung`의 Go 주석 약 150줄을 HTTP 규칙의 말로(RFC 절 이름을 출처로) 다시 쓴다. `native/net.rs`·`sock.rs`·`tls.rs`·`sigpipe.rs`의 Go 주석.
- [ ] **Step 2:** 06 "HTTP" 절, 매뉴얼 `http` 절(`User-Agent` 줄, 시간 한도), CHANGELOG(알려진 차이의 시간 한도 줄 갱신 포함), open-questions 4d 항목 다섯 개와 "6c: 이름과 주석"의 `tls_test_server`·기대 출력 줄을 해결로.
- [ ] **Step 3:** `cargo test -q -p fung-vm --lib`, `cargo test -q -p fung-cli --test http --test tls --test manual`, 커밋 `docs: close the stage 6c-5 items`.
