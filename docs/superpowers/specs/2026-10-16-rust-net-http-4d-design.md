# Rust 이식 4d단계: 소켓·TLS·fung으로 쓴 HTTP — 설계

2026-10-16. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-16-rust-regex-crates-4c-design.md`(4c).

## 목표

Rust 구현에 네트워크를 넣어 `http` 모듈(`get` `post` `request` `serve`)이 "fung HTTP 약속"(2절) 안에서 Go와 같은 결과를 낸다.
Rust는 소켓·TLS 원시 기능만(`_native.net`) 주고 HTTP/1.1 프로토콜은 fung으로 쓴다. 그린 스레드는 이 단계에서 하지 않는다(R7).
이로써 실행 비교에서 빼던 파일이 없어진다.

## 결정 사항 (2026-10-16)

| 항목 | 결정 |
|---|---|
| 범위 | 지금의 스레드 런타임 위에서 네트워크만. 막히는 소켓 호출은 실행권 밖에서(`outside`, 파일과 같음). **그린 스레드는 6단계(Go 삭제) 뒤 별도 단계 "R7 스케줄러"** — 언어 의미를 바꾸지 않는 내부 교체이고 `block()` 자리가 있다. 람다를 부르는 고차 함수의 구현 언어, 스택 크기도 그때 다시 판단 |
| 크레이트 | `mio`는 들이지 않는다(R7에서). TLS는 `rustls` + 암호 `ring` + 운영체제 인증서 저장소(`rustls-native-certs`, `SSL_CERT_FILE`·`SSL_CERT_DIR` 지원). 서버 TLS는 없음(Go에도 없음) |
| Go와 같음의 범위 | "fung HTTP 약속"(2절)을 06에 적고 그 안에서 Go와 같게. 압축·연결 재사용·HTTP/2는 하지 않음. 약속 밖 차이는 "0.2.0에서 바뀌는 것" |
| 구현 언어 | Rust는 `_native.net` 원시 기능, HTTP(URL 해석·요청 조립·응답 해석·조각 전송·리다이렉트·프록시·서버 루프)는 fung. `extras/rust/http.fung`(Rust 부팅만 읽음, `agent.fung`과 같은 자리) |

## 1. 구조

| 무엇 | 어디 | 기준(Go) |
|---|---|---|
| 크레이트 | `crates/fung-vm/Cargo.toml`: `rustls`(기본 기능 끄고 `ring`·`std`·`tls12`), `rustls-native-certs`. 딸려 오는 크레이트·라이선스를 `deny.toml`에. 2주 규칙. `ring`은 C 컴파일러 필요(CI 러너에 있음) | |
| `_native.net` | `crates/fung-vm/src/native/net.rs`: `connect(host, port, tls = false, timeout = 0s)`, `listen(addr)`, `local_addr(l)`, `accept(l)`, `read_line(c)`(CRLF 또는 LF까지, 끝 표시 제거), `read_bytes(c, n)`(정확히 n바이트), `read_all(c)`, `write(c, s)`, `close(c)`, `byte_len(s)`(UTF-8 바이트 수). 결과는 `Ok`/`Err({kind, message})`. 기한(시간 초과)은 호출 인자나 연결별 설정 중 구현 계획에서 정한다 | Go `net`, `crypto/tls` |
| 연결 값 | `Value`의 새 종류 둘(연결, 리스너). 파일 핸들처럼 핸들마다 잠금, 입출력은 실행권 밖, 마지막 사본이 사라지면 닫힘. `_native` 밖으로는 `http.fung`만 다룬다 | |
| 취소 | 태스크가 취소되면 그 태스크가 막힌 소켓 호출(`accept`·`read_*`·`connect`)이 깨어나 E0701로 끝난다. 깨우는 방법(소켓 `shutdown`, 취소 노드에 거는 깨우개 등)은 구현 계획에서 | Go `context` 취소 |
| HTTP | `extras/rust/http.fung`(+ 필요하면 같은 자리의 URL 모듈). 공개 함수는 Go와 같은 넷, 매개변수 표는 `spec/builtins.txt`와 같다. Go `extras/http.fung`·`internal/native/http.go`는 그대로 | `internal/native/http.go`, Go `net/http` |
| 모듈 등록 | `http`를 지원 안 함(E0805) 목록에서 빼고 Rust 부팅이 `extras/rust/http.fung`를 `http`로. 지원 안 함 목록이 비면 그 장치를 지울지 남길지 계획에서 정한다 | |

## 2. fung HTTP 약속 (06에 적는다)

**클라이언트** (`get` `post` `request`)
- HTTP/1.1. 요청에 `Host`, `User-Agent: Go-http-client/1.1`(Go 기본값, 과도기 잔가지), 본문이 있으면 `Content-Length`. 사용자 헤더가 같은 이름이면 사용자 것.
- 응답 본문은 `Content-Length`, `Transfer-Encoding: chunked`, 연결 닫힘까지 중 하나로 읽는다. `HEAD` 응답과 1xx·204·304는 본문 없음.
- 응답은 상태 코드와 무관하게 `Ok({status, headers, body})`. `headers` 키는 소문자, 같은 이름은 `", "`로 잇는다(지금과 같음).
- **리다이렉트**(Go `Client` 규칙): 301·302·303 → `GET`(원래 `HEAD`면 `HEAD`), 본문 버림. 307·308 → 메서드·본문 유지. 최대 10번, 넘으면 Go와 같은 문구의
  `Err 'network`. 다른 호스트로 넘어가면 `Authorization`·`Cookie` 헤더를 버린다. `Location`이 상대 경로면 현재 URL 기준으로 푼다.
- **프록시**(Go `ProxyFromEnvironment`): `HTTP_PROXY`/`http_proxy`(http용), `HTTPS_PROXY`/`https_proxy`(https용), `NO_PROXY`/`no_proxy`.
  `localhost`·루프백 주소는 거치지 않는다. http는 절대 URL 요청, https는 `CONNECT` 터널 뒤 TLS.
- **https**: rustls + 운영체제 루트 인증서. SNI. 인증서 오류는 `Err 'network`.
- **`timeout`**: 리다이렉트를 포함한 요청 전체(Go `Client.Timeout`). 넘으면 `Err({kind: 'timeout, message: "request to `<url>` timed out"})`. `0s`면 제한 없음.
- **에러 문구**: `cannot request `<url>`: <이유>`. 흔한 이유(연결 거부, 호스트 없음, 인증서 오류, 리다이렉트 초과, 잘못된 URL)는 Go 문구에 맞추고 나머지는 알려진 차이.
- 압축 요청(`Accept-Encoding`)을 보내지 않는다 → 본문은 Go가 풀어 준 것과 같다. 연결 재사용·HTTP/2 없음.
- 취소되면 E0701.

**서버** (`serve(addr, handler, ready = nil)`)
- 바인드 실패 `Err 'network`(Go 문구). 리스닝 직후 `ready(실제 주소)`. 끝나는 길은 태스크 취소 또는 `exit`.
- 연결마다 요청 하나: 읽고, 새 태스크에서 핸들러, 응답에 `Connection: close`. 응답 헤더 `Date`(Go 형식), `Content-Length`, 본문이 있고 `content-type`이 없으면
  `text/plain; charset=utf-8`(지금 Go와 같음).
- `req = {method, path, query, headers, body}`(`query`는 첫 값만, 퍼센트 디코딩은 Go `url.ParseQuery`와 같게).
- 한도: 헤더를 10초 안에 다 보내지 않으면 끊음, 본문 16MB 초과 413(핸들러 안 부름), 형식 오류 400. 조각 전송 요청 본문도 받는다.
- 핸들러 실패·다른 타입 → 500 + stderr `http: handler failed: …`, 핸들러 안 `exit`·취소 → 503(보고 없음). 응답 본문 문구는 Go와 같게.
- 취소되면 리스너를 닫고 처리 중 요청을 최대 2초 기다린 뒤 E0701(Go `Shutdown`).

**약속 밖(알려진 차이, 0.2.0에서 바뀌는 것)**: 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축, 바이너리 본문(UTF-8이 아니면 U+FFFD, 4a와 같은 원인),
Go가 덧붙이는 그 밖의 헤더, 흔하지 않은 에러 문구.

**2026-10-16 구현 중 결정(사용자).**
- 응답 본문 상한 **64MiB**(넘으면 `Err`). Go에는 없는 동작이라 06의 약속과 "0.2.0에서 바뀌는 것"에 적는다. 더 큰 응답은 나중에 요청별 `max_body` 같은 옵션으로.
- 크레이트 보안 공지: 공지가 우리 설정에 **닿지 않으면** 2주 규칙을 지키고(이유를 적어 무시, 해제 날짜를 open-questions에서 추적), **닿으면** 2주를 기다리지 않고 바로 올린다.
  rustls 공지 4건(RUSTSEC-2026-0346~0349)은 닿지 않음(세션 재개 끔, ECH·brotli 없음, ring에 MLKEM 없음) → 0.23.45 유지, 2026-10-28 이후 0.23.46으로.

## 3. 검증

- `tests/lang/http_serve_rust.fung`(`# only: rust`): `http_serve`와 같은 내용을 `agent`로. `http_serve.fung`(`# only: go`)은 그대로.
  `run_parity_skip.txt`의 마지막 줄 삭제(파일과 장치를 지울지는 계획에서).
- Go 대조(`crates/fung-cli/tests/http_parity.rs`):
  - 클라이언트: 같은 로컬 테스트 서버(Rust 테스트 코드의 가짜 서버)에 Go fung과 Rust fung 클라이언트를 붙여 출력 비교 — 상태·헤더·본문,
    조각 전송, 연결 닫힘까지 읽기, `HEAD`·204, 리다이렉트 각 코드·10번 초과·상대 경로·호스트 바뀜, 시간 초과, 연결 거부, 잘못된 URL, 프록시(가짜 프록시 서버, `CONNECT` 포함).
  - 서버: Go fung 서버와 Rust fung 서버에 같은 원시 요청을 보내 응답 비교(`Date` 값은 가림) — 기본 헤더, 413, 400, 500·503, 헤더 10초 한도(시간을 줄일 수 없으면 Rust만 따로).
  - https: 저장소의 테스트 인증서(긴 유효 기간, 생성 스크립트와 함께)로 TLS 서버를 띄우고 `SSL_CERT_FILE`로 신뢰. Go 대조는 macOS·Linux 모두(Go 1.27부터 `SSL_CERT_FILE`·`SSL_CERT_DIR`이 있으면 macOS에서도 운영체제 검증기 대신 그 인증서를 쓴다; 옛 동작은 GODEBUG `x509sslcertoverrideplatform`).
- 취소: `accept`·`read`에서 막힌 태스크를 `cancel`하면 바로 E0701로 끝나는지(상태로 순서를 정하는 탐침).
- 예제 `examples/serve.fung`·`api.fung`·`checkurls.fung`·`urlcheck.fung`·`static.fung`이 Rust로 도는지(필요하면 `atom` → `agent`는 6단계 몫이므로 Rust판 테스트에서만).
- `_native.net` 단위 테스트: 부분 읽기, 긴 줄, `byte_len`, 닫힌 연결, 마지막 사본에서 닫힘.

## 4. 문서

- 학습 노트 `docs/notes/R4d-net-http.md`(한국어): 소켓과 막히는 입출력, TLS와 인증서 신뢰(루트 저장소), HTTP/1.1 메시지 구조(시작 줄·헤더·본문 길이 세 가지), 리다이렉트·프록시·터널,
  fung으로 프로토콜을 쓴 경험(무엇이 편했고 무엇이 모자랐나), 그린 스레드를 미룬 이유, 실제로 부딪힌 함정.
- `docs/design/open-questions.md` "Rust 이전": 4d 완료 한 줄, 허용 목록 표 추가, **"R7 스케줄러" 항목**(그린 스레드·`mio`·태스크 생성 비용·고차 함수 구현 언어·스택 크기),
  과도기 잔가지(`User-Agent`, Go 에러 문구), 알려진 차이, "0.2.0에서 바뀌는 것"에 약속 밖 차이.
- `docs/design/06-*.md` HTTP 절: "fung HTTP 약속"으로 고쳐 쓴다("리다이렉트·https는 Go 기본" 대신 2절 내용).
- `CLAUDE.md` 현재 상태.

## 완료 기준

1. 실행 비교에서 빼는 파일이 없다. Go 대조 전체가 같다.
2. Rust 골든 실행기가 `http_serve_rust`를 포함해 통과한다.
3. `http_parity` 대조가 통과한다(https Go 대조는 Linux).
4. 커밋 전 검사 전체(`--locked`, `cargo-deny`·`cargo-audit` 포함)가 통과한다.
5. 학습 노트가 있다.

## 범위 밖

그린 스레드·`mio`·태스크별 힙(R7), 서버 TLS, HTTP/2, 압축, 연결 재사용, 쿠키 저장, 6단계 일(예제·매뉴얼의 `atom` → `agent`, CHANGELOG).
