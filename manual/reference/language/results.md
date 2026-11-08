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
    <tr><td><code>nil</code></td><td>런타임 에러</td><td><code>nil</code> 그대로</td><td><code>b</code></td></tr>
    <tr><td>그 외</td><td>런타임 에러</td><td>런타임 에러</td><td>그 값 그대로</td></tr>
  </tbody>
</table>

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

<p><strong>버려지는 <code>Err</code>는 에러다.</strong> 값을 쓰지 않는 자리(문장으로 쓴 호출, 그런 자리의 <code>if</code>·<code>match</code> 가지, <code>for</code> 문장의 본문, 파일의 마지막 식, <code>each</code>의 <code>f</code>, <code>with</code>의 <code>release</code>, <code>supervise</code>의 <code>on_fail</code>, <code>test</code> 블록이 돌려준 값)에서 <code>Err</code>가 나오면 E0606 <code>discarded Err</code>로 멈춘다. 버려지는 값이 리스트면 그 원소(한 겹만)를 보고, 처음 나온 <code>Err</code>를 몇 번째 원소인지와 함께 알린다. 그래서 <code>for</code>로 끝나는 함수를 문장으로 부르거나 <code>map(…)</code>을 문장으로 써도 실패가 사라지지 않는다. <code>set -e</code>를 늘 켜 둔 셸과 같다. 실패해도 상관없으면 <code>_ = 식</code>으로 일부러 버린다. 함수의 마지막 식은 반환값이라 버려지지 않고, REPL은 입력의 마지막 값을 출력한다.</p>

```fung frag
fs.write(p, text)          # 실패하면 E0606
fs.write(p, text)!         # 실패하면 E0603으로 중단
_ = fs.remove(tmp)         # 실패해도 상관없다
each(rxs, r => send(tx, r))   # 닫힌 tx면 E0606
```

<p><code>Ok</code>·<code>Err</code>는 값을 하나 담을 때만 결과값이다. <code>Ok(1, 2)</code>는 같은 이름의 보통 태그라 <code>!</code>·<code>?</code>는 E0605이고 <code>??</code>는 그대로 둔다. 여러 값을 돌려주려면 <code>Ok((a, b))</code>처럼 튜플 하나에 담는다.</p>
<p><code>??</code>는 <strong>모든</strong> <code>Err</code>를 기본값으로 바꾼다. <code>fs.read(p) ?? "{}"</code>는 파일이 없을 때만이 아니라 권한 오류일 때도 <code>"{}"</code>다. 종류를 가리려면 <code>match</code>로 <code>Err({kind: 'not_found}) =&gt; "{}"</code>처럼 쓰고 나머지는 <code>!</code>로 멈춘다.</p>
<p><strong>결과값은 글에 말없이 넣지 않는다.</strong> 문자열 보간과 <code>join</code>에 <code>Ok</code>·<code>Err</code>가 들어가면 E0519다. 풀기를 잊은 <code>"http://${recv(rx)}/"</code>가 <code>http://Ok("…")/</code>라는 주소가 되지 않게 하려는 것이다. 결과 자체를 글로 쓰려면 <code>str(r)</code>로 바꾸고, <code>print(r)</code>은 그대로 보인다. 리스트·맵 안의 결과는 리스트의 모양 그대로 보인다.</p>
<p>"여기 오면 버그"를 말할 때는 <code>fail(msg)</code>로 런타임 에러를 낸다. <code>x ?? fail("config.port is required")</code>처럼 <code>??</code>와 짝이 맞는다. 에러를 잡는 구문은 없고, 정리는 <code>with(acquire, use, release)</code>(와 그 위의 <code>fs.with_open</code>)로 한다.</p>

## 경계 사례와 에러 {#edges}

`!`는 `Err`에서 프로그램을 멈춘다(메시지와 위치를 보인다).

```fung run exit=1 error=E0603
print(parse_int("42")!)
print(parse_int("x")!)
```

`Err`를 문장으로 버리면 에러다. 일부러 버리려면 `_ = …`.

```fung run exit=1 error=E0606
_ = parse_int("x")
parse_int("y")
```

`?`는 함수 안에서만 쓴다. 최상위에서는 `!`를 쓴다.

```fung run exit=2 error=E0214
n = parse_int("1")?
print(n)
```
