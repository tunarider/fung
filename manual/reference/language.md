---
description: fung의 문법: 어휘, 블록과 줄 이어짐, 값과 연산자, 함수, 패턴 매칭, 결과값, 모듈, 형식 문법.
lede: 겉은 Go와 Python을 닮았고, 블록은 <code>:</code>에서 <code>;;</code>까지다. 들여쓰기에는 의미가 없다.
---
# 문법

```fung run
# 한눈에 보는 fung
import fs

fn total_size(dir):
  fs.list(dir)!
    |> map(f => fs.stat("${dir}/${f}")!.size)
    |> fold(0, (acc, n) => acc + n)
;;

size = total_size(".")
label = size > 1_000_000 then "big" else "small"
print("${size} bytes (${label})")
```

## 어휘 {#lexical}

<h3>주석</h3>
<p><code>#</code>부터 줄 끝까지. 다른 주석 형식은 없다(<code>//</code>는 정수 나눗셈이다). 파일 첫 줄의 <code>#!/usr/bin/env fung</code>도 그냥 주석이다.</p>
<h3>이름과 태그</h3>
<ul>
  <li>소문자, <code>_</code>, 또는 대소문자가 없는 글자(한글 등)로 시작하면 <strong>이름</strong>: <code>file_list</code>, <code>_cache</code>, <code>파일목록</code>.</li>
  <li>대문자로 시작하면 <strong>태그</strong>: <code>Ok</code>, <code>Circle</code>. 선언 없이 쓴다(<a href="#values">값과 타입</a>). 그래서 대문자 이름은 변수가 될 수 없다: <code>MAX_RETRIES = 3</code>과 <code>{HOME} = cfg</code>는 컴파일 에러 E0225다. 상수도 소문자로 쓰고(<code>max_retries = 3</code>), 대문자 키는 <code>{HOME: home} = cfg</code>나 <code>cfg.HOME</code>으로 읽는다.</li>
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
    <tr><td>정수</td><td><code>42</code> <code>1_000_000</code></td><td>64비트. <code>_</code>는 숫자 사이에만.</td></tr>
    <tr><td>진법</td><td><code>0xff</code> <code>0o17</code> <code>0b1010</code></td><td>정수.</td></tr>
    <tr><td>실수</td><td><code>3.14</code> <code>2.5e-3</code> <code>1e9</code></td><td>소수점 양쪽에 숫자가 필요하다(<code>.5</code>, <code>1.</code>은 안 됨). 지수가 있으면 실수.</td></tr>
  </tbody>
</table>
<h3>문자열</h3>
<ul>
  <li><code>"…"</code>: 이스케이프와 보간 <code>${식}</code>. 이스케이프는 <code>\n</code> <code>\t</code> <code>\r</code> <code>\\</code> <code>\"</code> <code>\$</code> <code>\u{1F600}</code>뿐이고, 모르는 이스케이프(<code>"C:\data"</code>)는 컴파일 에러다. 실제 줄바꿈은 넣을 수 없다.</li>
  <li><code>`…`</code>: 원시 문자열. 이스케이프도 보간도 없고, 여러 줄이 된다. 정규식과 Windows 경로에 쓴다.</li>
  <li>보간 안에는 어떤 식이든 온다: <code>"${m["k"]}"</code>.</li>
  <li>길이와 인덱스는 글자(유니코드 코드 포인트) 단위다. <code>len("참치") == 2</code>, <code>"참치"[0] == "참"</code>. 따로 문자 타입은 없다.</li>
</ul>

```fung run
name = "fung"
print("hello, ${name}! ${1 + 2}")
pattern = `^\d{4}-\d{2}$`
```

<h3>심볼</h3>
<p><code>'ok</code>, <code>'not_found</code>. 이름만 있는 값으로, 선언 없이 쓰는 열거형이다. 문자열과는 다른 타입이라 <code>'ok == "ok"</code>처럼 심볼과 문자열을 비교하면 런타임 에러 E0517이다(<code>sym(s)</code>로 바꿔 비교한다).</p>
<h3>기간</h3>
<p><code>500ms</code> <code>30s</code> <code>1.5s</code> <code>1h30m</code>. 단위는 <code>ms</code> <code>s</code> <code>m</code> <code>h</code>이고 큰 단위부터 쓴다. 숫자 바로 뒤에 붙여야 한다(<code>30 s</code>는 숫자와 이름 <code>s</code>).</p>
<p>기간을 숫자로 바꿀 때는 원하는 단위로 나눈다. <code>d / 1ms</code>는 ms 수(실수), <code>d // 1m</code>은 몇 분(정수), <code>d % 1m</code>은 분으로 나눈 나머지(기간)다. 숫자를 기간으로는 <code>n * 1ms</code>.</p>

```fung run
elapsed = 1m30s
print(elapsed / 1ms, elapsed // 1m, elapsed % 1m)
```

## 문장과 블록 {#blocks}

<ul>
  <li>문장은 줄바꿈이나 <code>;</code>로 나눈다. 빈 문장은 괜찮다.</li>
  <li>블록은 <code>:</code>로 열고 <code>;;</code>로 닫는다. 블록의 값은 마지막 식의 값이다.</li>
  <li>값을 아무도 쓰지 않는 자리(마지막이 아닌 식, 그런 자리의 <code>if</code>·<code>match</code> 가지의 끝, 파일의 마지막 식)의 값은 버려진다. 버려지는 값이 <code>Err</code>면 E0606이다(<a href="#results">결과값과 에러</a>).</li>
  <li>들여쓰기는 의미가 없다. 같은 코드를 한 줄로도 쓸 수 있다.</li>
</ul>

```fung run
fn classify(n):
  if n < 0:
    "negative"
  else if n == 0:
    "zero"
  else:
    "positive"
  ;;
;;

fn classify_short(n) = if n < 0: "negative" else if n == 0: "zero" else: "positive" ;;
```

<p><code>;;</code>는 렉서가 항상 가장 길게 자른다. <code>a;;</code>는 <code>a</code>와 블록 끝이다.</p>

### 줄 이어짐 {#continuation}

<p>줄바꿈은 문장 끝이지만, 다음 셋 중 하나면 다음 줄로 이어진다.</p>
<ol>
  <li>줄이 식을 끝낼 수 없는 토큰으로 끝날 때: 이항 연산자(<code>+</code> <code>-</code> <code>*</code> <code>/</code> <code>//</code> <code>%</code> <code>**</code> <code>==</code> <code>!=</code> <code>&lt;</code> <code>&gt;</code> <code>&lt;=</code> <code>&gt;=</code> <code>in</code> <code>??</code> <code>|&gt;</code> <code>..</code> <code>..=</code> <code>and</code> <code>or</code>), <code>not</code>, <code>,</code>, <code>=</code>, <code>=&gt;</code>, <code>-&gt;</code>, <code>.</code>, 여는 괄호, 블록 시작 <code>:</code>.</li>
  <li>다음 줄이 이어짐 전용 토큰으로 시작할 때: <code>|&gt;</code> <code>.</code> <code>??</code> <code>and</code> <code>or</code> <code>then</code> <code>else</code>. 이 토큰들로는 문장을 시작할 수 없으니 모호하지 않다.</li>
  <li>괄호 <code>()</code> <code>[]</code> <code>{}</code> 안: 줄바꿈은 공백이다. 단, 괄호 안에서 <code>fn(x):</code>처럼 블록이 열리면 그 <code>;;</code>까지는 줄바꿈이 다시 문장 구분자다.</li>
</ol>

```fung run
names = ["b.log", "a.txt", "c.log"]
logs = names
  |> filter(n => ends_with(n, ".log"))
  |> sort
ok = len(logs) > 0
  and all(logs, n => n != "")
print(logs, ok)
```

## 값과 타입 {#values}

<table>
  <thead><tr><th>타입</th><th>예</th><th>설명</th></tr></thead>
  <tbody>
    <tr><td><code>int</code></td><td><code>42</code></td><td>64비트 정수. 넘치면 에러.</td></tr>
    <tr><td><code>float</code></td><td><code>3.14</code></td><td>64비트 실수. <code>NaN</code>·<code>Inf</code>가 되면 에러.</td></tr>
    <tr><td><code>str</code></td><td><code>"a"</code></td><td>불변 UTF-8, 글자 단위 인덱스.</td></tr>
    <tr><td><code>bool</code></td><td><code>true</code></td><td>조건 자리에는 bool만 온다.</td></tr>
    <tr><td><code>nil</code></td><td><code>nil</code></td><td>"없음"이라는 정상 값. <code>==</code>, <code>??</code>, 출력, 컬렉션에 담기 외의 연산은 에러.</td></tr>
    <tr><td><code>sym</code></td><td><code>'ok</code></td><td>이름만 있는 값.</td></tr>
    <tr><td><code>dur</code></td><td><code>1h30m</code></td><td>기간. 기간끼리 <code>+ -</code>와 <code>/ // %</code>, 숫자와 <code>* /</code>, 순서 비교.</td></tr>
    <tr><td><code>time</code></td><td><code>time.now()</code></td><td>시각. 리터럴 없음. <code>± dur</code>, <code>time - time</code>은 기간, 순서 비교. 출력은 ISO 8601.</td></tr>
    <tr><td><code>list</code></td><td><code>[1, 2]</code></td><td>순서 있는 목록.</td></tr>
    <tr><td><code>map</code></td><td><code>{a: 1}</code></td><td>키는 항상 문자열. 넣은 순서를 지킨다.</td></tr>
    <tr><td><code>tuple</code></td><td><code>(1, "a")</code></td><td>고정 개수. 원소는 패턴으로만 꺼낸다. 하나짜리는 <code>(x,)</code>.</td></tr>
    <tr><td>태그</td><td><code>Ok(1)</code> <code>Circle(3)</code></td><td>이름 + 인자. 타입 이름은 태그 이름 자체.</td></tr>
    <tr><td><code>range</code></td><td><code>0..10</code> <code>0..=10</code></td><td>게으른 정수 범위. 리스트를 미리 만들지 않는다.</td></tr>
    <tr><td><code>fn</code></td><td><code>x =&gt; x</code></td><td>함수. 내장 함수도 같은 타입.</td></tr>
    <tr><td><code>module</code></td><td><code>fs</code></td><td>모듈의 공개 이름들로 된 불변 레코드.</td></tr>
    <tr><td><code>file</code></td><td><code>fs.open(p)!</code></td><td>열린 파일 핸들. 상태가 있는 자원.</td></tr>
    <tr><td><code>task</code> <code>scope</code> <code>sender</code> <code>receiver</code> <code>atom</code></td><td><code>scope(s =&gt; spawn(s, f))</code>, <code>(tx, rx) = channel()</code></td><td>동시성 값(<a href="prelude/concurrency.html">내장 함수</a>). 채널은 보내는 끝과 받는 끝 두 값으로 다룬다. atom은 값을 보이지 않고 <code>&lt;atom&gt;</code>으로 찍힌다(<code>deref</code>로 읽는다).</td></tr>
  </tbody>
</table>
<p><code>type_of(x)</code>가 위 타입 이름을 심볼로 돌려준다: <code>type_of(1) == 'int</code>, <code>type_of(Circle(3)) == 'Circle</code>, <code>type_of('red) == 'sym</code>.</p>
<h3>맵</h3>
<ul>
  <li><code>{name: "a"}</code>는 <code>{"name": "a"}</code>의 축약이다. 식별자 모양이면 대소문자와 상관없이 축약된다(<code>{DB_HOST: h}</code>).</li>
  <li>값이 같은 이름의 변수면 키만 쓴다: <code>{name, port}</code>는 <code>{name: name, port: port}</code>. 대문자 키는 그런 변수가 없으므로 축약할 수 없다(<code>{HOME}</code>은 E0225).</li>
  <li>같은 키를 두 번 쓰면 컴파일 에러. 없는 키를 읽으면 런타임 에러이고, 기본값이 필요하면 <code>get(m, k, d)</code>를 쓴다.</li>
  <li>수정된 사본은 함수로 만든다: <code>put</code>, <code>merge</code>, <code>remove</code>.</li>
</ul>
<h3>비교와 동일성</h3>
<table>
  <thead><tr><th>타입</th><th><code>==</code></th><th><code>&lt;</code> 등</th></tr></thead>
  <tbody>
    <tr><td>int, float</td><td>값. 섞여도 숫자로 비교(<code>1 == 1.0</code>), 2<sup>53</sup>을 넘는 정수도 정확히</td><td>값</td></tr>
    <tr><td>str</td><td>내용</td><td>코드 포인트 순</td></tr>
    <tr><td>dur, time</td><td>값(time은 같은 순간이면 시간대 무관)</td><td>값</td></tr>
    <tr><td>list, tuple</td><td>원소가 차례로 같음</td><td>앞 원소부터 사전식</td></tr>
    <tr><td>map</td><td>키 집합과 값이 같음(순서 무시)</td><td>에러</td></tr>
    <tr><td>bool, nil, sym, 태그, range</td><td>값</td><td>에러</td></tr>
    <tr><td>fn, module, file, task, scope, sender, receiver, atom</td><td>같은 객체인가</td><td>에러</td></tr>
    <tr><td>타입이 다를 때</td><td><code>false</code>(숫자끼리 제외). 단 심볼과 문자열은 에러 E0517</td><td>에러</td></tr>
  </tbody>
</table>
<p><code>sort</code>, <code>unique</code>, <code>contains</code>, <code>in</code>은 모두 이 표를 따른다.</p>
<p><strong>심볼과 문자열의 비교는 에러다.</strong> JSON·파일·명령 출력에서 온 값은 늘 문자열이라 <code>back.kind == 'not_found</code>가 조용히 거짓이 되는 것이 가장 흔한 함정이기 때문이다. 리스트·맵 안에서 맞붙어도, <code>in</code>도 같다. 문자열 쪽을 <code>sym(s)</code>로 바꾼다. 패턴(<code>'not_found =&gt;</code>)은 비교가 아니라서 그대로 맞지 않을 뿐이다.</p>

## 연산자 {#operators}

<h3>산술</h3>
<ul>
  <li><code>+ - *</code>, <code>/</code>는 항상 실수 결과(<code>7 / 2 == 3.5</code>), <code>//</code>는 내림 나눗셈(<code>7 // 2 == 3</code>), <code>%</code>는 <code>//</code>와 짝인 나머지(<code>-7 % 3 == 2</code>, 실수도 정확한 나머지라 나누는 수보다 작다), <code>**</code>는 거듭제곱(<code>2 ** 10 == 1024</code>, <code>2 ** -1 == 0.5</code>).</li>
  <li>정수와 실수를 섞으면 실수. 0으로 나누기, 정수 오버플로, <code>NaN</code>·<code>Inf</code>는 에러.</li>
  <li><code>+</code>는 문자열·리스트를 이어 붙인다. <code>"a" + 1</code>은 에러, 반복은 <code>repeat(s, n)</code>. 문자열은 1억 글자를 넘으면 E0806(보간도 같다).</li>
</ul>
<h3>비교·논리·포함</h3>
<ul>
  <li><code>==</code>와 <code>!=</code>는 값 비교. 비교는 연쇄하지 않는다: <code>a &lt; b &lt; c</code>는 문법 에러(<code>a &lt; b and b &lt; c</code>).</li>
  <li><code>and</code> <code>or</code> <code>not</code>은 bool만 받고 단락 평가한다. 빈 리스트나 0을 거짓으로 보지 않는다.</li>
  <li><code>x in c</code> / <code>x not in c</code>: 리스트는 원소, 맵은 키, 문자열은 부분 문자열, 범위는 정수 포함 여부.</li>
</ul>
<h3>범위</h3>
<p><code>a..b</code>는 <code>b</code>를 빼고, <code>a..=b</code>는 넣는다. 정수만 된다.</p>

### 파이프 {#pipe}

<p><code>|&gt;</code>는 실행 중의 연산이 아니라 컴파일할 때 평범한 호출로 바뀌는 문법 설탕이다. 규칙은 둘뿐이다.</p>
<ul>
  <li><code>x |&gt; f(a, b)</code>는 <code>f(x, a, b)</code>: 오른쪽이 호출이면 왼쪽 값을 <strong>맨 앞 인자로</strong> 넣는다.</li>
  <li><code>x |&gt; f</code>는 <code>f(x)</code>: 오른쪽이 이름뿐이면 그것으로 부른다.</li>
</ul>
<p>태그도 같다. <code>x |&gt; Ok</code>는 <code>Ok(x)</code>, <code>x |&gt; Pair(2)</code>는 <code>Pair(x, 2)</code>다(태그의 <code>Ok(x)</code>는 호출 형태이므로).</p>
<p>이어 쓰면 안쪽부터 감싸는 호출이 된다. <code>xs |&gt; filter(f) |&gt; map(g)</code>는 <code>map(filter(xs, f), g)</code>와 바이트코드까지 같다. 비용은 없고, 데이터가 흐르는 순서대로 읽힌다는 것이 전부다. 이름 인자도 그대로 따라간다: <code>xs |&gt; conc.pmap(f, limit: 2)</code>는 <code>conc.pmap(xs, f, limit: 2)</code>.</p>
<p>그래서 내장 함수는 모두 <strong>다루는 대상을 첫 번째 매개변수</strong>에 둔다: 리스트 함수는 리스트가(<code>filter(xs, f)</code>), 문자열 함수는 문자열이(<code>split(s, sep)</code>), 맵 함수는 맵이(<code>put(m, k, v)</code>) 먼저다. Python의 <code>map(f, xs)</code>나 데이터를 마지막에 두는 Haskell·OCaml과 달리 Elixir와 같은 쪽이다. 직접 만드는 함수도 이렇게 두면 파이프에 그대로 이어진다.</p>

```fung run
line = " ERROR,disk full "
fields = line |> trim |> lower |> split(",")
cfg = {host: "localhost"} |> put("port", 8080) |> merge({debug: false})
print(fields, cfg |> keys)
```

<p>대상이 첫 자리가 아닌 함수에는 람다로 넘긴다: <code>5 |&gt; (n =&gt; scale(2, n))</code>.</p>
<div class="aside">
<p><strong>파이프 오른쪽 호출의 인자에 <code>_</code>를 쓰면 컴파일 에러 E0227이다.</strong> 파이프 값은 늘 첫 인자로 들어가므로 <code>"a.txt" |&gt; path.join("/tmp", _)</code>는 <code>path.join("a.txt", "/tmp", _)</code>, 곧 함수가 되어 버리던 함정이다. 다른 자리에 넣으려면 람다를 쓴다. 더 안쪽 호출의 <code>_</code>는 그대로 부분 적용이다.</p>
<p><strong>파이프 오른쪽 끝의 후위 <code>?</code>·<code>!</code>도 컴파일 에러 E0229다.</strong> 후위 연산자가 <code>|&gt;</code>보다 먼저 묶여 <code>xs |&gt; try_map(f)?</code>는 <code>try_map(f)?</code>를 파이프에 넘기는 것이 되기 때문이다. 파이프 전체의 결과를 꺼내려면 <code>(xs |&gt; try_map(f))?</code>로 감싼다.</p>
</div>

```fung frag
"a.txt" |> path.join("/tmp", _)            # E0227
"a.txt" |> (f => path.join("/tmp", f))     # "/tmp/a.txt"
files |> map(path.join("/tmp", _))         # _의 올바른 쓰임: map에 넘길 함수를 만든다
```

### 우선순위 {#precedence}

<p>낮은 것부터 높은 것 순서.</p>
<table>
  <thead><tr><th>순위</th><th>연산자</th></tr></thead>
  <tbody>
    <tr><td>1</td><td><code>then … else</code></td></tr>
    <tr><td>2</td><td><code>or</code></td></tr>
    <tr><td>3</td><td><code>and</code></td></tr>
    <tr><td>4</td><td><code>not</code></td></tr>
    <tr><td>5</td><td><code>==</code> <code>!=</code> <code>&lt;</code> <code>&gt;</code> <code>&lt;=</code> <code>&gt;=</code> <code>in</code> <code>not in</code></td></tr>
    <tr><td>6</td><td><code>|&gt;</code></td></tr>
    <tr><td>7</td><td><code>??</code></td></tr>
    <tr><td>8</td><td><code>..</code> <code>..=</code></td></tr>
    <tr><td>9</td><td><code>+</code> <code>-</code></td></tr>
    <tr><td>10</td><td><code>*</code> <code>/</code> <code>//</code> <code>%</code></td></tr>
    <tr><td>11</td><td>단항 <code>-</code></td></tr>
    <tr><td>12</td><td><code>**</code>(오른쪽 결합)</td></tr>
    <tr><td>13</td><td>후위 <code>!</code> <code>?</code>, 호출 <code>()</code>, 인덱스 <code>[]</code>, 필드 <code>.</code></td></tr>
  </tbody>
</table>

```fung frag
name |> ends_with(".log") and size > 0    # (name |> ends_with(".log")) and (size > 0)
files |> len > 3                          # (files |> len) > 3
fs.read(p) ?? "{}" |> json.parse          # (fs.read(p) ?? "{}") |> json.parse
-2 ** 2                                   # -(2 ** 2) == -4
```

<p><code>not</code>은 비교보다 낮으므로 그보다 높은 연산자의 피연산자가 될 수 없다. <code>a == not b</code>, <code>1 + not c</code>는 에러이고 괄호로 <code>a == (not b)</code>라고 쓴다. <code>not a == b</code>는 <code>not (a == b)</code>다.</p>

## 인덱스·슬라이스·필드 {#access}

<ul>
  <li><code>xs[i]</code>: 음수는 뒤에서부터(<code>xs[-1]</code>). 범위를 벗어나면 에러.</li>
  <li><code>xs[a..b]</code>, <code>xs[..b]</code>, <code>xs[a..]</code>, <code>xs[a..=b]</code>: 리스트와 문자열의 슬라이스. 범위를 벗어나거나 시작이 끝보다 뒤면 에러이고, 있는 만큼만 원하면 <code>take</code>·<code>drop</code>을 쓴다(리스트와 문자열 모두). 열린 끝은 비워 쓴다: 끝에 <code>nil</code>을 주면 에러다.</li>
  <li><code>m.name</code>은 <code>m["name"]</code>과 같다. 모듈의 <code>fs.read</code>도 같은 규칙이다.</li>
  <li>튜플은 인덱스로 꺼내지 않는다. 패턴으로 꺼낸다: <code>(a, b) = pair</code>.</li>
</ul>

```fung run
xs = [10, 20, 30, 40]
print(xs[-1], xs[1..3], xs[..2], "참치김밥"[1..3])
```

## 변수와 스코프 {#scope}

<p>대입 <code>x = 식</code>은 이름을 값에 붙인다. 같은 이름에 다시 대입할 수 있지만 값 자체를 바꾸지는 않는다. 규칙은 넷이고 예외는 없다.</p>
<ol>
  <li><strong>이름은 위에서 아래로 묶인다.</strong> 대입보다 앞에서 읽으면 컴파일 에러(E0204).</li>
  <li><strong>최상위 <code>fn</code>은 프로그램 시작 전에 모두 만들어진다.</strong> 그래서 순서와 상관없이 서로 부를 수 있다.</li>
  <li><strong>함수 값은 만들어질 때 보이는 변수의 값을 복사한다.</strong> 나중에 그 이름에 다시 대입해도 함수가 가진 값은 그대로다.</li>
  <li><strong>블록은 바깥 이름을 읽을 수 있지만 대입할 수 없다</strong>(E0205). 블록 안에서 만든 이름은 밖으로 새지 않는다(E0206).</li>
</ol>
<p>여기서 따라 나오는 것: 최상위 <code>fn</code>은 만들어질 때 최상위 변수가 아직 없으므로 복사할 것이 없다. 대신 <strong>최상위에서 한 번만 대입되는 이름은 상수라서 최상위 <code>fn</code>이 부를 때 읽는다</strong>(다른 최상위 <code>fn</code>과 모듈이 보이는 것과 같은 이유다). 모듈 상태는 <code>counter = atom(0)</code>처럼 둔다. 두 번 이상 대입되는 이름은 보이지 않으니 인자로 넘긴다. 최상위 <code>fn</code>이 자기 본문에서 묶는 이름은 그 함수의 지역이고 같은 이름의 상수를 가린다. 파일이 시작하는 중에 상수의 대입보다 먼저 그것을 읽는 <code>fn</code>을 부르면 E0809다. 람다와 익명 함수는 그 자리에서 만들어지므로 그때까지의 변수를 복사한다.</p>

```fung frag
dir = "./logs"
fn clean() = fs.list(dir)       # 된다: dir은 한 번만 대입되는 상수
level = 1
level = 2
fn show() = level               # E0204: level은 두 번 대입된다. 인자로 넘긴다

total = 0
for f in files:
  total = total + 1             # E0205: 블록 안에서 바깥 변수에 대입할 수 없다
;;
total = len(files)              # 값을 식으로 만든다
```

<p><strong>값의 수명.</strong> 함수 안의 변수를 마지막으로 읽으면 값이 읽은 쪽으로 넘어가고, 그 문장이 끝나면 변수는 값을 놓는다(람다가 붙잡는 것도 읽기다). 그래서 홀로 쥔 맵·리스트를 받은 <code>put</code>·<code>+</code>는 복사하지 않고 제자리에서 고친다(<code>fold</code>로 쌓는 맵이 빠른 이유). 그래서 함수 안에서 만든 채널의 보내는 끝을 생산 태스크에 넘기면, 생산이 끝날 때 채널이 닫힌다. 한 번도 읽지 않는 변수(<code>_watch = stop_signal()</code>)는 함수가 돌아올 때까지 산다. 쥐고 있는 것 자체가 뜻일 때 쓴다. 최상위 변수는 프로그램 끝까지 산다.</p>

## 함수 {#functions}

```fung run
# 식 하나짜리
fn add(a, b) = a + b

# 블록 본문: 마지막 식이 반환값
fn report(dir):
  n = len(dir)
  "${dir}: ${n}"
;;

# 람다: 본문은 식 하나
double = x => x * 2
both = (a, b) => a + b
later = () => print("run")

# 익명 함수: 여러 문장이 필요할 때
shout = fn(s):
  t = upper(s)
  t + "!"
;;

print(add(1, 2), report("logs"), double(4), both(1, 2), shout("hi"))
later()
```

<h3>인자</h3>
<ul>
  <li>순서는 필수 → 기본값 있는 것 → 마지막에 <code>...rest</code> 하나(남는 위치 인자를 리스트로 받음).</li>
  <li>기본값은 호출할 때마다 왼쪽부터 평가되고, 앞 인자를 참조할 수 있다. 기본값과 <code>...rest</code>는 <code>fn</code>과 익명 <code>fn(…):</code>에서 쓴다(람다 <code>=&gt;</code>에는 없다).</li>
  <li>호출할 때 위치 인자 뒤에 이름 붙인 인자를 쓴다: <code>exec(cmd, cwd: dir)</code>. 내장 함수도 똑같다.</li>
  <li>인자 개수가 맞지 않거나, 같은 인자를 두 번 주거나, 모르는 이름이면 런타임 에러.</li>
  <li>매개변수 자리에 패턴을 쓸 수 있다: <code>fn dist((x, y)) = …</code>, <code>pairs(m) |&gt; map(((k, v)) =&gt; k)</code>.</li>
</ul>

```fung run
fn greet(name, greeting = "hello", ...extra) = "${greeting}, ${name}${join(extra)}"
print(greet("fung"), greet("fung", greeting: "hi"), greet("fung", "yo", "!", "?"))
```

<h3>반환</h3>
<p>마지막 식의 값을 돌려준다. 일찍 빠져나갈 때는 <code>return 식</code>(값 없으면 <code>nil</code>). <code>return</code>은 감싸는 함수나 람다에서 돌아오고, <code>for</code> 본문과 진입 파일 최상위에서는 쓸 수 없다(최상위에서는 <code>exit()</code>).</p>
<h3>부분 적용</h3>
<p>호출 인자의 <code>_</code>는 그 자리를 비운 새 함수를 만든다. <code>f(a, _)</code>는 <code>x =&gt; f(a, x)</code>이고, <code>a</code>는 그 자리에서 한 번 평가된다. 자동 커링은 없다. 파이프 오른쪽 호출의 인자로는 쓸 수 없다(E0227, <a href="#pipe">파이프</a>).</p>

```fung run
import path
files = ["a.txt", "b.txt"]
print(files |> map(path.join("/tmp", _)))
```

<h3>꼬리 호출</h3>
<p>함수 본문의 마지막 식, 꼬리 위치에 있는 <code>if</code>·<code>match</code>·<code>then … else</code>의 각 가지, 꼬리 위치의 <code>x |&gt; f(a)</code>에 있는 호출은 프레임을 재사용한다. 상태가 있는 반복은 이렇게 쓴다. 자기 재귀뿐 아니라 모든 꼬리 호출이 대상이다.</p>

```fung run
fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)
print(count(1_000_000, 0))
```

<p>꼬리가 아닌 것: 연산자의 피연산자(<code>1 + f()</code>), <code>f()!</code>·<code>f()?</code>·<code>f() ?? b</code>, 다른 호출의 인자, <code>and</code>·<code>or</code>의 오른쪽. 반환 타입을 적은 함수는 자기 자신으로의 꼬리 호출만 프레임을 재사용한다(반환값 검사는 끝에 한 번). 꼬리가 아닌 재귀는 깊이 100,000에서 "stack overflow" 에러다.</p>

## 조건 {#conditions}

```fung run
fn sign(n):
  if n > 0:
    "+"
  else if n < 0:
    "-"
  else:
    "0"
  ;;
;;
print(sign(5), 3 > 2 then "yes" else "no")
```

<ul>
  <li><code>if</code>는 식이다. <code>else</code>가 없고 조건이 거짓이면 값은 <code>nil</code>. 그래서 값을 쓰는 자리(대입의 오른쪽, 인자, <code>return</code>, 연산자, <code>fn f() = …</code>와 람다 <code>=&gt;</code>의 본문 등)의 <code>if</code>는 <code>else</code>가 있어야 한다(없으면 컴파일 에러 E0223, <code>nil</code>이 의도라면 <code>else: nil</code>). 같은 자리의 블록이 대입으로 끝나도 E0224다. 블록 본문 함수의 마지막 <code>if</code>와 문장으로 쓴 <code>if</code>는 <code>else</code> 없이 써도 된다.</li>
  <li><code>c then a else b</code>는 한 줄짜리 조건 식이다. <code>else</code>가 반드시 있어야 하고, 사슬로 이을 수 있다.</li>
  <li>조건은 bool이어야 한다. 다른 값이면 런타임 에러.</li>
</ul>

## match와 패턴 {#match}

```fung frag
match value:
  0                  => …    # 리터럴 (음수, 문자열, 심볼, 기간, true/false/nil 포함)
  'ok                => …    # 심볼
  Ok(x)              => …    # 태그와 인자
  (a, b)             => …    # 튜플
  []                 => …    # 빈 리스트
  [first, ...rest]   => …    # 머리와 꼬리. 꼬리 이름은 생략 가능: [first, ...]
  {name: n}          => …    # 맵의 일부 키 (다른 키는 무시)
  {kind, line}       => …    # {kind: kind, line: line}의 축약
  x when x > 100     => …    # guard
  'a | 'b            => …    # 여러 패턴 중 하나
  _                  => …    # 나머지 전부
;;
```

<ul>
  <li>절의 본문은 다음 절이나 <code>;;</code>가 나올 때까지의 문장들이다. 한 줄로도 쓴다: <code>match x: 0 =&gt; "zero"; _ =&gt; "other" ;;</code>.</li>
  <li>아무 패턴도 맞지 않으면 런타임 에러(E0601).</li>
  <li>패턴 안의 이름은 항상 새로 묶는다. 바깥 변수와 비교하려면 guard를 쓴다: <code>y when y == x =&gt; …</code>.</li>
  <li>대안(<code>|</code>)에 붙은 guard는 맞는 대안마다 차례로 본다. <code>(a, _) | (_, a) when a &gt; 1</code>은 <code>(0, 5)</code>에서 첫 대안(<code>a</code>가 0)의 guard가 거짓이라 둘째 대안(<code>a</code>가 5)으로 맞는다. guard는 그만큼 여러 번 돌 수 있다.</li>
  <li><code>|</code> 대안들은 같은 이름들을 묶어야 한다. 한 패턴에 같은 이름을 두 번 쓸 수 없다.</li>
  <li>괄호 없는 태그 패턴 <code>Ok</code>는 심볼 <code>'Ok</code>에만 맞고 <code>Ok(1)</code> 같은 태그 값에는 맞지 않는다. 태그 값은 <code>Ok(_)</code>처럼 인자 모양까지 쓴다.</li>
</ul>
<h3>구조 분해 대입</h3>
<p>대입의 왼쪽에도 같은 패턴을 쓴다. 맞지 않으면 런타임 에러(E0602).</p>

```fung run
(q, r) = (7 // 2, 7 % 2)
[top, ...rest] = ["a", "b", "c"]
{name, port: p} = {name: "api", port: 8080}
print(q, r, top, rest, name, p)
```

## for {#for}

<p><code>for</code>는 <code>map</code>의 설탕이고 결과로 리스트를 돌려준다. 값을 버리는 자리의 <code>for</code>는 리스트를 만들지 않고 <code>each</code>처럼 돌며, 본문이 낸 <code>Err</code>는 E0606이다. 순회 대상은 리스트, 범위, 채널의 받는 끝(값이 올 때마다, 닫히면 끝)이다. 문자열과 맵은 <code>chars(s)</code>, <code>pairs(m)</code>으로 리스트로 바꿔서 돈다.</p>

```fung run
squares = for x in 1..=3: x * x ;;
for (k, v) in pairs({a: 1, b: 2}):
  print("${k} = ${v}")
;;
print(squares)
```

<p>본문이 사실은 람다이므로 <code>for</code> 본문 안의 <code>return</code>과 <code>?</code>는 컴파일 에러다(<code>map</code>에 바로 쓴 람다 안의 <code>?</code>도 같다, E0226). 실패할 수 있는 변환은 첫 <code>Err</code>에서 멈추는 <code>try_map</code>, 찾기는 <code>find</code>, 검사는 <code>all</code>·<code>any</code>, 누적은 <code>fold</code>를 쓴다. <code>while</code>은 없다.</p>
<div class="aside">
<p><strong>누적과 재귀의 비용.</strong> 리스트와 맵은 불변이지만 사본이 원본과 구조를 공유하므로, <code>fold</code>로 <code>acc + [x]</code>나 <code>put(acc, k, v)</code>를 쌓는 것과 <code>[x, ...rest]</code>·<code>xs[1..]</code>로 꼬리를 떼는 재귀는 모두 선형이다(2만 개에 수십 ms). 피할 것은 둘이다. 문자열을 <code>acc + s</code>로 쌓지 말고 리스트에 모아 <code>join</code>하고, 앞에 붙이기 <code>[x] + acc</code>는 매번 전체를 복사하므로 뒤에 붙인 뒤 <code>reverse</code>한다. 자세한 것은 <a href="../vm/internals.html#collections">컬렉션의 내부</a>.</p>
</div>

## 결과값과 에러 {#results}

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

<p><strong>버려지는 <code>Err</code>는 에러다.</strong> 값을 쓰지 않는 자리(문장으로 쓴 호출, 그런 자리의 <code>if</code>·<code>match</code> 가지, <code>for</code> 문장의 본문, 파일의 마지막 식, <code>each</code>의 <code>f</code>, <code>with</code>의 <code>release</code>, <code>supervise</code>의 <code>on_fail</code>)에서 <code>Err</code>가 나오면 E0606 <code>discarded Err</code>로 멈춘다. <code>set -e</code>를 늘 켜 둔 셸과 같다. 실패해도 상관없으면 <code>_ = 식</code>으로 일부러 버린다. 함수의 마지막 식은 반환값이라 버려지지 않고, REPL은 입력의 마지막 값을 출력한다.</p>

```fung frag
fs.write(p, text)          # 실패하면 E0606
fs.write(p, text)!         # 실패하면 E0603으로 중단
_ = fs.remove(tmp)         # 실패해도 상관없다
each(rxs, r => send(tx, r))   # 닫힌 tx면 E0606
```

<p><code>??</code>는 <strong>모든</strong> <code>Err</code>를 기본값으로 바꾼다. <code>fs.read(p) ?? "{}"</code>는 파일이 없을 때만이 아니라 권한 오류일 때도 <code>"{}"</code>다. 종류를 가리려면 <code>match</code>로 <code>Err({kind: 'not_found}) =&gt; "{}"</code>처럼 쓰고 나머지는 <code>!</code>로 멈춘다.</p>
<p>"여기 오면 버그"를 말할 때는 <code>fail(msg)</code>로 런타임 에러를 낸다. <code>x ?? fail("config.port is required")</code>처럼 <code>??</code>와 짝이 맞는다. 에러를 잡는 구문은 없고, 정리는 <code>with(acquire, use, release)</code>(와 그 위의 <code>fs.with_open</code>)로 한다.</p>

## 모듈 {#modules}

```fung frag
import fs                              # 표준 라이브러리: 이름
import "./lib/rename"                  # 내 파일: import하는 파일 기준 상대 경로, .fung 생략
import "./lib/rename" as rn            # 별칭
import "./lib/rename" (slug, to_kebab) # 이름만 직접 가져오기
```

<ul>
  <li>파일 하나가 모듈 하나다. 모듈 이름은 파일명이고, 이름 규칙에 맞지 않으면(<code>my-lib.fung</code>) <code>as</code>가 필요하다.</li>
  <li><code>_</code>로 시작하는 최상위 이름은 비공개. 나머지는 공개다. 가져온 이름은 다시 내보내지 않는다.</li>
  <li>import된 파일의 최상위에는 <code>import</code>, <code>fn</code>, <code>type</code>, <code>test</code>, <code>이름 = 식</code>만 올 수 있다. 처음 import될 때 한 번 실행되고 캐시된다.</li>
  <li>순환 import는 컴파일 에러이고 순환 경로 전체를 보여 준다. 진입 파일을 import할 수도 없다.</li>
  <li>모듈은 값이다: 함수 인자로 넘길 수 있고 <code>type_of(fs) == 'module</code>.</li>
</ul>

## 타입 표기 {#types}

```fung frag
type shape = Circle | Rect             # 별칭: 같은 모듈 안에서만

fn area(s: shape) -> num = match s:
  Circle(r)  => 3.14 * r * r
  Rect(w, h) => w * h
;;
fn find_user(id: int) -> map | nil = …
fn load(p: str) -> Ok | Err = …
```

<ul>
  <li>타입 이름: <code>int</code> <code>float</code> <code>num</code>(int 또는 float) <code>str</code> <code>bool</code> <code>nil</code> <code>sym</code> <code>dur</code> <code>time</code> <code>list</code> <code>map</code> <code>tuple</code> <code>range</code> <code>fn</code> <code>module</code> <code>task</code> <code>scope</code> <code>sender</code> <code>receiver</code> <code>atom</code> <code>file</code> <code>any</code>, 그리고 태그 이름. 태그 이름은 인자 없는 태그(심볼)도 받으므로 열거형을 쓸 수 있다: <code>type color = Red | Green | Blue</code>, <code>fn hex(c: color)</code>. 그래도 <code>type_of(Red)</code>는 <code>'sym</code>이다.</li>
  <li><code>|</code>로 유니온을 만든다. <code>list[int]</code>처럼 원소 타입을 적을 수 있지만 런타임은 바깥 타입만 검사한다.</li>
  <li>인자와 반환 타입에만 쓴다. 맞지 않으면 런타임 에러(E0403, E0404). 변수에는 쓰지 않는다. <code>...rest</code>는 늘 리스트라 <code>list</code>만 적을 수 있다(다른 표기는 E0228).</li>
  <li>타입 이름은 값 이름과 다른 이름 공간이다. <code>list = [1]</code>이라는 변수가 있어도 <code>x: list</code>는 타입이다.</li>
  <li>별칭 이름은 소문자이고(대문자는 태그), 위의 타입 이름은 별칭으로 쓸 수 없다(<code>type int = str</code>은 E0222).</li>
</ul>

## 테스트와 디버깅 {#testing}

```fung run
fn half(n) = n // 2

test "half rounds down":
  assert half(5) == 2
  assert half(-5) == -3
;;

# [파일:줄] half(10) = 5 를 표준 에러로 찍고 값을 돌려준다
x = dbg(half(10)) + 1
print(x)
```

<ul>
  <li><code>test "이름": … ;;</code>는 최상위에만 둔다. <code>fung run</code>은 무시하고 <code>fung test</code>가 실행한다. 블록 하나가 실패해도 다음 블록이 돈다. 다만 <code>exit(n)</code>이 불리거나(띄운 태스크에서도) 교착이 보고되면 그 파일의 나머지 블록은 건너뛰고, 건너뛰기를 실패 하나로 센다.</li>
  <li><code>assert 식</code>은 어디서나 쓴다. 비교가 실패하면 양쪽 값을 보여 준다(E0807).</li>
  <li><code>dbg(x)</code>는 식의 소스 텍스트와 값을 찍고 값을 그대로 돌려준다.</li>
</ul>

## 형식 문법 {#grammar}

<p>파서가 그대로 옮긴 EBNF다. 식의 층은 위 우선순위 표와 같다.</p>
<details>
<summary>EBNF 펼치기</summary>

```plain
(* 토큰: NAME(소문자·_·대소문자 없는 글자로 시작) TAG(대문자 시작) INT FLOAT DURATION
         STRING(보간 조각 포함) RAW_STRING SYMBOL NEWLINE, 예약어, 연산자 *)

program     = body .
body        = { sep } { stmt { sep } } .
sep         = NEWLINE | ";" .
block       = ":" body ";;" .

stmt        = fn_def | import | type_alias | test_block | assert | assign | expr .
fn_def      = "fn" NAME "(" [ params ] ")" [ "->" type ] fn_body .
fn_body     = "=" expr | block .
params      = param { "," param } [ "," ] .
param       = [ "..." ] NAME [ ":" type ] [ "=" expr ] | pattern .
import      = "import" ( NAME | STRING ) [ "as" NAME ] [ "(" NAME { "," NAME } ")" ] .
type_alias  = "type" NAME "=" type .
test_block  = "test" STRING block .
assert      = "assert" expr .
assign      = pattern "=" expr .                   (* 문장 시작에서 `pattern =`를 먼저 시도 *)

expr        = lambda | then_expr .
lambda      = ( NAME | "_" | "(" [ pattern { "," pattern } ] ")" ) "=>" expr .
then_expr   = or_expr [ "then" expr "else" expr ] .
or_expr     = and_expr { "or" and_expr } .
and_expr    = not_expr { "and" not_expr } .
not_expr    = "not" not_expr | cmp_expr .
cmp_expr    = pipe_expr [ cmp_op pipe_expr ] .      (* 연쇄 없음 *)
cmp_op      = "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "not" "in" .
pipe_expr   = coalesce { "|>" coalesce } .
coalesce    = range { "??" range } .
range       = add [ ( ".." | "..=" ) add ] .
add         = mul { ( "+" | "-" ) mul } .
mul         = unary { ( "*" | "/" | "//" | "%" ) unary } .
unary       = "-" unary | pow .
pow         = postfix [ "**" unary ] .              (* 오른쪽 결합 *)
postfix     = primary { call | index | "." key_name | "!" | "?" } .
key_name    = NAME | TAG | KEYWORD .                (* 이름 자리에서는 예약어도 이름 *)
call        = "(" [ arg { "," arg } [ "," ] ] ")" .
arg         = [ NAME ":" ] expr | "_" .
index       = "[" ( expr | [ expr ] ".." [ expr ] | expr "..=" expr ) "]" .

primary     = INT | FLOAT | STRING | RAW_STRING | DURATION | SYMBOL
            | "true" | "false" | "nil" | NAME | TAG
            | "(" expr ")" | tuple | list | map
            | if_expr | match_expr | for_expr
            | "return" [ expr ]
            | "fn" "(" [ params ] ")" [ "->" type ] block .
tuple       = "(" ")" | "(" expr "," [ expr { "," expr } [ "," ] ] ")" .
list        = "[" [ expr { "," expr } [ "," ] ] "]" .
map         = "{" [ entry { "," entry } [ "," ] ] "}" .
entry       = ( key_name | STRING ) ":" expr | NAME | TAG .
if_expr     = "if" expr ":" body { "else" "if" expr ":" body } [ "else" ":" body ] ";;" .
match_expr  = "match" expr ":" { sep } { arm } ";;" .
arm         = pattern [ "when" expr ] "=>" body .
for_expr    = "for" pattern "in" expr ":" body ";;" .

pattern     = pat_term { "|" pat_term } .
pat_term    = "_" | NAME | [ "-" ] ( INT | FLOAT ) | STRING | SYMBOL | DURATION
            | "true" | "false" | "nil"
            | TAG [ "(" [ pattern { "," pattern } ] ")" ]
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pattern { "," pattern } ] [ [ "," ] "..." [ NAME ] ] "]"
            | "{" [ pat_entry { "," pat_entry } ] "}" .
pat_entry   = key_name ":" pattern | NAME | TAG .

type        = type_term { "|" type_term } .
type_term   = NAME [ "[" type "]" ] | TAG .
```

</details>
