---
description: 토큰의 규칙: 이름, 숫자, 문자열, 기간, 주석, 예약어.
lede: 토큰의 규칙: 이름, 숫자, 문자열, 기간, 주석, 예약어.
---
# 어휘

<h3>주석</h3>
<p><code>#</code>부터 줄 끝까지. 다른 주석 형식은 없다(<code>//</code>는 정수 나눗셈이다). 파일 첫 줄의 <code>#!/usr/bin/env fung</code>도 그냥 주석이다.</p>
<h3>이름과 태그</h3>
<ul>
  <li>소문자, <code>_</code>, 또는 대소문자가 없는 글자(한글 등)로 시작하면 <strong>이름</strong>: <code>file_list</code>, <code>_cache</code>, <code>파일목록</code>.</li>
  <li>대문자로 시작하면 <strong>태그</strong>: <code>Ok</code>, <code>Circle</code>. 선언 없이 쓴다(<a href="values.html">값과 타입</a>). 그래서 대문자 이름은 변수가 될 수 없다: <code>MAX_RETRIES = 3</code>과 <code>{HOME} = cfg</code>는 컴파일 에러 E0225다. 상수도 소문자로 쓰고(<code>max_retries = 3</code>), 대문자 키는 <code>{HOME: home} = cfg</code>나 <code>cfg.HOME</code>으로 읽는다.</li>
  <li>이름 표기는 snake_case. <code>_</code>로 시작하는 최상위 이름은 모듈 밖에서 보이지 않는다.</li>
  <li>이어지는 글자는 글자·숫자·<code>_</code>와 결합 문자(유니코드 UAX #31의 식별자 규칙): <code>हिंदी</code>, <code>café</code>. 이름은 NFC로 정규화해서, 조합형과 분해형으로 쓴 <code>é</code>는 같은 이름이다. 맵 키 자리(<code>m.k</code>, <code>{k: 1}</code>, 패턴 <code>{k: v}</code>)의 식별자는 문자열이라 쓴 그대로다(<code>m.k</code>는 정확히 <code>m["k"]</code>). 모듈 멤버(<code>lib.이름</code>)는 이름이라 NFC로 찾는다.</li>
</ul>
<h3>예약어</h3>
<p><code>fn</code> <code>if</code> <code>else</code> <code>then</code> <code>match</code> <code>when</code> <code>for</code> <code>in</code> <code>not</code> <code>and</code> <code>or</code> <code>return</code> <code>import</code> <code>as</code> <code>type</code> <code>test</code> <code>assert</code> <code>true</code> <code>false</code> <code>nil</code></p>
<p>이름만 올 수 있는 세 자리, 곧 <code>.</code> 뒤, 맵 리터럴의 키, 맵 패턴의 키에서는 예약어도 그냥 이름이다. JSON의 <code>type</code> 같은 키를 그대로 쓰기 위해서다. 변수나 함수 이름으로는 쓸 수 없다.</p>

```fung run
event = {type: "deploy", match: "api-*"}
print(event.type, event.match)
```

<h3>숫자</h3>
<table>
  <thead><tr><th>형태</th><th>예</th><th>비고</th></tr></thead>
  <tbody>
    <tr><td>정수</td><td><code>42</code> <code>1_000_000</code></td><td>64비트. <code>_</code>는 숫자 사이에만. 두 자리 이상이면 <code>0</code>으로 시작하지 않는다(<code>0755</code>는 E0102, 8진수는 <code>0o755</code>).</td></tr>
    <tr><td>진법</td><td><code>0xff</code> <code>0o17</code> <code>0b1010</code></td><td>정수. 범위는 10진 정수와 같은 64비트다.</td></tr>
    <tr><td>실수</td><td><code>3.14</code> <code>2.5e-3</code> <code>1e9</code></td><td>소수점 양쪽에 숫자가 필요하다(<code>.5</code>, <code>1.</code>은 안 됨). 지수가 있으면 실수.</td></tr>
  </tbody>
</table>
<h3>문자열</h3>
<ul>
  <li><code>"…"</code>: 이스케이프와 보간 <code>${식}</code>. 이스케이프는 <code>\n</code> <code>\t</code> <code>\r</code> <code>\\</code> <code>\"</code> <code>\$</code> <code>\u{1F600}</code> <code>\xHH</code>뿐이고, 모르는 이스케이프(<code>"C:\data"</code>)는 컴파일 에러다. <code>\xHH</code>는 바이트 하나다(<code>"\xe9"</code>는 글자 <code>é</code>가 아니라 바이트 0xE9, 글자는 <code>"\u{e9}"</code>). 실제 줄바꿈은 넣을 수 없다.</li>
  <li><code>`…`</code>: 원시 문자열. 이스케이프도 보간도 없고, 여러 줄이 된다. 정규식과 Windows 경로에 쓴다.</li>
  <li>보간 안에는 어떤 식이든 온다: <code>"${m["k"]}"</code>.</li>
  <li>길이와 인덱스는 글자(유니코드 코드 포인트) 단위다. <code>len("참치") == 2</code>, <code>"참치"[0] == "참"</code>. 따로 문자 타입은 없다.</li>
  <li>문자열은 바이트 열이고, 글자 연산이 그것을 UTF-8로 읽는다. UTF-8에 속하지 않는 바이트는 그 하나가 한 글자이고, 값으로 보일 때는 <code>"\xff"</code>처럼 보인다(<a href="../prelude/strings.html#bytes">바이트</a>). 소스 파일은 UTF-8이어야 한다(<a href="../errors.html#E0312">E0312</a>).</li>
</ul>

```fung run
name = "fung"
print("hello, ${name}! ${1 + 2}")
pattern = `^\d{4}-\d{2}$`
```

<h3>심볼</h3>
<p><code>'ok</code>, <code>'not_found</code>, <code>'Red</code>. <code>'</code> 뒤에 식별자가 온다(대문자로 시작해도 된다). 이름만 있는 값으로, 선언 없이 쓰는 열거형이다. 문자열과는 다른 타입이라 <code>'ok == "ok"</code>처럼 심볼과 문자열을 비교하면 런타임 에러 E0517이다(<code>sym(s)</code>로 바꿔 비교한다).</p>
<h3>기간</h3>
<p><code>500ms</code> <code>30s</code> <code>1.5s</code> <code>1h30m</code>. 단위는 <code>ms</code> <code>s</code> <code>m</code> <code>h</code>이고 큰 단위부터 쓴다. 숫자 바로 뒤에 붙여야 한다(<code>30 s</code>는 숫자와 이름 <code>s</code>).</p>
<p>기간을 숫자로 바꿀 때는 원하는 단위로 나눈다. <code>d / 1ms</code>는 ms 수(실수), <code>d // 1m</code>은 몇 분(정수), <code>d % 1m</code>은 분으로 나눈 나머지(기간)다. 숫자를 기간으로는 <code>n * 1ms</code>.</p>

```fung run
elapsed = 1m30s
print(elapsed / 1ms, elapsed // 1m, elapsed % 1m)
```

## 경계 사례와 에러 {#edges}

숫자의 밑줄은 숫자 사이에만 온다. 잘못된 모양은 그 자리에서 멈춘다.

```fung run exit=2 error=E0102
print(1_000_000)
print(1__0)
```

기간은 큰 단위부터 쓴다. 작은 단위 뒤에 큰 단위가 오면 기간이 아니다.

```fung run exit=2 error=E0105
print(1h30m, 90s)
print(30m1h)
```

문자열의 이스케이프는 정해진 것뿐이다(`\n`, `\t`, `\\`, `\"`, `\$`, `\u{…}`, `\xHH`).

```fung run exit=2 error=E0103
print("a\tb")
print("\q")
```
