---
lede: 실패할 수 있는 함수는 <code>Ok(값)</code>이나 <code>Err(내용)</code>을 돌려준다. <code>Ok</code>·<code>Err</code>는 평범한 태그다. 풀 때는 주로 <code>!</code> <code>?</code> <code>??</code>나 <code>match</code>를 쓰고(<a href="../language/results.html">문법</a>), 이 페이지의 두 함수는 결과값을 가릴 때 쓴다.
description: fung prelude 결과값: Ok와 Err, is_ok, is_err.
---
# 결과값

## 결과값이란 {#concepts}

파일 읽기, 수 읽기, 외부 명령 실행처럼 **실패할 수 있는 일**이 있다. 파일이 없을 수도 있고, 사용자가 `"abc"`를 포트 번호로 줄 수도 있다. 많은 언어는 이런 실패를 **예외**(exception)로 다룬다. 실패가 일어나면 지금 하던 일을 멈추고 어딘가의 `try`/`catch`로 튀어 간다. 어디서 튈지 함수 모양만 보고는 알 수 없다는 것이 단점이다.

fung은 실패를 **값**으로 돌려준다. 실패할 수 있는 함수는 성공하면 `Ok(값)`을, 실패하면 `Err(내용)`을 돌려준다. 이 둘을 **결과값**(result)이라고 부른다. 결과값은 평범한 [태그](../language/values.html) 값이라 변수에 담고, 리스트에 넣고, 함수에 넘길 수 있다. 받은 쪽은 결과를 꺼내기 전에 성공인지 실패인지 정해야 하므로 실패를 모르는 사이에 지나치지 않는다.

표준 라이브러리의 `Err` 내용은 `{kind: 심볼, message: 문자열, …}` 맵이다. `kind`로 종류를 가리고(`'not_found`, `'parse`, `'timeout` …), `message`를 사람에게 보여 준다.

```fung run
print(parse_int("42"))
print(parse_int("abc"))
```

결과값을 푸는 방법은 넷이다. 자세한 규칙은 [결과값과 에러](../language/results.html)에 있다.

<table>
  <thead><tr><th>쓰는 법</th><th>뜻</th><th>어울리는 자리</th></tr></thead>
  <tbody>
    <tr><td><code>r!</code></td><td><code>Ok(v)</code>면 <code>v</code>, <code>Err</code>면 프로그램을 멈춘다(E0603)</td><td>스크립트의 맨 위: 실패하면 더 할 일이 없을 때</td></tr>
    <tr><td><code>r?</code></td><td><code>Ok(v)</code>면 <code>v</code>, <code>Err</code>면 지금 함수가 그 <code>Err</code>를 돌려준다</td><td>함수 안: 실패를 부른 쪽에 넘길 때</td></tr>
    <tr><td><code>r ?? 기본값</code></td><td><code>Ok(v)</code>면 <code>v</code>, <code>Err</code>(나 <code>nil</code>)면 기본값</td><td>실패해도 쓸 값이 있을 때</td></tr>
    <tr><td><code>match r:</code></td><td><code>Ok(v) =&gt;</code>와 <code>Err(e) =&gt;</code>로 가른다</td><td>실패의 종류마다 다르게 할 때</td></tr>
  </tbody>
</table>

```fung run
fn port_of(s):
  n = parse_int(s)?
  n > 0 and n < 65536 then Ok(n) else Err({kind: 'invalid, message: "out of range: ${n}"})
;;

print(port_of("8080"), port_of("x") ?? 80)
match port_of("70000"):
  Ok(p)  => print("port", p)
  Err(e) => print("bad:", e.message)
;;
```

결과값을 쓰지 않고 버리면(문장으로 쓴 호출이 `Err`를 돌려주면) E0606으로 멈춘다. 실패를 조용히 넘기지 않기 위한 규칙이다. 일부러 버릴 때는 `_ = 식`으로 쓴다.

## 실패의 종류 {#kinds}

표준 라이브러리가 쓰는 `kind`는 아래가 전부다. 같은 원인은 어느 함수에서나 같은 `kind`다.

<table>
  <thead><tr><th><code>kind</code></th><th>뜻</th><th>주로 내는 곳</th></tr></thead>
  <tbody>
    <tr><td><code>'not_found</code></td><td>경로나 명령이 없다. 경로 중간이 디렉터리가 아닌 것도(<code>ENOENT</code>·<code>ENOTDIR</code>)</td><td><code>fs</code>, <code>exec</code>·<code>sh</code></td></tr>
    <tr><td><code>'permission</code></td><td>권한이 없다(<code>EACCES</code>·<code>EPERM</code>)</td><td><code>fs</code>, <code>exec</code>·<code>sh</code></td></tr>
    <tr><td><code>'exists</code></td><td>이미 있다. 비지 않은 디렉터리도(<code>EEXIST</code>·<code>ENOTEMPTY</code>)</td><td><code>fs</code></td></tr>
    <tr><td><code>'invalid</code></td><td>받은 값의 모양이 틀렸다: NUL이 든 경로·인자, 운영체제가 거절한 인자(<code>EINVAL</code>), URL·스킴·주소·프록시 값의 문법, 잘못된 명령줄</td><td><code>fs</code>, <code>exec</code>·<code>sh</code>, <code>http</code>, <code>cli</code></td></tr>
    <tr><td><code>'io</code></td><td>그 밖의 입출력 실패(디스크가 참 등), 한도를 넘는 바깥 글(<code>max_body</code>를 넘는 응답, 너무 긴 출력)</td><td><code>fs</code>, <code>exec</code>·<code>sh</code>, <code>http</code>, 표준 입력</td></tr>
    <tr><td><code>'network</code></td><td>전송 실패: 연결 거부, 이름 조회 실패, 끊긴 연결, 잘못된 응답</td><td><code>http</code></td></tr>
    <tr><td><code>'closed</code></td><td>닫힌 핸들을 썼거나 다시 닫았다. 채널에서는 정상적인 끝(닫혔고 남은 값이 없음)</td><td>파일 핸들, 채널</td></tr>
    <tr><td><code>'timeout</code>·<code>'cancelled</code></td><td>기한이 지났다, 취소됐다</td><td><code>timeout</code>, <code>scope</code>, <code>await</code>, <code>http</code></td></tr>
    <tr><td><code>'parse</code></td><td>글을 값으로 읽지 못했다</td><td><code>parse_*</code>, <code>json.parse</code>, <code>time.parse</code></td></tr>
    <tr><td><code>'exit</code></td><td>명령이 0이 아닌 코드나 신호로 끝났다</td><td><code>exec</code>·<code>sh</code></td></tr>
    <tr><td><code>'error</code></td><td>태스크가 런타임 에러로 죽었다</td><td><code>await</code>, <code>supervise</code></td></tr>
    <tr><td><code>'empty</code>·<code>'full</code></td><td>기다리지 않는 채널 연산이 기다려야 했다</td><td><code>recv_now</code>, <code>send_now</code></td></tr>
  </tbody>
</table>

운영체제의 에러 번호(errno)는 표의 앞 다섯 줄로 옮긴다: `ENOENT`·`ENOTDIR` → `'not_found`, `EACCES`·`EPERM` → `'permission`, `EEXIST`·`ENOTEMPTY` → `'exists`, NUL·`EINVAL` → `'invalid`, 나머지 → `'io`. `fs`·`exec`·`sh`가 이 한 표를 쓴다. `'invalid`처럼 다시 해도 같은 실패와 `'network`처럼 다시 해 볼 만한 실패를 `kind`로 가를 수 있다.

`'closed`는 두 뜻이다. 파일·소켓 핸들에서는 "이미 닫은 핸들을 썼다"(버그)이고, 채널에서는 "보내는 쪽이 모두 끝났고 남은 값이 없다"(정상적인 끝)이다. 흐름의 끝도 두 모양이다: 파일과 표준 입력의 `read_line`은 끝에서 `Ok(nil)`이고, 채널의 `recv`는 `Err({kind: 'closed})`다. 채널은 `nil`을 값으로 보낼 수 있어 `Ok(nil)`이 끝을 뜻할 수 없기 때문이다. 채널을 끝까지 받을 때는 `for x in rx:`가 닫힘에서 멈추므로 `'closed`를 직접 볼 일이 드물다.

결과값과 다른 실패도 하나 있다. **런타임 에러**는 코드가 틀린 경우(없는 키 읽기, 0으로 나누기, 타입이 틀린 인자)에 프로그램을 멈추는 것으로, 값으로 받을 수 없다(태스크 경계의 [`supervise`](concurrency.html#fn-supervise)만 예외로 그 태스크의 에러를 `Err({kind: 'error, …})`로 준다). 에러 번호와 뜻은 [에러 목록](../errors.html)에 있다.

## 함수 {#functions}

:::fn is_ok(x) fung

`Ok(…)`이면 `true`.

#### 설명

`match x: Ok(_) => true; _ => false ;;`와 같다. 결과값이 아닌 값(`nil`, 수 등)에도 에러 없이 `false`를 돌려준다. 하나의 결과값을 다룰 때는 `match`나 `!`·`?`·`??`가 낫다. `is_ok`는 결과값의 **리스트**를 거를 때 빛난다: `rs |> filter(is_ok)`.

#### 매개변수

- `x`: 아무 값.

#### 돌려주는 값

`bool`.

#### 에러

없다.

#### 예제

**여러 값**

```fung run
print(is_ok(Ok(1)), is_ok(Err(1)), is_ok(nil), is_ok(42))
```

**성공한 것만 세기**

```fung run
inputs = ["80", "http", "443", ""]
rs = map(inputs, parse_int)
print(len(filter(rs, is_ok)), "of", len(rs), "parsed")
print(filter(rs, is_ok) |> map(r => r!))
```

#### 같이 보기

[`is_err`](#fn-is_err), [`try_map`](collections.html#fn-try_map), [`type_of`](convert.html#fn-type_of)

:::

:::fn is_err(x) fung

`Err(…)`이면 `true`.

#### 설명

`match x: Err(_) => true; _ => false ;;`와 같다. 결과값이 아닌 값에는 `false`다. 실패한 것만 모아 보고할 때 쓴다.

#### 매개변수

- `x`: 아무 값.

#### 돌려주는 값

`bool`.

#### 에러

없다.

#### 예제

**여러 값**

```fung run
print(is_err(Err("x")), is_err(Ok(1)), is_err(nil))
```

**실패한 것만 보고하기**

```fung run
hosts = ["web-1", "web-2", "web-3"]
rs = map(hosts, h => exec(["sh", "-c", h == "web-2" then "exit 3" else "true"]))
bad = zip(hosts, rs) |> filter(((_, r)) => is_err(r)) |> map(((h, _)) => h)
print(len(bad), "failed:", bad)
```

#### 같이 보기

[`is_ok`](#fn-is_ok), [`match`와 패턴](../language/match.html)

:::
