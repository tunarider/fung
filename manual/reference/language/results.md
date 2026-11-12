---
description: Ok·Err, !·?·??, 버려지는 Err.
lede: <code>Ok</code>·<code>Err</code>, <code>!</code>·<code>?</code>·<code>??</code>, 버려지는 <code>Err</code>.
---
# 결과값과 에러

<p>실패할 수 있는 함수는 <code>Ok(값)</code>이나 <code>Err(내용)</code>을 돌려준다. 표준 라이브러리의 <code>Err</code> 내용은 <code>{kind: 심볼, message: 문자열, …}</code> 맵이다. 세 축약 연산자로 다룬다.</p>
<table>
  <thead><tr><th>왼쪽 값</th><th><code>x!</code></th><th><code>x?</code></th><th><code>x ?? b</code></th></tr></thead>
  <tbody>
    <tr><td><code>Ok(v)</code></td><td><code>v</code></td><td><code>v</code></td><td><code>v</code></td></tr>
    <tr><td><code>Err(e)</code></td><td>프로그램 중단(종료 코드 1)</td><td>현재 함수에서 <code>Err(e)</code>를 반환</td><td><code>b</code></td></tr>
    <tr><td><code>nil</code></td><td>런타임 에러(E0604)</td><td>런타임 에러(E0604)</td><td><code>b</code></td></tr>
    <tr><td>그 외</td><td>런타임 에러(E0605)</td><td>런타임 에러(E0605)</td><td>그 값 그대로</td></tr>
  </tbody>
</table>
<p>규칙은 하나다: <code>!</code>·<code>?</code>는 결과값만 받는다. <code>nil</code>을 돌려주는 함수(<code>get</code>·<code>find</code>·<code>env</code> 등)에 붙이면 에러이고, 없을 때의 값은 <code>??</code>로 준다. 없을 때 호출자에게 넘기려면 <code>match</code>나 <code>if x == nil: return nil ;;</code>로 드러내 쓴다.</p>

```fung frag
text = fs.read("a.json")!                    # 스크립트: 실패하면 멈춘다
data = json.parse(text)?                      # 함수 안: 실패를 호출자에게 넘긴다
port = env("PORT") ?? "8080"                  # 없거나 실패하면 기본값(모든 Err)
match fs.read(p):
  Ok(text)                => use(text)
  Err({kind: 'not_found}) => print("no file")
  Err(e)                  => print(e.message)
;;
```

<p><strong>실행 중에 실패할 수 있는 함수는 결과값을 돌려준다.</strong> 넘겨받은 함수 <code>f</code>를 불러 그 값을 내주는 함수도 런타임이 <code>Err</code>를 만들 수 있으면(시간 초과·취소·파일 열기 실패) 같다: <code>scope</code>·<code>supervise</code>·<code>await</code>·<code>timeout</code>·<code>fs.with_open</code>·<code>fs.with_temp_dir</code>는 성공하면 <code>f</code>의 값을 <code>Ok</code>로 감싼다. 그래서 <code>!</code>·<code>?</code>가 <code>f</code>가 무엇을 돌려주든 붙고, <code>f</code>가 돌려준 <code>Err</code>는 <code>Ok(Err(e))</code>로 런타임의 <code>Err</code>와 구별된다. <code>f</code>가 결과값을 돌려주면 두 겹이 드러나므로 한 겹씩 푼다: <code>timeout(() =&gt; http.get(url), 2s)!?</code>는 시간 초과에서 멈추고 요청의 실패는 호출자에게 넘긴다. 런타임 에러가 실패로 올라가고 <code>Err</code>를 만들지 않는 함수(<code>with</code>·<code>swap</code>·<code>conc.pmap</code>)는 <code>f</code>의 값을 그대로 준다.</p>

```fung frag
n = timeout(() => count_rows(db), 5s)!       # Ok(n)이면 n, 시간 초과면 멈춘다
body = timeout(() => http.get(url), 2s)!?    # 시간 초과는 멈추고, 요청 실패는 넘긴다
match await(t):
  Ok(v)  => use(v)                           # 태스크 함수의 값(Err일 수도 있다)
  Err(e) => print("task died: ${e.message}") # 런타임 에러로 죽었거나 취소됐다
;;
```

<p><strong>버려지는 <code>Err</code>는 에러다.</strong> 값을 쓰지 않는 자리(문장으로 쓴 호출, 그런 자리의 <code>if</code>·<code>match</code> 가지, <code>for</code> 문장의 본문, 파일의 마지막 식, <code>each</code>의 <code>f</code>, <code>with</code>의 <code>release</code>, <code>supervise</code>의 <code>on_fail</code>, <code>test</code> 블록이 돌려준 값)에서 <code>Err</code>가 나오면 E0606 <code>discarded Err</code>로 멈춘다. 버려지는 값이 리스트·맵·튜플·태그면 그 안을 깊이와 상관없이 보고(<code>[[Err(e)]]</code>, <code>{a: Err(e)}</code>, <code>Ok(Err(e))</code>도), 처음 나온 <code>Err</code>를 어디 있는지와 함께 알린다. 그래서 <code>for</code>로 끝나는 함수를 문장으로 부르거나 <code>map(…)</code>을 문장으로 써도 실패가 사라지지 않는다. <code>set -e</code>를 늘 켜 둔 셸과 같다. 실패해도 상관없으면 <code>_ = 식</code>으로 일부러 버린다. 함수의 마지막 식은 반환값이라 버려지지 않고, REPL은 입력의 마지막 값을 출력한다.</p>

```fung frag
fs.write(p, text)          # 실패하면 E0606
fs.write(p, text)!         # 실패하면 E0603으로 중단
_ = fs.remove(tmp)         # 실패해도 상관없다
each(rxs, r => send(tx, r))   # 닫힌 tx면 E0606
```

<p><code>Ok</code>·<code>Err</code>는 값을 <strong>정확히 하나</strong> 담는다. 식·패턴·파이프(<code>x |&gt; Ok(2)</code>는 <code>Ok(x, 2)</code>)에서 개수가 다르면 컴파일 에러 E0236이다. 여러 값을 돌려주려면 <code>Ok((a, b))</code>처럼 튜플 하나에 담는다. 그래서 이름이 <code>Ok</code>·<code>Err</code>인 태그는 모두 결과값이고, 연산자·패턴·<code>is_ok</code>·보간이 결과값을 같게 본다. 타입 표기 <code>Ok</code>·<code>Err</code>도 결과값만 받는다(<code>sym("Ok")</code> 같은 심볼은 받지 않는다). 값이 없는 맨 <code>Ok</code>·<code>Err</code>는 식에도 패턴에도 쓸 수 없고(E0236), 파이프의 오른쪽 <code>x |&gt; Ok</code>만 된다(파이프가 값을 채우므로). 결과값인지는 <code>is_ok</code>·<code>is_err</code>나 <code>match</code>로 가린다.</p>
<p><code>??</code>는 <strong>모든</strong> <code>Err</code>를 기본값으로 바꾼다. <code>fs.read(p) ?? "{}"</code>는 파일이 없을 때만이 아니라 권한 오류일 때도 <code>"{}"</code>다. 종류를 가리려면 <code>match</code>로 <code>Err({kind: 'not_found}) =&gt; "{}"</code>처럼 쓰고 나머지는 <code>!</code>로 멈춘다.</p>
<p><code>??</code>는 <code>nil</code>과 결과값을 한 연산자로 다루므로(위 표), 왼쪽이 <code>Ok(v)</code>이면 언제나 <code>v</code>로 벗긴다. 맵에 결과값을 담아 두었다면 <code>get({k: Ok(1)}, "k") ?? 0</code>은 <code>Ok(1)</code>이 아니라 <code>1</code>이다. 결과값 그대로 꺼내려면 <code>get</code>의 <code>nil</code>을 <code>== nil</code>로 따로 본다.</p>
<p><strong>결과값과 <code>nil</code>은 글에 말없이 넣지 않는다.</strong> 문자열 보간과 <code>join</code>에 결과값(<code>Ok</code>·<code>Err</code>), <code>nil</code>, 핸들(함수·모듈·태스크·범위·채널 끝·atom·파일·소켓)이 들어가면, 리스트·맵·튜플·태그 안 어느 깊이에 있어도 E0519다. 풀기를 잊은 <code>"http://${recv(rx)}/"</code>가 <code>http://Ok("…")/</code>라는 주소가, 없는 환경 변수가 <code>http://nil/</code>이 되지 않게 하려는 것이다. 결과는 <code>!</code>·<code>?</code>로 풀고 <code>nil</code>에는 <code>??</code>로 기본값을 준다. 값을 그대로 글로 쓰려면(로그) <code>str(x)</code>로 바꾸고, <code>print(x)</code>는 무엇이든 그대로 보인다.</p>
<p>"여기 오면 버그"를 말할 때는 <code>fail(msg)</code>로 런타임 에러를 낸다. <code>x ?? fail("config.port is required")</code>처럼 <code>??</code>와 짝이 맞는다. 런타임 에러를 잡는 구문은 없다. 런타임 에러가 값이 되는 곳은 태스크 경계 하나다: <code>supervise</code> 안의 태스크가 런타임 에러로 죽으면 <code>on_fail(e)</code>와 그 태스크의 <code>await</code>가 <code>{kind: 'error, code, message, at}</code> 맵을 받는다(<a href="../prelude/concurrency.html#fn-supervise"><code>supervise</code></a>). 정리는 <code>with(acquire, use, release)</code>(와 그 위의 <code>fs.with_open</code>)로 한다.</p>

## 경계 사례와 에러 {#edges}

`!`는 `Err`에서 프로그램을 멈춘다(메시지와 위치를 보인다).

```fung run exit=1 error=E0603
print(parse_int("42")!)
print(parse_int("x")!)
```

`Err`를 문장으로 버리면 에러다. 일부러 버리려면 `_ = …`.

```fung run exit=1 error=E0606
fn port(s) = parse_int(s)
_ = port("x")
port("y")
```

리스트·맵·튜플·태그 안의 `Err`도 버려지면 에러다.

```fung run exit=1 error=E0606
fn check_all() = {web: Ok(1), db: Err("down")}
check_all()
```

`?`도 `nil`을 받지 않는다. `nil`을 돌려주는 함수에 붙인 `?`는 E0604다.

```fung run exit=1 error=E0604
fn home() = env("FUNG_NO_SUCH_VARIABLE")?
print(home())
```

`?`는 함수 안에서만 쓴다. 최상위에서는 `!`를 쓴다.

```fung run exit=2 error=E0214
n = parse_int("1")?
print(n)
```
