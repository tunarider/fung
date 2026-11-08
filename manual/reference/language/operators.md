---
description: 산술·비교·논리 연산자, 파이프 |>, 우선순위.
lede: 산술·비교·논리 연산자, 파이프 <code>|&gt;</code>, 우선순위.
---
# 연산자

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

```fung run
import json

name = "a.log"
size = 3
files = ["a", "b", "c", "d"]
print(name |> ends_with(".log") and size > 0)  # (name |> ends_with(".log")) and (size > 0)
print(files |> len > 3)                        # (files |> len) > 3
print(Err("none") ?? "{}" |> json.parse)       # (Err("none") ?? "{}") |> json.parse
print(-2 ** 2)                                 # -(2 ** 2) == -4
```

<p><code>not</code>은 비교보다 낮으므로 그보다 높은 연산자의 피연산자가 될 수 없다. <code>a == not b</code>, <code>1 + not c</code>는 에러이고 괄호로 <code>a == (not b)</code>라고 쓴다. <code>not a == b</code>는 <code>not (a == b)</code>다.</p>

## 경계 사례와 에러 {#edges}

비교 연산자는 잇지 않는다(`a < b < c`는 수학과 다르게 읽히기 쉽다).

```fung run exit=2 error=E0111
a = 1
b = 2
c = 3
print(a < b and b < c)
print(a < b < c)
```

`+`는 같은 종류끼리만이다. 문자열과 정수를 더하지 않는다(보간을 쓴다).

```fung run exit=1 error=E0501
n = 3
print("n = ${n}")
print("n = " + n)
```

파이프의 값은 언제나 첫 인자다. 오른쪽 호출에 `_`를 쓰면 에러다.

```fung run exit=2 error=E0227
print([3, 1, 2] |> sort)
print([1, 2] |> map(_, x => x))
```

`/`는 언제나 실수, `//`는 정수 나눗셈이다. 0으로 나누면 에러다.

```fung run exit=1 error=E0502
print(7 / 2, 7 // 2, 7 % 2)
print(1 // 0)
```
