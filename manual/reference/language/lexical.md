---
description: 토큰의 규칙: 이름, 숫자, 문자열, 기간, 주석, 예약어.
lede: 토큰의 규칙: 이름, 숫자, 문자열, 기간, 주석, 예약어.
---
# 어휘

<h3>주석</h3>
<p><code>#</code>부터 줄 끝까지. 다른 주석 형식은 없다(<code>//</code>는 정수 나눗셈이다). 파일 첫 줄의 <code>#!/usr/bin/env fung</code>도 그냥 주석이다.</p>
<p>파일 맨 앞의 BOM(U+FEFF) 하나는 건너뛴다. 그 밖의 BOM은 문자열(원시 문자열 포함)과 주석 안에서는 다른 글자처럼 그대로 받고, 토큰 사이(둘째 BOM 포함)에 오면 E0101이다.</p>
<p>줄 끝은 <code>\n</code>이나 <code>\r\n</code>이다. 홀로 선 CR(<code>\r</code>)은 문자열 안이 아니면 주석 안에서도 E0101이다. 터미널과 편집기가 CR을 서로 다르게 그려서, 주석처럼 보이는 코드가 실행되는 일을 막는다. 같은 까닭으로 양방향 제어 문자(U+202A~U+202E, U+2066~U+2069)는 주석과 문자열(원시 문자열 포함) 안에서도 E0101이다. 글자가 읽히는 순서와 다르게 보이게 하기 때문이다. 문자열에 그 글자가 필요하면 <code>"\u{202E}"</code>처럼 이스케이프로 쓴다.</p>
<h3>이름과 태그</h3>
<ul>
  <li>대문자로 시작하지 않으면 <strong>이름</strong>이다. 소문자, <code>_</code>, 한글처럼 대소문자가 없는 글자, 제목 문자 <code>ǅ</code>(Lt), 글자 숫자 <code>Ⅷ</code>(Nl)이 모두 그렇다: <code>file_list</code>, <code>_cache</code>, <code>파일목록</code>.</li>
  <li>첫 글자가 유니코드 대문자(일반 범주 Lu)면 <strong>태그</strong>: <code>Ok</code>, <code>Circle</code>. 선언 없이 쓴다(<a href="values.html">값과 타입</a>). 그래서 대문자 이름은 변수가 될 수 없다: <code>MAX_RETRIES = 3</code>과 <code>{HOME} = cfg</code>는 컴파일 에러 E0225다. 상수도 소문자로 쓰고(<code>max_retries = 3</code>), 대문자 키는 <code>{HOME: home} = cfg</code>나 <code>cfg.HOME</code>으로 읽는다.</li>
  <li>이름 표기는 snake_case. <code>_</code>로 시작하는 최상위 이름은 모듈 밖에서 보이지 않는다.</li>
  <li>이어지는 글자는 글자·숫자·<code>_</code>와 결합 문자(유니코드 UAX #31의 식별자 규칙): <code>हिंदी</code>, <code>café</code>. 이름은 NFC로 정규화해서, 조합형과 분해형으로 쓴 <code>é</code>는 같은 이름이다. 보이지 않는 글자(폭 없는 이음자 U+200C·U+200D, 변형 선택자, 채움 문자 등 유니코드의 Default_Ignorable_Code_Point)는 이름과 심볼에 들어가지 않는다(E0101): 눈으로 같은 두 이름이 다른 변수가 되기 때문이다. 맵 키 자리(<code>m.k</code>, <code>{k: 1}</code>, 축약 <code>{k}</code>, 패턴 <code>{k: v}</code>)의 식별자도 같은 NFC 이름이라, 분해형으로 쓴 <code>{café: 1}</code>과 조합형 <code>m.café</code>는 같은 키다(<code>m.k</code>는 NFC로 읽은 <code>m["k"]</code>). 따옴표로 쓴 키 <code>{"café": 1}</code>는 문자열이라 쓴 그대로이고, 그래서 NFC가 아닌 키는 값으로 찍을 때 따옴표를 붙여 보인다(맨 이름으로 쓰면 다시 읽을 때 NFC 키가 된다).</li>
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
    <tr><td>진법</td><td><code>0xff</code> <code>0o17</code> <code>0b1010</code></td><td>정수. 범위는 10진 정수와 같은 64비트다(<code>-0x8000000000000000</code>까지). 16진 숫자는 대소문자를 가리지 않는다(<code>0xFF</code>, <code>0xCafe</code>; 찍을 때는 10진). 접두사는 문법 글자라 소문자만이다(<code>0XFF</code>는 E0102). 그 진법에 없는 숫자도 E0102다(<code>0b12</code>). 모양은 맞는데 64비트에 담기지 않는 정수는 진법과 상관없이 E0135다(<code>9223372036854775808</code>, <code>0xffffffffffffffffff</code>).</td></tr>
    <tr><td>실수</td><td><code>3.14</code> <code>2.5e-3</code> <code>1e9</code></td><td>소수점 양쪽에 숫자가 필요하다(<code>.5</code>, <code>1.</code>은 안 됨, E0102). 정수 바로 뒤의 <code>.</code>는 소수부나 범위 <code>..</code>만 될 수 있다(<code>1.foo</code>도 E0102). 지수가 있으면 실수이고 지수는 소문자 <code>e</code>만(<code>1E5</code>는 E0102). 0이 아닌 수가 0.0으로 아래 넘치거나(<code>1e-400</code>) 실수의 범위를 넘으면(<code>1e400</code>) E0135.</td></tr>
  </tbody>
</table>
<h3>문자열</h3>
<ul>
  <li><code>"…"</code>: 이스케이프와 보간 <code>${식}</code>. 이스케이프는 <code>\n</code> <code>\t</code> <code>\r</code> <code>\\</code> <code>\"</code> <code>\$</code> <code>\u{1F600}</code> <code>\xHH</code>뿐이고, 모르는 이스케이프(<code>"C:\data"</code>)는 컴파일 에러다. <code>\xHH</code>는 16진 숫자 꼭 둘, <code>\u{…}</code>는 중괄호 안에 16진 숫자 1~6개다(모양이 틀리면 E0103). 16진 숫자는 숫자 리터럴처럼 대소문자를 가리지 않고(<code>\xFF</code>, <code>\u{1F600}</code>), 이스케이프 글자 <code>\x</code>·<code>\u</code>는 소문자만이다. <code>\xHH</code>는 바이트 하나다(<code>"\xe9"</code>는 글자 <code>é</code>가 아니라 바이트 0xE9, 글자는 <code>"\u{e9}"</code>). 실제 줄바꿈은 넣을 수 없다.</li>
  <li><code>$</code> 바로 뒤에 이름의 첫 글자가 오면 컴파일 에러 E0129다: <code>"$name"</code>은 보간처럼 보이지만 보간이 아니다. 값은 <code>"${name}"</code>, 글자 그대로는 <code>"\$name"</code>으로 쓴다. 그 밖의 <code>$</code>(<code>"$5"</code>, 끝의 <code>$</code>)는 글자다. 셸 명령처럼 <code>$</code>가 많은 글은 원시 문자열로 쓴다.</li>
  <li><code>`…`</code>: 원시 문자열. 이스케이프도 보간도 없고, 여러 줄이 된다. 정규식과 Windows 경로에 쓴다. 안의 줄 끝은 파일이 <code>\r\n</code>으로 끝나도 값에서는 <code>\n</code>이다. 보간 없는 문자열이 필요한 자리(문자열 패턴, 맵 키와 맵 패턴의 키, <code>import</code> 경로, <code>test</code> 이름)에는 원시 문자열도 같은 것으로 온다.</li>
  <li>보간 안에는 어떤 식이든 온다: <code>"${m["k"]}"</code>.</li>
  <li>길이와 인덱스는 글자(유니코드 코드 포인트) 단위다. <code>len("참치") == 2</code>, <code>"참치"[0] == "참"</code>. 따로 문자 타입은 없다.</li>
  <li>문자열은 바이트 열이고, 글자 연산이 그것을 UTF-8로 읽는다. UTF-8에 속하지 않는 바이트는 그 하나가 한 글자이고, 값으로 보일 때는 <code>"\xff"</code>처럼 보인다(<a href="../prelude/strings.html#bytes">바이트</a>). 소스 파일은 UTF-8이어야 한다(<a href="../errors.html#E0312">E0312</a>).</li>
</ul>

```fung run
name = "fung"
print("hello, ${name}! ${1 + 2}")
pattern = `^\d{4}-\d{2}$`
print(pattern)
```

<h3>심볼</h3>
<p><code>'ok</code>, <code>'not_found</code>. <code>'</code> 뒤에 이름이 온다. 이름만 있는 값으로, 선언 없이 쓰는 열거형이다. 대문자로 시작하는 심볼은 인자 없는 태그로 쓴다: <code>Red</code>가 그 심볼이고, <code>'Red</code>는 같은 값의 둘째 표기라 컴파일 에러 E0128이다. 찍을 때도 쓴 그대로 <code>Red</code>다(<code>print(Red)</code>, <code>print(sym("Red"))</code>). <code>Ok</code>·<code>Err</code>도 같다: 맨 <code>Ok</code>는 심볼이고, 값을 담은 <code>Ok(1)</code>만 결과값이다. 문자열과는 다른 타입이라 <code>'ok == "ok"</code>는 <code>false</code>다(바깥에서 온 문자열은 <code>sym(s)</code>로 바꿔 비교한다).</p>
<h3>기간</h3>
<p><code>500ms</code> <code>30s</code> <code>1.5s</code> <code>1h30m</code>. 단위는 <code>ms</code> <code>s</code> <code>m</code> <code>h</code>이고 큰 단위부터 쓴다. 숫자 바로 뒤에 붙여야 한다(<code>30 s</code>는 숫자와 이름 <code>s</code>).</p>
<p>성분마다 숫자는 10진 정수처럼 <code>0</code>으로 시작하지 않고(<code>1h05m</code>은 E0105), 소수점은 마지막 성분에만 온다(<code>1h30.5m</code>은 되고 <code>1.5h30m</code>은 E0105). 기간은 나노초 단위라, 0이 아닌 성분이 0 ns로 반올림되면 E0105다(<code>0.0000004ms</code>). 모양은 맞는데 기간의 범위(약 292년)를 넘으면 정수처럼 E0135다(<code>9999999999h</code>). 성분의 크기에는 제한이 없다: <code>1h90m</code>과 <code>1m0s</code>도 되고 각각 <code>2h30m</code>, <code>1m</code>과 같은 값이다.</p>
<p>기간을 숫자로 바꿀 때는 원하는 단위로 나눈다. <code>d / 1ms</code>는 ms 수(실수), <code>d // 1m</code>은 몇 분(정수), <code>d % 1m</code>은 분으로 나눈 나머지(기간)다. 숫자를 기간으로는 <code>n * 1ms</code>. 기간을 수로 나누는 것은 <code>/</code>뿐이다(<code>1s / 2</code>는 <code>500ms</code>이고, 나눈 결과가 이미 나노초로 반올림되므로 <code>1s // 2</code>와 <code>1s % 2</code>는 E0501이다).</p>

```fung run
elapsed = 1m30s
print(elapsed / 1ms, elapsed // 1m, elapsed % 1m)
```

<p>기간의 산술은 정확히 계산하고 결과를 나노초로 한 번 반올림한다(반은 0에서 먼 쪽). 기간을 기간으로 나누는 <code>d / e</code>도 나노초 정수로 정확히 나눈 뒤 한 번 반올림한 실수이고, 피연산자를 실수로 바꾸지 않으므로 E0523이 없다. 수로 곱하거나 나눌 때 그 수가 실수여도 기간을 실수로 바꾸지 않고 실수의 정확한 값으로 계산하므로, 아주 긴 기간도 깎이지 않고 E0523이 없다. 결과가 기간의 범위를 넘으면 E0503이다.</p>

```fung run
ns = 1ms / 1000000
print(1s / 3, 90m * 1.5, (2600h + ns) * 1.0, ns * 0.5, 1s * 0.0000000001)
```

## 경계 사례와 에러 {#edges}

제목 문자(Lt)나 글자 숫자(Nl)로 시작하는 이름은 태그가 아니라 변수가 된다.

```fung run
ǅx = 1
Ⅷ = 2
print(ǅx + Ⅷ, type_of(Red))
```

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

진법의 접두사와 지수는 문법 글자라 소문자로 쓴다. 대문자는 도움말이 소문자 모양을 보인다. 16진 숫자는 대소문자를 가리지 않는다.

```fung run exit=2 error=E0102
print(0xff, 0xFF, 1e5)
print(0XFF)
```

기간의 소수점은 마지막 성분에만 온다.

```fung run exit=2 error=E0105
print(1h30.5m)
print(1.5h30m)
```

`$` 뒤에 이름이 오면 보간이 아니라 에러다. 값은 `${…}`로, 글자는 `\$`로 쓴다.

```fung run exit=2 error=E0129
user = "ann"
print("${user} paid \$5, $5 and \$user")
print("hello $user")
```

대문자로 시작하는 심볼은 태그로 쓴다.

```fung run exit=2 error=E0128
print(Red == sym("Red"), 'red)
print('Red)
```

문자열의 이스케이프는 정해진 것뿐이다(`\n`, `\t`, `\r`, `\\`, `\"`, `\$`, `\u{…}`, `\xHH`).

```fung run exit=2 error=E0103
print("a\tb")
print("\q")
```
