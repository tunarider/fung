---
lede: <code>import http</code>. HTTP/1.1 클라이언트(<code>get</code>, <code>post</code>, <code>request</code>)와 작은 서버(<code>serve</code>). 소켓·TLS 원시 기능 위에 fung으로 썼다. 응답을 받았으면 상태 코드와 무관하게 <code>Ok</code>다(404도 <code>Ok</code>).
description: fung 모듈 http — HTTP 요청 보내기(get, post, request)와 HTTP 서버(serve), 그리고 지키는 약속.
---
# http — HTTP

## HTTP란 {#concepts}

**HTTP**(HyperText Transfer Protocol)는 웹 브라우저와 웹 서버, 그리고 대부분의 API가 쓰는 약속이다. 늘 한쪽이 **요청**(request)을 보내고 다른 쪽이 **응답**(response)을 돌려준다. 요청을 보내는 쪽이 **클라이언트**(client), 받아서 답하는 쪽이 **서버**(server)다. `curl https://example.com`은 클라이언트이고, nginx는 서버다.

요청은 네 부분이다.

- **메서드**(method): 무엇을 하려는지. `GET`(가져오기), `POST`(보내서 만들기), `PUT`(바꾸기), `DELETE`(지우기) 등.
- **URL**: 어디에. `https://api.example.com:8443/v1/hosts?env=prod`는 스킴(`https`), 호스트(`api.example.com`), 포트(`8443`, 생략하면 http는 80, https는 443), 경로(`/v1/hosts`), 쿼리(`env=prod`)로 이루어진다.
- **헤더**(header): 덧붙이는 정보. `Content-Type: application/json`(본문의 종류), `Authorization: Bearer …`(인증) 같은 이름-값 쌍이다. 이름은 대소문자를 가리지 않는다.
- **본문**(body): 보내는 데이터. `GET`에는 보통 없다.

응답은 **상태 코드**(status code), 헤더, 본문이다. 상태 코드는 세 자리 숫자로, 첫 자리가 종류를 말한다: 2xx 성공(`200 OK`, `201 Created`), 3xx 다른 곳으로 가라(리다이렉트), 4xx 요청이 잘못됨(`404 Not Found`, `401 Unauthorized`), 5xx 서버 쪽 실패(`500`, `503`).

`https`는 같은 HTTP를 **TLS**로 암호화한 연결 위에서 주고받는다. 클라이언트는 서버의 **인증서**(certificate)를 검사해 상대가 진짜 그 호스트인지 확인한다.

### fung에서의 모양 {#shape}

- 클라이언트 함수는 응답을 `Ok({status, headers, header_lines, body})`로 준다. **응답을 받았으면 상태 코드가 무엇이든 `Ok`**다. "서버가 404라고 답했다"와 "요청이 가지 못했다"는 다른 일이기 때문이다. 상태 코드는 직접 검사한다.
- 요청이 가지 못하면(연결 거부, 없는 호스트, 인증서 오류 등) `Err({kind: 'network, message})`, 시간 안에 끝나지 않으면 `Err({kind: 'timeout, message})`다. `message`는 ``cannot request `URL`: 이유`` 꼴이고, 이유는 `connection refused`, `no such host`, ``unsupported scheme `ftp` ``, `invalid URL: …`, `unknown certificate authority`, ``certificate is not valid for `host` ``, `certificate has expired (valid until …)`, `too many redirects (limit 10)` 같은 짧은 문구다. 시간 초과의 문구는 ``request to `URL` timed out``이다. 문구 속 URL의 비밀번호는 언제나 `***`로 가린다(`http://ann:***@host/`): 에러는 로그에 남기 때문이다.
- 응답 `headers`는 맵이고 **키는 소문자**다(`r.headers["content-type"]`). 같은 이름의 헤더가 여럿이면 `", "`로 이어 하나로 준다. 받은 헤더를 더하거나 빼지 않는다(`Connection`·`Transfer-Encoding`도 그대로 있다).
- `header_lines`는 받은 헤더 줄을 **받은 순서 그대로** 담은 `(이름, 값)` 튜플의 리스트이고, 이름은 받은 대소문자 그대로다(`[("Content-Type", "text/html"), ("Set-Cookie", "a=1"), ("Set-Cookie", "b=2")]`). `Set-Cookie`처럼 `", "`로 이으면 어디서 나눌지 알 수 없는 헤더는 여기서 하나씩 읽는다.
- 본문은 문자열이다. JSON이면 [`json.parse`](json.html#fn-json-parse)로 읽는다. 문자열은 바이트를 그대로 담으므로 이미지 같은 바이너리 본문도 바뀌지 않는다([바이트](../prelude/strings.html#bytes)). URL 경로의 UTF-8이 아닌 바이트는 `%FF`처럼 퍼센트 인코딩해 보낸다.
- 태스크가 취소되면(바깥의 `timeout`, `cancel`) 요청도 멈춘다(E0701).

이 페이지의 예제는 네트워크나 열린 포트가 필요해서 실행하지 않고 검사만 한다. 출력 칸은 실제로 돌렸을 때의 예다.

## 클라이언트 {#client}

:::fn http.get(url, headers = {}, timeout = 30s, max_body = 67108864) fung

URL에 `GET` 요청을 보내고 응답을 돌려준다.

#### 설명

가장 흔한 요청이다. `curl URL`과 같다. 리다이렉트(3xx)는 10번까지 따라간다. 요청에는 `Host`, `User-Agent: fung/<버전>`이 자동으로 붙고, `headers`의 것이 같은 이름의 자동 헤더를 대신한다. URL에 `user:pass@`가 있으면 `Authorization: Basic …` 헤더가 된다. 프록시 환경 변수(`HTTPS_PROXY` 등)를 따른다. 자세한 규칙은 [약속](#promises)에 있다.

`timeout`은 이름 조회·연결·TLS 핸드셰이크·리다이렉트를 모두 포함한 **요청 전체**의 한도 하나다. prelude의 [`timeout`](../prelude/concurrency.html#fn-timeout)과 같은 규칙이라 `0s` 이하면 곧바로 시간 초과이고, 요청 자신의 한도를 두지 않으려면 `timeout: nil`을 준다. 바깥에 더 짧은 [`timeout`](../prelude/concurrency.html#fn-timeout) 범위가 있으면 그쪽이 먼저 끊는다(그때는 바깥 `timeout`의 `Err({kind: 'timeout})`).

#### 매개변수

- `url`: `http://`나 `https://`로 시작하는 URL.
- `headers`: 보낼 헤더 맵(`{"Authorization": "Bearer ${token}"}`). 맵 순서대로 보낸다. 값은 문자열이다. 이름은 토큰(영문자·숫자와 `` !#$%&'*+-.^_`|~ ``)이어야 하고, 값에는 탭 밖의 제어 문자(줄바꿈·NUL 등)가 올 수 없으며, 대소문자만 다른 이름 둘(`"x-a"`와 `"X-A"`)은 한 헤더라서, 어기면 모두 E0403이다. 그래서 값에 줄바꿈을 넣어 헤더를 끼워 넣을 수 없다. 본문의 끝을 정하는 `Content-Length`·`Transfer-Encoding`·`Trailer`는 클라이언트가 쓰므로, 사용자가 주면(대소문자 무관) 역시 보내기 전에 E0403이다.
- `timeout`: 요청 전체의 한도(기간), 또는 한도 없음 `nil`. `0s`는 곧바로 시간 초과, 음수 기간은 E0403.
- `max_body`: 읽을 응답 본문의 최대 바이트 수(기본 64 MiB). 넘으면 `Err({kind: 'network})`(`response body longer than N bytes`). 큰 파일을 받을 때 늘린다. 제한 없음은 없다.

#### 돌려주는 값

`Ok({status, headers, header_lines, body})`. `status`는 정수, `headers`는 소문자 키의 맵, `header_lines`는 받은 순서의 `(이름, 값)` 리스트, `body`는 문자열.

#### 에러

- `Err({kind: 'network, message})`: 연결·이름 조회·TLS·URL·리다이렉트 실패, 응답 본문이 `max_body`를 넘음.
- `Err({kind: 'timeout, message})`: `timeout` 안에 끝나지 않았다.
- 런타임 에러 E0403: 인자가 틀렸다(`url`이 문자열이 아님, 대소문자만 다른 헤더 이름 둘, 사용자 `Content-Length` 헤더 등). 인자는 무엇이든 보내기 전에 검사한다.

#### 예제

**상태와 헤더 보기**

```fung check
import http

r = http.get("https://example.com")!
print(r.status, r.headers["content-type"], len(r.body) > 0)
```

```expect
200 text/html; charset=utf-8 true
```

**JSON API 읽기**

```fung check
import http
import json

token = env("API_TOKEN") ?? fail("API_TOKEN is not set")
r = http.get("https://api.example.com/v1/hosts?env=prod", headers: {"Authorization": "Bearer ${token}"})!
if r.status != 200:
  eprint("HTTP ${r.status}: ${r.body}")
  exit(1)
;;
for h in json.parse(r.body)!.hosts:
  print(h.name, h.ip)
;;
```

```expect
web-1 10.0.1.11
web-2 10.0.1.12
```

**실패를 종류별로**

```fung check
import http

fn probe(url) = match http.get(url, timeout: 3s):
  Ok(r) when r.status < 400 => "up (${r.status})"
  Ok(r)                     => "error (${r.status})"
  Err({kind: 'timeout})     => "timeout"
  Err(e)                    => "down: ${e.message}"
;;

print(probe("https://example.com"))
print(probe("https://example.com/missing"))
print(probe("http://127.0.0.1:1/"))
print(probe("https://expired.badssl.com/"))
```

```expect
up (200)
error (404)
down: cannot request `http://127.0.0.1:1/`: connection refused
down: cannot request `https://expired.badssl.com/`: certificate has expired (valid until 2015-04-12T23:59:59Z)
```

**같은 이름의 헤더를 하나씩**

```fung check
import http

r = http.get("https://example.com/login")!
cookies = r.header_lines |> filter(((k, _)) => lower(k) == "set-cookie") |> map(((_, v)) => v)
for c in cookies:
  print(c)
;;
```

```expect
session=abc; Path=/; HttpOnly
theme=dark; Expires=Wed, 21 Oct 2026 07:28:00 GMT
```

`r.headers["set-cookie"]`였다면 두 쿠키가 `", "`로 이어져, 날짜 속의 쉼표 때문에 다시 나눌 수 없다.

#### 같이 보기

[`http.request`](#fn-http-request), [`json.parse`](json.html#fn-json-parse), [`conc.pmap`](conc.html#fn-conc-pmap), [URL 여러 개 상태 동시 확인](../../recipes/check-urls.html), [JSON API에서 필드 뽑기](../../recipes/json-fields.html)

:::

:::fn http.post(url, body, headers = {}, timeout = 30s, max_body = 67108864) fung

URL에 `POST` 요청으로 본문을 보내고 응답을 돌려준다.

#### 설명

본문은 문자열이다(`nil`이면 본문 없이 보낸다). JSON을 보내려면 [`json.stringify`](json.html#fn-json-stringify)로 바꾸고 `Content-Type: application/json` 헤더를 붙인다(자동으로 붙지 않는다). `Content-Length`는 클라이언트가 쓴다. 나머지는 [`http.get`](#fn-http-get)과 같다.

#### 매개변수

- `url`: 요청할 URL.
- `body`: 보낼 문자열 또는 `nil`.
- `headers`: 보낼 헤더 맵.
- `timeout`: 요청 전체의 한도.
- `max_body`: 읽을 응답 본문의 최대 바이트 수.

#### 돌려주는 값

`Ok({status, headers, header_lines, body})`.

#### 에러

[`http.get`](#fn-http-get)과 같다.

#### 예제

**JSON 보내기**

```fung check
import http
import json

payload = json.stringify({name: "web-1", env: "prod"})
r = http.post("https://httpbin.org/post", payload, headers: {"Content-Type": "application/json"})!
print(r.status, json.parse(r.body)!.json)
```

```expect
200 {env: "prod", name: "web-1"}
```

**웹훅으로 알리기**

```fung check
import http
import json

fn notify(text):
  url = env("WEBHOOK_URL") ?? fail("WEBHOOK_URL is not set")
  r = http.post(url, json.stringify({text}), headers: {"Content-Type": "application/json"}, timeout: 5s)?
  r.status < 300 then Ok(nil) else Err({kind: 'http, message: "webhook answered ${r.status}"})
;;

match notify("backup finished"):
  Ok(_)  => print("sent")
  Err(e) => eprint("notify failed:", e.message)
;;
```

```expect
sent
```

#### 같이 보기

[`http.request`](#fn-http-request), [`json.stringify`](json.html#fn-json-stringify)

:::

:::fn http.request(method, url, body = nil, headers = {}, timeout = 30s, max_body = 67108864) fung

아무 메서드로나 요청을 보낸다.

#### 설명

`PUT`, `DELETE`, `PATCH`, `HEAD` 등 `get`·`post`가 아닌 메서드에 쓴다. `method`는 **쓴 그대로** 보낸다: 메서드 이름은 대소문자를 가리므로(`"delete"`는 `DELETE`가 아니다) 보통 대문자로 쓴다. `HEAD` 응답과 1xx·204·304 응답에는 본문이 없다. 나머지는 [`http.get`](#fn-http-get)과 같다.

#### 매개변수

- `method`: 메서드 이름(문자열). 토큰 글자(영문자·숫자와 `` !#$%&'*+-.^_`|~ ``)로 된 한 글자 이상이어야 하고, 아니면(빈 문자열, 공백이 든 이름) E0403이다.
- `url`: 요청할 URL.
- `body`: 보낼 문자열 또는 `nil`.
- `headers`: 보낼 헤더 맵.
- `timeout`: 요청 전체의 한도.
- `max_body`: 읽을 응답 본문의 최대 바이트 수.

#### 돌려주는 값

`Ok({status, headers, header_lines, body})`.

#### 에러

[`http.get`](#fn-http-get)과 같다. 메서드가 토큰이 아니면 E0403.

#### 예제

**지우기와 바꾸기**

```fung check
import http
import json

base = "https://api.example.com/v1"
r = http.request("DELETE", "${base}/hosts/web-9")!
print("delete:", r.status)
r = http.request("PUT", "${base}/hosts/web-1", body: json.stringify({env: "staging"}), headers: {"Content-Type": "application/json"})!
print("put:", r.status)
```

```expect
delete: 204
put: 200
```

**`HEAD`로 크기만 보기**

```fung check
import http

r = http.request("HEAD", "https://httpbin.org/bytes/1024")!
print(r.status, r.headers["content-length"], r.body == "")
```

```expect
200 1024 true
```

#### 같이 보기

[`http.get`](#fn-http-get), [`http.post`](#fn-http-post)

:::

## 서버 {#server}

**서버**는 한 **주소**(호스트와 포트, `127.0.0.1:8080`)에서 연결을 기다리다가, 요청이 오면 답하는 프로그램이다. `127.0.0.1`은 자기 컴퓨터에서만 접속할 수 있고, `:8080`처럼 호스트를 비우면 모든 네트워크에서 받는다. 포트 `0`은 "남는 포트 아무거나"다.

:::fn http.serve(addr, handler, ready = nil, drain = 10s, reuse_port = false) fung

주소에서 HTTP 요청을 받아 `handler`로 답한다. 멈출 때까지 돌아오지 않는다.

#### 설명

`addr`에서 듣기 시작하고, 요청마다 **새 태스크**에서 `handler(req)`를 불러 그 결과로 답한다. 요청이 동시에 여럿 오면 핸들러도 동시에 돈다. 그래서 요청 수 같은 공유 상태는 [`atom`](../prelude/concurrency.html#fn-atom)에 둔다. 라우팅 기능은 따로 없고 `match req.path:`(또는 `match (req.method, req.path):`)로 나눈다.

`req`는 `{method, target, host, path, query, query_pairs, headers, header_lines, body}` 맵이다.

- `method`: `"GET"`, `"POST"` … 받은 그대로.
- `target`: 요청 줄의 요청 대상을 받은 그대로(이스케이프를 풀지 않음, `"/a%2Fb?tag=x&tag=y"`). `%2F`와 `/`를 가르거나 쿼리를 직접 나눌 때 쓴다.
- `host`: 요청한 호스트. 보통은 `Host` 헤더의 값이고(`"example.com:8080"`), 요청 대상이 절대 URL(`GET http://other/x`, 프록시에 보내는 꼴)이면 그 URL의 호스트다(이때 `Host` 헤더는 따르지 않는다). 둘 다 없으면(`Host` 없는 HTTP/1.0) `nil`. 가상 호스트를 가를 때 쓴다. 절대 URL의 호스트가 비었거나(`http:///x`) `user@`가 붙어 있으면 400으로 답한다.
- `path`: 쿼리를 뺀 경로, 이스케이프를 푼 것(`"/health"`). 절대 URL 대상의 빈 경로는 `"/"`다.
- `query`: 쿼리의 맵. 같은 이름이 여럿이면 **마지막 값**이다.
- `query_pairs`: 쿼리의 `(이름, 값)` 쌍을 받은 순서대로, 같은 이름도 모두 담은 리스트(`?tag=a&tag=b` → `[("tag", "a"), ("tag", "b")]`).
- `headers`: 소문자 키의 맵(같은 이름은 `", "`로 이음). `host`도 들어 있다. 받은 헤더를 더하거나 빼지 않는다.
- `header_lines`: 받은 헤더 줄을 받은 순서대로, 이름은 받은 대소문자 그대로 담은 `(이름, 값)` 리스트. 여러 줄의 `Cookie`처럼 `", "`로 이으면 뜻이 바뀌는 헤더를 읽을 때 쓴다.
- `body`: 본문 문자열.

쿼리는 `&`로 나누고, 쌍마다 첫 `=` 앞이 이름이다(`=`가 없으면 값은 `""`). `+`는 공백, `%20` 같은 이스케이프는 바이트로 풀린다. `;`는 보통 글자다(`z=1;2` → `("z", "1;2")`, 웹 폼의 규칙과 같다). 빈 조각(`a=1&&b=2`의 가운데)은 쌍이 아니다. 잘못된 `%` 이스케이프(`%zz`)는 경로에서처럼 핸들러를 부르지 않고 400으로 답하고, 쌍이 1만 개를 넘어도 400이다.

핸들러는 **문자열**(상태 200, `text/plain; charset=utf-8`)이나 **맵** `{status = 200, headers = {}, body = ""}`(빠진 키는 기본값)을 돌려준다. 응답에는 `Date`와 `Content-Length`가 붙고, 본문이 있는데 `content-type`이 없으면 `text/plain; charset=utf-8`이 붙는다. 연결마다 요청 하나만 받으므로 응답에는 늘 `Connection: close`가 붙는다.

답의 틀은 서버가 정한다. 그래서 핸들러가 `Transfer-Encoding`·`Trailer`·`Connection` 헤더를 넣거나, 본문 길이와 다른 `Content-Length`를 넣으면 잘못된 답이다(본문이 없는 `HEAD`·204·304의 `Content-Length`는 GET의 길이로 받는다). 대소문자만 다른 같은 이름의 헤더 둘(`X-A`와 `x-a`), 토큰이 아닌 헤더 이름(`"X Bad"`), 제어 문자(줄바꿈·NUL 등, 탭은 됨)가 든 헤더 값도 잘못된 답이다. 서버는 헤더를 고치거나 빼지 않는다(값 앞뒤의 공백만 뗀다). 상태는 200~599다: 핸들러는 마지막 답 하나를 돌려주므로 1xx(중간 답, 연결 넘겨주기 101)는 잘못된 답이다.

핸들러가 런타임 에러를 내면 500으로 답하고 표준 에러에 `http: handler failed: …` 한 줄을 쓴다. 문자열·맵이 아닌 것이나 잘못된 답을 돌려주면 500과 `http: bad handler response: …`(이유)다. 어느 쪽이든 서버는 계속 돈다.

**끝나는 길은 둘이다.**

- **정지 요청**: 첫 SIGINT(Ctrl-C)·SIGTERM(`kill`, 컨테이너 종료)이나 [`request_stop()`](../prelude/concurrency.html#fn-request_stop)이 오면([`stop_signal`](../prelude/concurrency.html#fn-stop_signal)) 새 연결을 그만 받고(듣던 소켓을 닫음), 처리 중인 요청을 최대 `drain`까지 마저 끝낸 뒤, 그때도 남은 것은 취소해 503으로 답하고 `Ok(nil)`을 돌려준다. 정상 종료다.
- **취소**: 서버가 든 태스크 범위가 취소되면(바깥 `timeout`, `cancel`, `exit`) 기다리지 않고 곧바로 멈춘다. 핸들러 안의 `exit`도 503이다.

`ready`를 주면 듣기 시작한 직후 실제 주소(`"127.0.0.1:53121"`)로 한 번 부른다. 포트 `0`으로 띄운 서버의 포트를 아는 길이고, 시험에서 서버가 준비된 뒤에 요청을 보내는 데 쓴다. `reuse_port: true`는 `SO_REUSEPORT`를 켜서 프로세스 여럿이 한 포트에서 받게 한다(Linux는 연결을 나눠 주고, macOS는 받게만 한다). 여러 코어를 쓰는 방법이다.

서버의 한도: 헤더는 10초 안에 와야 하고(아니면 끊음), 요청 머리는 1 MiB·헤더 줄 500개까지(넘으면 431), 본문은 16 MiB까지(넘으면 413, 핸들러를 부르지 않음; `Expect: 100-continue`면 `100 Continue`를 보내기 전에 413), 형식이 틀리면 400이다. `Transfer-Encoding`이 든 HTTP/1.0 요청은 본문의 끝을 알 수 없어 400이고, HTTP/1.0 요청의 `Expect`는 무시한다. 서버가 스스로 내는 답에는 `Date`·`Content-Length`가 붙고, 본문은 `<코드> <이유 문구>`다(`500 Internal Server Error`). 표준 이유 문구가 없는 코드(`599`)는 빈 문구로 보낸다. macOS는 받기 전 대기 줄(`kern.ipc.somaxconn`, 기본 128)이 차면 새 연결을 끊으므로, 한꺼번에 그보다 많이 몰려오는 접속은 그만큼 실패한다(Linux 기본값은 4096).

#### 매개변수

- `addr`: 들을 주소(`":8080"`, `"127.0.0.1:0"`). 포트는 숫자다. `":http"` 같은 이름은 풀지 않는다(``invalid port `http` (a port is a number)``): 작은 컨테이너에는 이름표(`/etc/services`)가 없어 같은 스크립트가 달라지기 때문이다.
- `handler`: `req`를 받아 문자열이나 응답 맵을 돌려주는 함수.
- `ready`: 실제 주소를 받는 함수 또는 `nil`.
- `drain`: 정지 요청 뒤 처리 중인 요청을 기다릴 최대 기간. 음수 기간은 E0403.
- `reuse_port`: `SO_REUSEPORT`를 켤지.

#### 돌려주는 값

정지 요청으로 끝나면 `Ok(nil)`.

#### 에러

- `Err({kind: 'network, message})`: 듣기 시작하지 못했다(``cannot listen on `127.0.0.1:80`: permission denied``, 이미 쓰는 포트, ``invalid address `bogus` (missing port)``).

정상 종료가 `Ok(nil)`이므로 `http.serve(…)!`는 듣기에 실패했을 때만 멈추고, 깔끔하게 멈춘 서버는 종료 코드 0으로 끝난다. `!` 없이 문장으로 써도 듣기 실패의 `Err`는 버려지는 `Err`(E0606)로 멈춘다.

#### 예제

**가장 작은 서버**

```fung check
import http

http.serve(":8080", req => match req.path:
  "/health" => "ok"
  _         => {status: 404, body: "not found"}
;;, ready: a => print("listening on", a))
```

```expect
listening on [::]:8080
```

다른 터미널에서 `curl -i localhost:8080/health`로 부르면 `200`과 `ok`, `/nope`는 `404`와 `not found`가 온다. Ctrl-C를 누르면 처리 중인 요청을 마치고 끝난다(종료 코드 0).

**JSON API와 공유 상태**

```fung check
import http
import json

fn json_response(status, data) = {status, headers: {"Content-Type": "application/json"}, body: json.stringify(data)}

hits = atom(0)

fn handle(req):
  n = swap(hits, x => x + 1)
  match (req.method, req.path):
    ("GET", "/stats") => json_response(200, {requests: n})
    ("POST", "/echo") => match json.parse(req.body):
      Ok(data) => json_response(200, {received: data, from: get(req.headers, "user-agent")})
      Err(e)   => {status: 400, body: "bad json: ${e.message}"}
    ;;
    ("GET", "/hello") => "hello, ${get(req.query, "name", "stranger")}"
    _                 => {status: 404, body: "not found: ${req.path}"}
  ;;
;;

http.serve("127.0.0.1:8080", handle, drain: 5s)
```

다른 터미널에서 불러 본 모습:

```plain
$ curl 'localhost:8080/hello?name=kim'
hello, kim
$ curl -d '{"x": 1}' localhost:8080/echo
{"received":{"x":1},"from":"curl/8.7.1"}
$ curl -d '{' localhost:8080/echo
bad json: invalid JSON at line 1, column 2: unexpected end of input
$ curl -i localhost:8080/stats
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 20 Oct 2026 14:06:38 GMT
Content-Length: 14
Connection: close

{"requests":4}
```

**시험 안에서 띄우고 부르기**

포트 `0`과 `ready`로 남는 포트에 띄우고, 채널로 주소를 받은 뒤 요청한다. 마지막에 `request_stop()`으로 서버를 정상 종료시킨다.

```fung check
import http

(tx, rx) = channel(1)
r = scope(fn(s):
  spawn(s, () => http.serve("127.0.0.1:0", req => "pong", ready: a => send(tx, a)!))
  base = "http://${recv(rx)!}"
  print(http.get("${base}/ping")!.body)
  request_stop()
  "stopped"
;;)
print(r)
```

```expect
pong
stopped
```

#### 같이 보기

[`stop_signal`](../prelude/concurrency.html#fn-stop_signal), [`request_stop`](../prelude/concurrency.html#fn-request_stop), [`atom`](../prelude/concurrency.html#fn-atom), [`json.stringify`](json.html#fn-json-stringify), [웹훅을 받는 작은 HTTP 서버](../../recipes/webhook-server.html)

:::

## 약속하는 것과 하지 않는 것 {#promises}

`http`가 지키는 동작이다. 여기 없는 세부는 바뀔 수 있다.

<ul>
  <li><b>클라이언트 헤더</b>: 요청에 <code>Host</code>, <code>User-Agent: fung/&lt;버전&gt;</code>, 본문이 있으면 <code>Content-Length</code>가 붙는다. 사용자 헤더는 이름을 표준 대소문자로 바꿔 맵 순서대로 보내고(그래서 대소문자만 다른 이름 둘은 E0403), 메서드는 쓴 그대로 보내고, 같은 이름이면 사용자 것이 이긴다(빈 <code>User-Agent</code>는 보내지 않는다). 비어 있지 않은 사용자 <code>Host</code>도 자동 값을 대신하고, 리다이렉트가 다른 호스트로 가면 그 요청부터 버린다. <code>Content-Length</code>·<code>Transfer-Encoding</code>·<code>Trailer</code>는 본문의 끝을 정하므로 언제나 클라이언트가 쓰고, 사용자 헤더에 있으면 E0403이다(조용히 빼지 않는다). URL의 <code>user:pass@</code>는 사용자 <code>Authorization</code>이 없을 때 <code>Authorization: Basic</code>이 된다. 응답 본문은 <code>Content-Length</code>, 조각 전송(chunked), 연결 닫힘 중 하나로 읽는다. HTTP/1.0 응답에 <code>Transfer-Encoding</code>이 있으면 본문의 끝을 알 수 없으므로 <code>Err({kind: 'network})</code>(<code>Transfer-Encoding in an HTTP/1.0 response</code>)다(서버가 같은 요청을 400으로 답하는 것과 같은 규칙). https에서 연결 닫힘까지 읽는 본문은 TLS의 끝 알림(close_notify)으로 끝나야 하고, TCP 연결만 끊기면 잘렸을 수 있으므로 <code>Err({kind: 'network})</code>(<code>connection closed without TLS close_notify</code>)다. 압축을 요청하지 않으므로 본문은 서버가 보낸 그대로다.</li>
  <li><b>리다이렉트</b>: 301·302·303은 <code>GET</code>으로 바꿔(원래 <code>HEAD</code>면 <code>HEAD</code>) 본문 없이, 307·308은 메서드와 본문을 지켜 따라간다. 10번까지 따라가고 11번째는 <code>Err({kind: 'network})</code>(<code>too many redirects (limit 10)</code>). 리다이렉트 응답의 본문은 읽지 않는다. 첫 요청의 origin을 떠나면 <code>Authorization</code>·<code>Cookie</code> 같은 민감한 헤더를 그 뒤로 내내 버린다. 떠나는 것은 첫 요청의 호스트나 그 하위 도메인이 아닌 곳, 다른 포트(쓰지 않았으면 스킴의 기본 포트로 견주므로 http에서 https로 가는 것도), https에서 http로 내려가는 곳이다. <code>Referer</code>는 붙이지 않는다(이전 URL의 쿼리에 비밀값이 있을 수 있다). 필요하면 <code>headers</code>로 준다.</li>
  <li><b>프록시</b>: 환경 변수 <code>HTTP_PROXY</code>(http용), <code>HTTPS_PROXY</code>(https용), <code>NO_PROXY</code>(이름·하위 도메인·IP·CIDR, 포트 선택)를 따르고 소문자 이름도 읽는다. <code>localhost</code>와 루프백 주소는 프록시를 거치지 않는다. CGI 프로그램(<code>REQUEST_METHOD</code>가 있음) 안에서는 <code>HTTP_PROXY</code>를 쓰지 않는다. https는 <code>CONNECT</code> 터널로 간다. 스킴 없는 값(<code>proxy.example:3128</code>)은 http 프록시다. URL로 읽히지 않는 값(스킴 뒤에 <code>//</code>가 없는 <code>http:/proxy:3128</code>도)은 그 프록시를 쓸 요청의 <code>Err({kind: 'network})</code>(<code>invalid HTTP_PROXY `…`: …</code>, 비밀번호는 <code>***</code>로 가림)이고, 프록시 없이 바로 가지 않는다. <code>Proxy-Authorization</code>은 프록시에만 간다: <code>headers</code>에 준 값이 프록시 URL의 <code>user:pass@</code>보다 앞서고, http 프록시면 요청에 한 번, https면 <code>CONNECT</code>에만 쓰며, 프록시를 거치지 않는 요청에는 보내지 않는다. 리다이렉트가 첫 origin을 떠나면 다른 자격 증명처럼 그 뒤로 버린다(홉마다 프록시가 다를 수 있다). SOCKS 프록시와 https 프록시 너머의 https는 지원하지 않는다(<code>Err({kind: 'network})</code>).</li>
  <li><b>https</b>: 운영체제의 루트 인증서로 검증하고 SNI와 ALPN <code>http/1.1</code>을 보낸다. <code>SSL_CERT_FILE</code>·<code>SSL_CERT_DIR</code>이 하나라도 있으면 <b>그 변수들이 가리키는 것만</b> 믿는다(사내 CA 묶음처럼 "이것만 믿어라"). 인증서 오류는 <code>Err({kind: 'network})</code>다. 믿을 루트가 하나도 없으면 <code>no root certificates found (set SSL_CERT_FILE)</code>, 변수가 가리키는 파일을 읽지 못하면 그 까닭(<code>cannot read SSL_CERT_FILE `/x`: no such file or directory</code>)이 붙는다.
    <b>컨테이너</b>: fung의 Linux 바이너리는 정적이라 어디서나 돌지만, 루트 인증서는 실행 중에 시스템 파일에서 읽는다. <code>scratch</code>·distroless처럼 CA 파일이 없는 이미지에서는 <code>ca-certificates</code> 묶음을 복사하고(<code>COPY --from=… /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/</code>) 필요하면 <code>SSL_CERT_FILE</code>로 가리킨다.</li>
  <li><b>연결</b>: 이름의 주소가 여럿이면 IPv6와 IPv4를 번갈아 놓고, 앞 주소가 250 ms 안에 연결되지 않으면 다음 주소도 시작해 먼저 된 것을 쓴다(RFC 8305 "Happy Eyeballs"). IPv6 경로가 막힌 망에서도 IPv4로 곧 넘어간다. 이름 조회는 중간에 끊을 수 없어서, 취소되면 요청은 바로 돌아오지만 조회는 해석기가 답할 때까지 뒤에서 남는다(한 번에 8개까지, 파일 일과 따로).</li>
  <li><b>응답 본문 상한</b>은 요청의 <code>max_body</code>(기본 64 MiB). 넘으면 <code>Err({kind: 'network})</code>.</li>
  <li><b>호스트는 ASCII</b>로 보낸다. 비ASCII 이름(<code>例え.jp</code>, <code>ex%C3%A9.com</code>)은 퓨니코드(<code>xn--…</code>)로 쓰라는 에러다. 괄호 안은 IPv6 주소여야 한다.</li>
  <li><b>이른 답</b>: 서버가 요청을 다 받기 전에 답하고 닫으면(이른 413) 그 답을 준다. 프록시의 <code>CONNECT</code> 답은 2xx면 모두 터널이다. <code>http</code>·<code>https</code> 밖의 스킴인 프록시는 에러다.</li>
  <li><b>본문은 문자열</b>이다. 바이트를 그대로 담으므로 UTF-8이 아닌 본문(바이너리)도 바뀌지 않고 오간다. 요청 경로와 쿼리의 퍼센트 이스케이프도 바이트로 풀린다.</li>
  <li><b>URL의 경로와 쿼리</b>는 요청 줄에 ASCII로 나간다: 거기 쓸 수 있는 글자와 올바른 <code>%XX</code>는 그대로, 나머지 바이트(공백, 한글 같은 UTF-8, 홀로 선 <code>%</code>)는 <code>%XX</code>로 바꿔 보낸다(<code>?q=한 글</code> → <code>?q=%ED%95%9C%20%EA%B8%80</code>).</li>
  <li><b>응답의 상태 줄</b>은 <code>HTTP/1.x</code>와 100~599의 세 자리 숫자여야 한다. 아니면 <code>Err({kind: 'network})</code>(<code>invalid status code `999`</code>, <code>invalid HTTP version `HTTP/2.0`</code>).</li>
  <li><b>없는 것</b>: 연결 재사용(keep-alive)과 파이프라이닝, HTTP/2, 압축. 서버는 연결마다 요청 하나만 받는다.</li>
</ul>

배우는 순서로는 [13장 JSON과 HTTP](../../learn/13-json-and-http.html)를 본다.
