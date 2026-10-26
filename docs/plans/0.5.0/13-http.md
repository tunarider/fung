# L13. HTTP 클라이언트·서버의 세부 한계

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

출처: open-questions "알려진 한계"의 HTTP 절, 06 "HTTP"("fung HTTP 약속"), `docs/notes/R4d-net-http.md`.
코드는 모두 `extras/http.fung`(클라이언트 `_request`·`_send`·`_exchange`·`_open`·`_tunnel`·`_parse_url`, 서버 `serve`·`_conn`·`_reply`·`_own_answer`)이고,
그 밑의 소켓·TLS 원시 기능이 `crates/fung-vm/src/native/net.rs`·`sock.rs`·`tls.rs`다. 시험은 `crates/fung-cli/tests/http.rs`와 기대 출력 `crates/fung-cli/tests/expected/http/*.txt`.
아래 재현은 2026-10-26 릴리스 바이너리로 127.0.0.1에서만 했다(서버는 `http.serve`, 날 요청은 `nc`).

## 0. 나눔: 문제와 기능

open-questions의 목록에는 틀린 동작과 없는 기능이 섞여 있다. README 3절에 따라 **기능은 0.5.0 밖**이다.

- **0.5.0 밖(기능)**: 연결 재사용(keep-alive)·파이프라이닝(그래서 서버가 HTTP/1.1에 늘 `Connection: close`, HTTP/1.0 keep-alive에 닫음으로 답함),
  HTTP/2, 압축, SOCKS 프록시, https 프록시 너머의 https(TLS 안의 TLS), 101 뒤 연결 넘겨주기(업그레이드·웹소켓), 본문 흘려 받기·보내기,
  IDNA(퓨니코드 변환) 자체, 서버의 `CONNECT` 2xx 답, 요청 컨텍스트를 취소하는 배경 읽기.
  지금 동작은 모두 RFC와 맞는 "안 함"이다. 예: HTTP/1.0 요청에 `Connection` 없이 답하고 닫는 것은 RFC 9112 9.3과 맞다(재현에서 확인).
- **문제**: 아래 H1~H12. 조용히 틀린 바이트를 보내거나, 조용히 값을 버리거나, 답을 읽을 수 있는데 에러를 내는 것.
- **고칠 것 없음(기록만 닫기)**: H13~H15.

## 1. 서버의 문제

### H1. 핸들러가 넣은 틀 헤더가 본문과 어긋난다
```
handler: {headers: {"Content-Length": "3"}, body: "hello"}   → Content-Length: 3 … 본문 hello (5바이트)
handler: {headers: {"Transfer-Encoding": "chunked"}, body: "hello"}
  → Transfer-Encoding: chunked 와 Content-Length: 5 를 함께 보내고 본문은 청크가 아님
```
원인: `_reply`가 `Content-Length`가 있으면 자동 값만 끄고 본문은 다 쓰며, `Transfer-Encoding`은 막지 않는다. 앞의 것은 같은 연결의 다음 바이트를 다음 응답으로 읽히게 하고(재사용이 생기면 응답 밀반입),
뒤의 것은 RFC 9112 6.1("둘을 함께 보내면 안 된다") 위반이다. 틀은 서버의 일이다(Go `net/http`는 선언한 길이를 넘는 쓰기를 `ErrContentLength`로 거절).
선택지: (가) `Transfer-Encoding`·`Connection`·`Trailer`는 핸들러가 넣으면 500 + `http: bad handler response: …`, `Content-Length`는 본문 바이트 수와 같거나 본문이 비어야 하는 답(`HEAD`·304)일 때만 받는다.
(나) 핸들러의 틀 헤더를 조용히 버리고 늘 서버 값을 쓴다(`HEAD`에 GET 길이를 알리는 쓰임을 잃음). 추천: (가). 조용히 버리지 않고, 정당한 쓰임(`HEAD`의 길이)은 남는다.

### H2. 이름이 대소문자만 다른 응답 헤더는 하나만 남는다
`{"X-A": "1", "x-a": "2"}` → `X-A: 2`만 간다(`1`은 말없이 사라짐). 선택지: (가) 500 + `bad handler response`(중복 이름), (나) `", "`로 잇기(`Set-Cookie`에서 틀림), (다) 지금처럼 뒤의 것.
추천: (가). 맵 리터럴의 중복 키처럼 실수이고, 잇기는 헤더마다 의미가 다르다.

### H3. 핸들러의 101은 반쪽이다
`{status: 101, headers: {Upgrade: "websocket", Connection: "Upgrade"}}` → `101 Switching Protocols`에 `Connection: close`를 붙이고 닫는다(핸들러의 `Connection`은 지움).
업그레이드는 기능이라 0.5.0 밖이고, 그 전까지 101을 보내는 것은 거짓말이다. 추천: 핸들러의 101은 500 + `bad handler response: status 101 is not supported`. 06의 상태 표에서 101을 뺀다.

### H4. 서버의 자기 답(400·431·501·505)에 `Content-Length`가 없고, 431 뒤 반쯤 닫기가 없다
`_own_answer`는 `Connection: close`만 쓰므로 클라이언트는 닫힘까지 읽어야 끝을 안다. 431 뒤에는 쓰기 쪽을 닫지 않은 채 500 ms 읽어 버리므로(`_linger`) 그동안 클라이언트는 끝을 못 본다.
고치는 법: 자기 답에 `Content-Length`와 `Date`를 붙이고(`_reply`와 같은 길), 431 뒤에는 쓰기 쪽만 닫은 뒤(`shutdown(SHUT_WR)`) 읽어 버린다.
`_native.net`에 반쯤 닫기가 없으므로(`net.rs`에 `shutdown` 없음, 확인함) 내부 원시 기능 `_net.close_write(c)`를 더한다(TLS면 close_notify 뒤 반쯤 닫기; 12 문서의 close_notify와 맞춘다).

## 2. 클라이언트의 문제

### H5. 서버가 일찍 답하고 닫으면, 답을 읽을 수 있어도 쓰기 에러다
답(413)을 쓰고 읽지 않은 채 닫는 서버에 64 MiB를 `post`하면:
```
Err({kind: 'network, message: "cannot request `http://127.0.0.1:18433/`: connection reset by peer"})
```
(fung 서버는 413 전에 버퍼를 읽어 버려서 같은 요청이 `Ok({status: 413, …})`다.) 원인: `_exchange`가 `_net.write`의 `Err`를 그대로 돌려준다.
RFC 9112 9.6은 서버가 요청 도중 답하고 닫을 수 있다고 하고, curl은 그 답을 보여 준다. 고치는 법: 쓰기 에러면 남은 기한 안에서 답을 한 번 읽어 보고, 상태 줄과 머리가 다 오면 그 답을 `Ok`로, 아니면 원래 쓰기 에러를 돌려준다.

### H6. 알 수 없는 스킴의 프록시가 포트 80으로 연결된다
`HTTP_PROXY=foo://127.0.0.1` → ``proxy `127.0.0.1`: connection refused``(80번에 연결). `_open`은 `socks5`·`socks5h`만 거절하고 나머지는 http 프록시로 친다.
추천: `http`·`https` 밖의 스킴은 연결 전에 ``unsupported proxy scheme `foo` ``(`Err 'network`). curl도 모르는 스킴을 거절한다.

### H7. URL의 호스트: 비ASCII를 그대로 보내고, `%XX`를 풀지 않고, 괄호 IPv6를 검사하지 않는다
로컬 리스너를 프록시로 두고(이름 조회 없음) 보낸 요청 머리:
```
http://例え.jp/      → GET http://例え.jp/ HTTP/1.1  /  Host: 例え.jp        (UTF-8 바이트 그대로)
http://ex%C3%A9.com/ → Host: ex%C3%A9.com                                    (이스케이프 그대로)
http://EX%41mple.com/ → invalid URL: invalid escape `%41`                    (ASCII 이스케이프는 거절)
http://[1:2]:1/      → no such host   (잘못된 IPv6가 이름 조회로 감; `[zz]`는 invalid IP address)
```
`Host`는 ASCII여야 하고(RFC 9110 7.2, DNS 이름은 A-label), 직접 연결이면 비ASCII 이름이 `getaddrinfo`로 가서 운영체제마다 다르게 실패한다.
선택지: (가) 호스트의 `%XX`를 RFC 3986 3.2.2대로 푼 뒤, 비ASCII가 남으면 ``invalid URL: non-ASCII host `例え.jp` (write it in punycode, `xn--…`)``로 거절한다.
(나) IDNA(UTS #46 매핑 + 퓨니코드)를 구현한다(유니코드 표가 더 필요, 크레이트는 허용 목록 밖). 추천: (가). IDNA는 기능이라 0.5.0 밖이고, (가)는 틀린 바이트를 보내지 않는다.
`NO_PROXY`와 리다이렉트의 같은 사이트 판정은 같은 정규화를 거치게 한다. 괄호 안은 이미 있는 `_parse_addr`(`NO_PROXY`용 IPv6 해석)로 검사해 ``invalid IP address `1:2` ``로 거절한다.

### H8. 에러 URL이 비밀번호를 보이고, 조각을 이스케이프하지 않는다 (이 문서를 쓰며 찾음)

비밀번호를 가리는 부분은 작은 문제 S19로 옮겨 2026-10-26 세션에서 고친다([README](README.md)). 여기에는 조각·비ASCII 표시만 남는다.
```
http.get("http://u:secret@127.0.0.1:1/") → "cannot request `http://u:secret@127.0.0.1:1/`: connection refused"
```
`_failed(url, e)`가 사용자가 준 글을 그대로 쓴다. 리다이렉트 뒤의 문구(``redirected to `…` ``)만 `_url_string`으로 비밀번호를 `***`로 가린다. 에러는 로그에 남으므로 새는 것이다(Go도 가린다).
고치는 법: 해석에 성공한 URL은 언제나 `_url_string(u)`으로 보이고, `_url_tail`의 조각도 경로처럼 다시 이스케이프한다(지금 `#a b`·DEL이 그대로).
사용자 정보의 비ASCII(`%C3%A9`)는 유효한 UTF-8이면 풀어 보인다(`_unescape_ascii`를 UTF-8 열까지). 해석에 실패한 URL은 글 그대로 두되 `user:pass@` 꼴이면 비밀번호를 가린다.

### H9. `CONNECT` 답
`_tunnel`은 `200`만 터널로 받는다. RFC 9110 9.3.6은 모든 2xx가 터널이고 2xx 답의 `Content-Length`·`Transfer-Encoding`은 무시한다. 추천: 2xx를 모두 받는다.
"프록시 요청의 `Host` 헤더 검사가 없다"는 open-questions 문구가 클라이언트 쪽인지(절대 형식 요청에서 사용자 `Host`가 URL과 다름) 서버 쪽인지(RFC 9112 3.2.2: 절대 형식이면 `Host` 대신 대상의 authority) **확인하지 못했다**. 세션이 먼저 R4d 노트와 시험을 보고 정한다.

### H10. 응답 본문 상한 64 MiB를 바꿀 수 없다
`_body_cap = 67108864`(`extras/http.fung`)를 넘으면 `Err 'network`. 06은 "더 큰 응답은 나중에 요청별 옵션"이라 적었다. 더 큰 파일을 받을 길이 아예 없으므로 문제로 친다(흘려 받기는 기능, 0.5.0 밖).
추천: `get`·`post`·`request`에 `max_body: int = 67108864`(바이트). "제한 없음" 값은 두지 않는다(메모리를 끝없이 쓰지 않는다는 06 약속).

### H11. 인자 에러에 내부 프레임이 보인다
`http.serve(…, ready: 1)`의 경로에 `extras/http.fung:55 _ready_check`와 `serve` 두 프레임. 06은 extras 프레임을 보이게 두므로 `serve` 하나는 맞고, `_ready_check`는 군더더기다.
고치는 법: `ready: fn | nil = nil`로 매개변수 타입을 바로 쓴다(언어가 `fn | nil`을 받음, 확인함) — `_ready_check`를 지운다. 꼬리 호출이 흐리는 위치는 L8 문서.

### H12. 헤더를 fung으로 해석하는 비용 (서버 쪽은 서비스 거부)
open-questions의 수치: 접히거나 끝없는 머리 40만 줄(10 MiB 예산)에 클라이언트 약 1초, 꽉 찬 요청 머리(1 MiB, 값 500개) 하나에 서버 요청마다 약 0.1~0.25초, 1바이트 청크 10만 개에 약 0.6초.
실행 스레드가 하나라, 서버에서는 꽉 찬 머리를 보내는 클라이언트 몇이 모든 요청을 늦춘다. 선택지: (가) 머리 읽기(줄 나누기·접기·이름 검사·한도)를 내부 원시 기능 `_net.read_head(c, limit, max_lines)`로 옮기고 의미 검사(`Host`·`Content-Length`·TE)는 fung에 남긴다.
(나) 한도를 낮춘다(1 MiB → 64 KiB, nginx 기본 8 KiB 줄·Go 1 MiB). (다) 그대로. 추천: (가) + 측정. 한도는 그 뒤 다시 본다. 06의 "fung으로 쓴 HTTP" 원칙과 부딪히므로 사용자에게 묻는다.

## 3. 고칠 것 없음 (기록만 닫기)

- **H13** `PRI * HTTP/2.0`: 지금 `505 HTTP Version Not Supported`로 답하고 닫는다(재현함). HTTP/2를 모르는 HTTP/1.1 서버의 올바른 답이다. 시험으로 못박고 한계 목록에서 지운다.
- **H14** `NO_PROXY`의 ASCII 공백만 다듬기: 환경 변수에 유니코드 공백이 들어가는 쓰임이 없다. curl도 ASCII 공백만 본다고 알고 있으나 **확인하지 않았다**(세션이 확인).
- **H15** macOS의 받기 대기 줄(`kern.ipc.somaxconn` 128): 운영체제 설정이고 fung은 가장 긴 줄을 요청한다. 매뉴얼 `reference/modules/http.md`에 한 줄로 옮기고 닫는다.

## 정할 것

1. H1: 핸들러의 틀 헤더를 500으로 거절하고 맞는 `Content-Length`만 받는다. 추천: 예.
2. H2: 대소문자만 다른 응답 헤더 이름은 500. 추천: 예.
3. H3: 핸들러의 101은 업그레이드를 만들 때까지 500. 추천: 예.
4. H7: 비ASCII 호스트는 퓨니코드로 쓰라는 에러로 거절(IDNA는 0.5.0 밖). 추천: 예.
5. H10: 요청별 `max_body`(바이트 수, 기본 64 MiB, 무제한 없음). 추천: 예. 이름(`max_body`)도 확인.
6. H12: 머리 읽기를 네이티브로 옮긴다(측정 먼저). 추천: 예, 단 06 원칙과의 관계를 사용자에게 설명하고 묻는다.
7. H5: 쓰기 에러 뒤 답 읽기에 따로 짧은 한도를 둘지(예: 남은 기한과 1초 중 짧은 쪽). 추천: 남은 기한 그대로(요청 하나에 기한 하나라는 06 약속).

## 할 일

1. 시험 먼저(TDD): `crates/fung-cli/tests/http.rs`에 H1~H8·H10·H11·H13의 실패 사례. 서버는 날 소켓으로 응답 바이트를 비교하고, 클라이언트는 테스트 안의 가짜 서버(일찍 답하고 닫기, 프록시 머리 받기)로.
   자식 프로세스는 `common::Reaped`로 쥔다. 기대 출력 `tests/expected/http/{server,client,proxies,tunnel}.txt`를 늘리고 사례 수 상수를 고친다.
2. 서버: `_reply`의 틀 헤더 검사(H1)·중복 이름(H2)·101(H3), `_own_answer`의 `Content-Length`·`Date`(H4), `_native.net`의 `close_write`와 `_refuse`의 반쯤 닫기(H4, `net.rs`·`sock.rs`, 단위 시험 `net_tests.rs`).
3. 클라이언트: `_exchange`의 쓰기 에러 뒤 읽기(H5), `_open`의 스킴 검사(H6), `_parse_url`의 호스트 정규화와 IPv6 검사(H7), `_failed`와 `_url_tail`(H8), `_tunnel`의 2xx(H9), `max_body`(H10), `ready` 타입(H11).
4. H12는 먼저 `bench/`에 머리 40만 줄·꽉 찬 요청 머리의 측정을 두고 전/후를 잰다(`scripts/bench.sh <전> <후>`). 원시 기능은 `_native.net`에만 두고 공개하지 않는다(06 48줄).
5. 문서: 06 "HTTP"(틀 헤더·101·중복·`max_body`·호스트 규칙·에러 URL), open-questions "알려진 한계"의 HTTP 절에서 고친 것을 지우고 0절의 기능은 "0.5.0 뒤"로 옮김, R4d 노트에 덧붙임.
6. 매뉴얼: `manual/reference/modules/http.md`(매개변수 표가 `manual.rs`와 맞아야 함), 필요하면 `manual/learn/13-json-and-http.md`. `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build` 뒤 diff 검토.
7. `CHANGELOG.md` `[Unreleased]`: 호환 깨짐(H1·H2·H3의 500, H7의 거절, H6의 거절), 고침(H4·H5·H8·H9), 더함(H10 `max_body`).
8. 커밋 전 검사 전부(CLAUDE.md), `spec/builtins.txt`가 바뀌면 `FUNG_UPDATE_SPEC=1 cargo test spec_file`.

## 끝의 기준

- 0절 "문제"의 H1~H12가 시험으로 고정되어 통과하고, 위 재현 명령이 새 동작(거절 문구 또는 올바른 바이트)을 낸다.
- H12는 전/후 수치가 R4d 노트(또는 `bench-languages.md`)에 있고, 결정대로 했거나 사용자가 미루기로 한 기록이 있다.
- open-questions의 HTTP 절에는 0.5.0 밖의 기능만 "0.5.0 뒤"로 남고, 06·매뉴얼·CHANGELOG가 같은 말을 한다.
- 커밋 전 검사가 Linux·macOS CI에서 통과한다.
