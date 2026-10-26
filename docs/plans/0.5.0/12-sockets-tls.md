# L12. 소켓·TLS의 세부 한계

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

출처: open-questions "알려진 한계"의 "소켓·TLS", 06 "HTTP"의 https, `docs/notes/R4d-net-http.md`(2·3절; 쓰레드 시절의 설명이라 기다림 부분은 R4a 이벤트 루프로 바뀌었다).
코드: `crates/fung-vm/src/native/net.rs`(`_native.net`), `sock.rs`(소켓·`getaddrinfo`), `tls.rs`(rustls), `vm/block.rs`(`park_io`·`IoWait`), `vm/pool.rs`(`POOL_MAX` 64), `extras/http.fung`.

## 먼저 알아 둘 것: 사용자에게 닿는 길

`_native.net`은 사용자가 가져올 수 없다(E0310). 소켓을 쓰는 공개 모듈은 `http` 하나다. 그래서 아래 항목은 둘로 갈린다.
- **지금 사용자에게 보이는 것**: 주소 계열 순서(B), 이름 조회가 풀 스레드를 붙잡음(D), TLS 세부(E~H), `http.serve(":http")`.
- **지금은 `http.fung` 안에서만 보이는 것**: `set_deadline`(A), 연결의 차례 줄(C). 고칠 값어치는 원시 기능을 바르게 두는 데 있다.

곁에서 본 것(이 문서 범위 밖, 작은 문제): `import _native.net`의 E0310 문구가 ``import `net` instead``인데 `net` 모듈은 없다(`import net` → E0301). README 1절에 더할지 사용자에게 묻는다.

## A. `set_deadline`이 막힌 기다림에 닿지 않음

**문제·원인.** 읽기·쓰기가 막히면 `wait_io(a, fd, interest, deadline)`이 그때의 기한을 `IoStep::Wait`에 담고(`net.rs` 263행), `IoWait::after`가 그것을 타이머(`Wake { until, timed: true }`)로 건다(`block.rs` 435행).
`set_deadline`(`net.rs` 748행)은 `Conn.deadline`만 바꾸고 기다리는 태스크를 깨우지 않는다. 깨어나면 `fill`·`write`가 기한을 새로 읽으므로 **늘리기·없애기는 맞게 된다**(옛 타이머에 한 번 깨어 다시 기다림).
**줄이기와 없던 기한을 새로 걸기만** 닿지 않는다. 지금 `set_deadline`을 부르는 곳(`http.fung` 1513·1535·1721행)은 모두 같은 태스크가 차례로 부르므로 재현되는 사용자 프로그램은 없다.

**선택지.** (가) 그대로 두고 06에 규칙으로 적는다. 원시 기능의 함정이 남는다.
(나) `set_deadline`이 그 연결의 기다림을 깨운다(`drop_io_waits`처럼 깨우되 등록 해제와 닫음 없이). 깨어난 쪽은 처음부터 다시 시도해 새 기한을 읽는다. 함수가 `Task`를 받게 바꾼다.
추천: (나). 몇 줄이고, Go `SetDeadline`("이미 막힌 호출에도 적용")과 같은 뜻이 된다.

## B. 주소 계열을 경주시키지 않음 (Happy Eyeballs 없음)

**문제.** `Dial::step`(`net.rs` 1004행)은 `getaddrinfo`가 준 순서대로 주소를 하나씩 시도한다. 앞 주소가 응답 없이 버려지면(IPv6 경로가 막힌 네트워크에서 흔함)
운영체제의 연결 시간 한도(macOS 약 75초, Linux 약 2분)까지 기다린 뒤에야 다음 주소로 간다. `http.get`의 기본 한도는 30초라 **IPv4로 가 보지도 못하고 시간 초과**다.
실패를 모두 겪으면 첫 에러만 보고한다(`self.first`). 이 재현은 막힌 IPv6 경로가 필요해 이 기계에서 하지 못했다. 코드로만 확인했다.

**고려할 점.** RFC 8305(Happy Eyeballs v2): 주소를 계열이 번갈게 섞고, 앞 시도가 끝나지 않았어도 250ms 뒤 다음을 시작해 먼저 된 것을 쓰고 나머지를 닫는다.
Go(`net.Dialer`의 `FallbackDelay` 300ms), Python asyncio(`happy_eyeballs_delay`, 기본 끔), curl(200ms)이 이렇게 한다. 지금의 `IoStep::Wait`는 fd 하나만 담는다.
`IoRegs`(`block.rs` 337행)는 여러 등록을 이미 담을 수 있으므로, 여러 fd를 기다리는 단계(`IoStep::WaitAny` 같은 것)를 더하면 된다.

**선택지.** (가) 그대로, 06에 적는다. (나) 주소마다 기한을 나눔(남은 시간 / 남은 주소 수). 단순하지만 첫 주소가 느린 정상 서버면 손해다.
(다) RFC 8305의 연결 부분: 계열을 번갈게 정렬, 250ms 간격으로 다음 시도 시작, 먼저 된 연결 채택, 나머지 닫음. 모두 실패하면 첫 에러.
추천: (다). 스크립트가 도는 곳(컨테이너, 사내망)에서 IPv6 블랙홀은 실제로 겪는 일이고, 이벤트 루프 위라 스레드가 늘지 않는다. 조회 단계의 경주(AAAA·A 따로 묻기)는 `getaddrinfo`를 쓰는 한 할 수 없으므로 하지 않는다.

## C. 한 연결에 읽는 쪽 하나, 쓰는 쪽 하나

**지금.** 연결마다 `Turnstile` 두 개(`rd_turn`·`wr_turn`, `net.rs` 270~298행). 두 번째 읽는 쪽은 줄에서 주차하고 취소·기한에 끊긴다(`a_second_reader_waits_for_the_first`, `a_second_reader_can_be_cancelled_while_it_waits`).
`release`는 기다리는 쪽을 모두 깨워 다시 다투게 한다(공정성은 L14). HTTP는 연결당 읽는 쪽·쓰는 쪽이 하나라 사용자에게 보이지 않는다.

**선택지.** (가) 한계가 아니라 규칙으로 둔다: 06에 "한 연결의 읽기(쓰기)는 한 번에 하나, 나머지는 차례를 기다린다"를 적고 open-questions에서 뺀다.
(나) 읽기를 여럿 섞어 받게 한다. 줄 단위 읽기가 서로의 줄을 깨므로 의미가 없다.
추천: (가). 줄을 섞지 않는 것이 맞는 동작이다. 깨움을 하나만(줄 맨 앞) 하는 것은 L14에서 다른 차례 줄과 함께 정한다.

## D. `getaddrinfo`를 끊을 수 없어 풀 스레드를 붙잡음

**문제.** 이름 조회는 `offload_kind(Kind::Abandon)`·`offload_abandoning`으로 풀에서 돈다(`connect_fn` 1136행·`listen_fn` 1181행). 취소·시간 초과된 `connect`는 바로 돌아오지만
스레드는 해석기가 답할 때까지(해석기 한도, 보통 5초 × 시도 횟수) 남는다. 풀은 파일 일과 같은 하나이고 최대 64개(`pool.rs` 28행)라, 응답 없는 DNS로 몰린 조회 64개면 **파일 일도 줄에서 기다린다**.

**고려할 점.** glibc의 `getaddrinfo_a`는 musl·macOS에 없다. macOS에는 `getaddrinfo_async_start`(비공개에 가까움)가 있다. 직접 DNS 해석기(UDP, `/etc/resolv.conf`)를 쓰면 끊을 수 있지만
`/etc/hosts`·nsswitch·mDNS·macOS의 망별 해석기 설정을 잃는다. Go는 순수 Go 해석기와 cgo 해석기를 고르고, Rust의 tokio는 `getaddrinfo`를 blocking 풀에 넘긴다(fung과 같음).

**선택지.** (가) 그대로. (나) 이름 조회에 따로 상한(예: 동시 8개)을 둔 줄을 두어 파일 일을 굶기지 않게 한다. 같은 이름의 동시 조회는 하나로 합친다.
(다) 직접 쓴 DNS 해석기. 크고, 시스템 설정과 다르게 답할 수 있다.
추천: (나). 끊을 수 없다는 사실은 남지만 해가 이름 조회 안에 갇힌다. `LOOKUPS` 셈(`net.rs` 195행)이 이미 있다.

## E. 이름 붙은 포트(`:http`)

**재현.**
```
$ fung -e 'import http
print(http.serve(":http", fn(r): {status: 200} ;;))'
Err({kind: 'network, message: "cannot listen on `:http`: invalid port `http`"})
$ fung -e 'import http
print(http.get("http://localhost:http/"))'
Err({kind: 'network, message: "cannot request `http://localhost:http/`: invalid URL: invalid port `http`"})
```
**고려할 점.** URL의 포트는 RFC 3986 3.2.3상 숫자뿐이라 둘째는 **맞는 거절**이다(Go `url.Parse`도 거절). 남는 것은 `serve`의 주소다. Go `net.Listen`은 `/etc/services`로 이름을 푼다.
그 파일은 `scratch`·distroless 같은 작은 컨테이너에 없어서, 같은 스크립트가 호스트에 따라 달라진다. 풀려면 `getservbyname`(막힘, 스레드 안전하지 않음) 또는 파일 직접 읽기가 필요하다.

**선택지.** (가) 풀지 않고 06에 규칙으로 적는다(포트는 십진수). 문구를 ``invalid port `http` (a port is a number)``처럼 분명하게.
(나) `/etc/services`를 읽는다.
추천: (가). 호스트마다 달라지는 동작을 들이지 않는다. 한계 목록에서 빼고 06의 `serve` 주소 설명에 적는다.

## F. TLS: close_notify를 기다리지 않음, 버린 핸들은 보내지 않음

**지금.** `Conn::close`(`net.rs` 737행)가 `tls::close_notify`(`tls.rs` 330행)를 부른다. 소켓이 지금 받는 만큼만 쓰고(`flush_quietly`) 곧 `shutdown`한다. 보내기 버퍼가 차 있으면 경고가 사라진다.
마지막 사본이 사라질 때(`Drop`)는 보내지 않는다. 받는 쪽은 레코드 사이의 TCP 끝을 끝으로 받아들이는 경우가 많아 대개 해가 없지만, 엄격한 상대에게는 잘림 공격과 구별되지 않는다.

**고려할 점.** RFC 8446 6.1은 보내는 쪽이 상대의 close_notify를 기다릴 필요는 없다고 한다. Go `tls.Conn.Close`는 close_notify 쓰기에 5초 기한을 건다. `_native.net.close`는 지금 바로 돌아오는 네이티브라 기다리려면 `run_io` 연산이 되어야 한다.

**선택지.** (가) 그대로, 적어 둔다. (나) `close`가 close_notify를 다 쓸 때까지 주차하되 짧은 상한(예: 1초)과 바깥 기한·취소를 따른다. `Drop`은 지금처럼 보내지 않는다(실행 스레드에서 기다릴 수 없음, L11의 배경 닫기와 같은 문제).
추천: (나). 상한 값은 정할 것 2.

## G. TLS: ALPN을 보내지 않음

**지금.** `client_config`(`tls.rs` 358행)에 `alpn_protocols`가 비어 있다. 서버는 HTTP/1.1로 말하니 동작은 맞다.
**고려할 점.** ALPN `http/1.1`을 보내면 서버가 프로토콜을 확정할 수 있고, 다른 프로토콜 서버에 같은 인증서로 붙는 교차 프로토콜 공격(ALPACA, 2021)의 방어가 된다. 브라우저·curl·Go `net/http`는 보낸다.
`_native.net`은 HTTP 전용이 아니므로 목록을 인자로 받는 것이 맞다.
**선택지.** (가) 그대로. (나) `connect(…, tls, alpn = [])`·`start_tls(c, host, alpn)`에 목록을 받고, `http.fung`은 `["http/1.1"]`을 준다. 서버가 목록 밖의 것을 고르면 rustls가 에러를 낸다(문구는 `tls::reason`에 하나 더).
추천: (나). 설정 하나와 인자 하나다. 연결마다 다른 `ClientConfig`가 필요해지므로 `OnceLock`의 설정을 ALPN별로 둘(또는 `Arc` 복제) 방법을 구현 때 정한다.

## H. `SSL_CERT_FILE`·`SSL_CERT_DIR`가 시스템 저장소를 대신함

**지금.** `rustls_native_certs::load_native_certs()`(`tls.rs` 350행): 변수가 하나라도 있으면 변수의 위치만 읽는다. OpenSSL(curl·Python)은 변수마다 자기 반쪽만 바꾼다(`SSL_CERT_FILE`은 기본 파일 대신, 기본 디렉터리는 그대로).
Linux의 Go도 OpenSSL 쪽이다(`docs/notes/R-port-record.md` 366행). macOS의 Go 1.27은 변수가 있으면 그 파일만 쓴다.
**선택지.** (가) 지금 뜻("변수가 있으면 그것만")을 06·매뉴얼에 규칙으로 적는다. 두 운영체제에서 같다. (나) OpenSSL 뜻을 직접 구현한다(파일·디렉터리를 우리가 읽음, macOS는 키체인 + 변수의 것?). 운영체제마다 뜻이 갈린다.
추천: (가). "이 묶음만 믿어라"가 변수를 쓰는 흔한 뜻(사내 CA 묶음)이고 운영체제마다 같다. 대신 S14의 시험(깨진 PEM, `SSL_CERT_DIR`만)을 함께 넣는다.

## I. CA 파일이 없는 곳(`scratch`)에서 https 실패

**재현**(로컬 `openssl s_server`, `crates/fung-cli/tests/data/tls/localhost.pem`, 127.0.0.1:18443):
```
SSL_CERT_FILE=ca.pem           → 200
SSL_CERT_FILE=/nonexistent     → Err({kind: 'network, message: "cannot request `https://localhost:18443/`: unknown certificate authority"})
SSL_CERT_DIR=/nonexistent      → (같은 에러)
변수 없음(키체인에 그 CA 없음) → (같은 에러)
```
루트가 하나도 없는 것과 믿지 않는 CA가 **같은 문구**다. `scratch`에서 사용자는 원인을 알 수 없다.
**고려할 점.** 루트를 실행 파일에 넣으면(`webpki-roots`, Mozilla 목록) 어디서나 되지만, 목록이 낡고(폐기된 CA를 릴리스 전까지 믿음) 릴리스마다 갱신해야 하며 `deny.toml`의 허용 목록·라이선스(CDLA-Permissive-2.0, 확인 필요)·2주 규칙이 따른다.
Go는 기본으로 넣지 않고 따로 고르는 `x509roots/fallback` 꾸러미를 둔다. Python·curl도 시스템 것을 쓴다.
**선택지.** (가) 넣지 않고, 읽은 루트가 0개면 다른 문구: ``no root certificates found (system store empty; set SSL_CERT_FILE)``. 매뉴얼에 컨테이너 안내(`ca-certificates` 파일 복사 또는 변수).
(나) `webpki-roots`를 시스템 저장소가 비었을 때만 쓴다. (다) 언제나 넣은 것을 쓴다.
추천: (가). 신뢰 목록의 주인을 운영체제(또는 사용자)로 둔다. `load_native_certs`의 `errors`도 문구에 짧게 붙일지는 정할 것 4.

## 정할 것

1. 위 항목별 추천(A 나, B 다, C 가, D 나, E 가, F 나, G 나, H 가, I 가)을 그대로 갈지. 추천: 그대로.
2. F의 close_notify 쓰기 상한. 추천: 1초(스크립트 끝이 오래 늘지 않게; Go는 5초).
3. B의 시도 간격. 추천: 250ms(RFC 8305 권장값), 상수로 두고 인자로는 열지 않는다.
4. I의 문구에 읽기 실패(`errors`, 예: 읽을 수 없는 `SSL_CERT_FILE`)를 붙일지. 추천: 변수가 가리키는 곳을 못 읽었으면 붙인다(``cannot read SSL_CERT_FILE `/x`: no such file``), 시스템 저장소가 빈 것은 위 문구.
5. D의 조회 상한 값. 추천: 8(풀 64의 1/8), 같은 이름 합치기 포함.
6. E0310의 `import net` 문구(곁에서 본 것)를 README 1절에 더할지. 추천: 더한다.

## 할 일

시험을 먼저 쓰고 실패를 확인한 뒤 고친다(TDD). 단위 시험은 `net_tests.rs`·`tls_tests.rs`, 사용자 동작은 `crates/fung-cli/tests/` 기대 출력.
1. A: 막힌 읽기 옆에서 다른 태스크가 기한을 줄이면 새 기한에 끝나는 시험(상태로 순서, `testutil` 탐침) → `set_deadline`이 깨움.
2. B: 응답 없는 첫 주소를 흉내(듣기만 하고 `accept`하지 않는 리스너는 SYN을 받으므로 안 됨; 시험용 `Target` 목록에 닿지 않는 주소를 앞에 두는 방법을 찾는다, 못 찾으면 `Dial` 단위 시험) →
   여러 fd를 기다리는 `IoStep`과 `Dial`의 경주. 진 연결을 닫고 열린 소켓 수가 0으로 돌아오는지 본다(R4d의 "끝난 뒤 0" 검사).
3. C: 06 `fs`/소켓 절에 규칙 문장, open-questions 한계에서 뺌.
4. D: 응답 없는 조회(`gate.invalid` 탐침) 9개 + 파일 읽기가 줄에 서지 않는 시험 → 조회 줄.
5. E: 문구 시험 → 문구, 06의 `serve` 주소 설명.
6. F: 상대가 읽지 않아 버퍼가 찬 연결의 `close` 뒤 상대가 close_notify를 받는 시험(`tls_tests.rs`의 `close_notify` 도우미) → `close`를 `run_io` 연산으로.
7. G: 시험 서버가 받은 ALPN이 `http/1.1`인지, `h2`만 받는 서버에서의 에러 문구 → 인자·설정, `spec/builtins.txt`는 `_native`이 들어 있으면 갱신.
8. H·I: `SSL_CERT_FILE`의 깨진 PEM, `SSL_CERT_DIR`만 있는 경우, 루트 0개의 문구 시험 → 문구.
9. 문서: 06 "HTTP"의 https·`serve`, open-questions "소켓·TLS" 한계를 정리(고친 것은 빼고, 규칙이 된 것은 06으로), 매뉴얼의 `http` 페이지(컨테이너 안내), `FUNG_UPDATE_MANUAL=1`로 다시 만들고 diff 검토.
10. CHANGELOG `[Unreleased]`: 주소 경주, ALPN, close_notify 대기, 루트 없음 문구, 포트 문구.

## 끝의 기준

- 위 시험이 모두 있고 고치기 전 실패를 확인했다. 커밋 전 검사(fmt·clippy·`cargo test` 두 프로필·deny·audit)가 통과한다.
- open-questions "알려진 한계"의 "소켓·TLS" 항목이 모두 고쳐졌거나 06의 규칙으로 옮겨졌다. 남는 한계(이름 조회를 끊을 수 없음)는 왜 남는지 한 줄로 적혀 있다.
- 매뉴얼과 CHANGELOG가 바뀐 동작을 말한다.
