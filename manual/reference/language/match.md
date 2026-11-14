---
description: match, 패턴의 종류, guard, 구조 분해 대입.
lede: <code>match</code>, 패턴의 종류, guard, 구조 분해 대입.
---
# match와 패턴

```fung frag
match value:
  0                  => …    # 리터럴 (음수, 문자열, 심볼, 기간(-1s도), true/false/nil 포함)
  'ok                => …    # 심볼
  Ok(x)              => …    # 태그와 인자
  (a, b)             => …    # 튜플
  []                 => …    # 빈 리스트
  [first, ...rest]   => …    # 머리와 꼬리. 꼬리에 이름을 붙이지 않으면 [first, ...] ([first, ..._]는 에러)
  {}                 => …    # 빈 맵
  {name: n}          => …    # 키가 정확히 name 하나인 맵
  {name: n, ...}     => …    # 다른 키는 무시. 나머지에 이름을 붙이면 {name: n, ...rest} ({...}는 모든 맵)
  {kind, line}       => …    # {kind: kind, line: line}의 축약
  x when x > 100     => …    # guard
  'a | 'b            => …    # 여러 패턴 중 하나
  _                  => …    # 나머지 전부
;;
```

<ul>
  <li>절의 본문은 다음 절이나 <code>;;</code>가 나올 때까지의 문장들이다. 한 줄로도 쓴다: <code>match x: 0 =&gt; "zero"; _ =&gt; "other" ;;</code>.</li>
  <li>절의 본문은 비울 수 있다: 할 일이 없는 절은 <code>_ =&gt;</code> 뒤에 바로 다음 절이나 <code>;;</code>가 온다(<code>match x: 1 =&gt; print("a"); _ =&gt; ;;</code>). 값으로 쓰는 <code>match</code>의 빈 절은 다른 빈 블록처럼 E0224다.</li>
  <li>아무 패턴도 맞지 않으면 런타임 에러(E0601).</li>
  <li>패턴 안의 이름은 항상 새로 묶는다. 바깥 변수와 비교하려면 guard를 쓴다: <code>y when y == x =&gt; …</code>.</li>
  <li>guard 없는 앞 절의 패턴이 <strong>덮는</strong> 절은 결코 돌지 않으므로 컴파일 에러다(E0235). 절의 <code>|</code> 대안 가운데 하나만 덮이면 그 대안이 E0241이다. 한 <code>|</code> 안에서 앞 대안이 덮는 뒤 대안도 E0241이다. 패턴 A가 B를 덮는다는 것은 B에 맞는 값이 모두 A에도 맞는다는 것을 모양만 보고 아는 것이다:
    <ul>
      <li><code>_</code>와 맨 이름은 무엇이든 덮는다. 그래서 모든 값에 맞는 절은 마지막에 둔다(그 뒤에 절이 있으면 에러가 그 절을 가리킨다).</li>
      <li>리터럴은 같은 리터럴을 덮는다: 값이 같은 수(<code>1</code>과 <code>1.0</code>, <code>16</code>과 <code>0x10</code>), 쓴 그대로 같은 문자열·심볼·<code>nil</code> 등.</li>
      <li>튜플은 개수가 같은 튜플을, 태그는 이름과 값의 개수가 같은 태그를 칸마다 덮을 때 덮는다(<code>Ok(_)</code>는 <code>Ok(1)</code>을).</li>
      <li>리스트 패턴은 길이가 같은 닫힌 리스트 패턴을 칸마다 덮을 때, <code>...</code>로 연 것(<code>[x, ...]</code>)은 앞 칸들을 덮는 더 긴 패턴(열렸든 닫혔든)도 덮는다.</li>
      <li>맵 패턴은 닫힌 것이 키가 같은 닫힌 것을, 연 것(<code>{a: _, ...}</code>)은 그 키들을 모두 가진 것(열렸든 닫혔든)을 값마다 덮을 때 덮는다. <code>{...}</code>는 모든 맵 패턴을 덮고, 닫힌 것은 연 것을 덮지 않는다.</li>
      <li><code>a | b</code>는 갈래 하나가 덮으면 덮고, <code>c | d</code>는 갈래가 모두 덮일 때 덮인다.</li>
    </ul>
    guard가 있는 절은 덮지 않는다. 여러 절이 함께 덮는 것(<code>true</code>와 <code>false</code> 뒤의 <code>_</code>)은 보지 않는다.</li>
  <li>절 본문의 문장이 <code>패턴 =&gt;</code>로 읽히면 새 절이다(파서는 문장마다 먼저 "<code>패턴 =&gt;</code>인가"를 본다). 절의 값인 람다는 괄호로 감싼다: <code>1 =&gt; (y =&gt; y + 1)</code>. 괄호 없이 쓴 람다 문장은 모든 값에 맞는 새 절이 되므로, 뒤에 절이 있으면 E0235로 드러난다.</li>
  <li>guard의 맨 바깥에 오는 <code>이름 =&gt;</code>·<code>(이름들) =&gt;</code>는 람다가 아니라 절의 화살표다: <code>y when y == x =&gt; …</code>. guard 안에서 람다가 필요하면 괄호 안(호출 인자 등)에 쓴다: <code>n when any(xs, x =&gt; x &gt; n) =&gt; …</code>.</li>
  <li>대안(<code>|</code>)에 붙은 guard는 맞는 대안마다 차례로 본다. <code>(a, _) | (_, a) when a &gt; 1</code>은 <code>(0, 5)</code>에서 첫 대안(<code>a</code>가 0)의 guard가 거짓이라 둘째 대안(<code>a</code>가 5)으로 맞는다. guard는 그만큼 여러 번 돌 수 있다. 이렇게 펼치는 것은 이름을 묶는 대안뿐이다: <code>1 | 2</code>처럼 이름을 묶지 않는 <code>|</code>는 guard가 보는 값을 바꾸지 못하므로 펼치지 않고, 몇 겹이든 한도가 없다. 이름을 묶는 대안을 펼친 조합은 256개까지 되고(중첩된 <code>|</code>마다 곱해진다), 넘으면 E0230이다.</li>
  <li><code>|</code> 대안들은 같은 이름들을 묶어야 한다. 한 패턴에 같은 이름을 두 번 쓸 수 없다.</li>
  <li>맵 패턴은 리스트 패턴과 같은 규칙이다: 적은 키와 정확히 맞는다. 다른 키도 받으려면 끝에 <code>...</code>를 한 번 쓴다. <code>{kind: k, ...}</code>는 다른 키를 무시하고, <code>{kind: k, ...rest}</code>는 나머지 항목을 맵 <code>rest</code>에 맵에 넣은 순서대로 묶는다. <code>{}</code>는 빈 맵에만, <code>{...}</code>는 모든 맵에 맞는다. 표준 라이브러리의 <code>Err</code> 내용에는 <code>kind</code> 말고도 <code>message</code> 등이 있으므로 <code>Err({kind: 'not_found, ...})</code>로 가른다. 대입·<code>for</code>·매개변수의 구조 분해도 같다.</li>
  <li>맵 패턴의 축약 <code>{kind}</code>는 키와 같은 이름을 묶는다. <code>_</code>는 이름이 아니라 아무것도 묶지 않으므로 축약에 쓸 수 없다(<code>{_}</code>는 E0112). 키 <code>"_"</code>는 <code>{_: v}</code>, 키가 있는지만 보려면 <code>{key: _, ...}</code>, 모든 맵은 <code>{...}</code>로 쓴다.</li>
  <li>괄호 없는 태그 패턴 <code>Red</code>는 심볼 <code>Red</code>에만 맞고 <code>Red(1)</code> 같은 태그 값에는 맞지 않는다. 태그 값은 <code>Red(_)</code>처럼 인자 모양까지 쓴다. <code>Ok</code>·<code>Err</code>도 같다: 패턴 <code>Ok</code>는 심볼 <code>Ok</code>에만 맞고, 결과값에는 <code>Ok(_)</code>로 맞춘다.</li>
</ul>
<h3>구조 분해 대입</h3>
<p>대입의 왼쪽에도 패턴을 쓴다. 맞지 않으면 런타임 에러(E0602). 대입·<code>for</code>·매개변수는 같은 <strong>구조 분해 패턴</strong>을 받는다: 이름, <code>_</code>, 튜플, 리스트, 맵, 값을 담은 태그, 또는 그것들의 <code>|</code> 대안이다(<code>Ok(x) | Err(x) = r</code>). 값을 고르기만 하는 리터럴(<code>1</code>, <code>"a"</code>, <code>'a</code>, 인자 없는 태그, <code>nil</code>)은 맨 바깥에 올 수 없고(E0112) 안쪽에는 온다(<code>(1, y) = p</code>).</p>

```fung run
(q, r) = (7 // 2, 7 % 2)
[top, ...rest] = ["a", "b", "c"]
{name, port: p} = {name: "api", port: 8080}
print(q, r, top, rest, name, p)
```

## 경계 사례와 에러 {#edges}

아무 절도 맞지 않으면 에러다. 마지막 절을 `_`로 둔다.

```fung run exit=1 error=E0601
fn sign(n) = match n:
  0            => "zero"
  k when k > 0 => "plus"
;;
print(sign(1))
print(sign(-1))
```

구조 분해 대입이 맞지 않으면 에러다.

```fung run exit=1 error=E0602
(a, b) = (1, 2)
print(a + b)
[_x, _y] = [1, 2, 3]
```

맵 패턴도 리스트 패턴처럼 적은 키와 정확히 맞는다. 다른 키가 있는 맵은 `...` 없는 맵 패턴에 맞지 않는다.

```fung run exit=1 error=E0602
{name: n} = {name: "api", port: 8080}
print(n)
```

다른 키는 `...`로 무시하거나 `...rest`로 받는다. `{}`는 빈 맵에만 맞는다.

```fung run
{name: n, ...} = {name: "api", port: 8080}
{name: m, ...rest} = {name: "web", port: 80, tls: true}
print(n, m, rest)
fn shape(v) = match v:
  {}    => "empty map"
  {...} => "map"
  _     => "not a map"
;;
print(shape({}), shape({a: 1}), shape([]))
```

앞 절이 덮는 절은 돌 수 없어 에러다. 모양이 같고 칸마다 덮으면 덮는다.

```fung run exit=2 error=E0235
fn f(r) = match r:
  Ok(_) => "ok"
  Ok(0) => "zero"
  _     => "error"
;;
print(f(Ok(0)))
```

```fung run exit=2 error=E0241
fn f(code) = match code:
  200 | 201 => "ok"
  201 | 204 => "no content"
  _         => "other"
;;
print(f(204))
```

모든 값에 맞는 절 뒤의 절도 같다. 패턴의 이름은 바깥 변수와 비교하지 않고 새로 묶으므로, 맨 이름 패턴도 모든 값에 맞는다. 비교는 guard로 한다.

```fung run exit=2 error=E0235
limit = 5
fn f(x) = match x:
  limit => "at limit"
  _     => "other"
;;
print(f(3))
```

```fung run
limit = 5
fn f(x) = match x:
  v when v == limit => "at limit"
  _                 => "other"
;;
print(f(3), f(5))
```

절 본문 안에 람다를 쓰려던 `y => y + 1` 줄은 람다가 아니라 새 절(모든 값에 맞는 `y`)로 읽힌다. `fung fmt`는 그 줄을 절의 들여쓰기로 옮겨 보여 준다. 같은 줄의 `1 => y => y + 1`도 본문이 빈 절 `1 =>`과 절 `y => y + 1`이다(값으로 쓰면 빈 절이 E0224, 문장으로 쓰면 뒤에 절이 있을 때 `y`가 모든 값에 맞아 E0235이고 도움말이 괄호를 알린다). 절의 값인 람다는 괄호로 감싼다.

```fung run exit=2 error=E0235
fn pick(m) = match m:
  1 =>
    print("one")
  y => y + 1
  _ => (y => y)
;;
print(pick(2)(1))
```

```fung run
fn pick(m) = match m:
  1 => (y => y + 1)
  _ => (y => y)
;;
print(pick(1)(1), pick(2)(1))
```

대안 패턴(`|`)의 갈래들은 같은 이름을 묶어야 한다.

```fung run exit=2 error=E0216
fn f(v) = match v:
  (a, 0) | (0, b) => 1
  _               => 0
;;
print(f((1, 0)))
```

guard의 맨 바깥 `=>`는 절의 화살표이고, 괄호 안의 람다는 람다다.

```fung run
xs = [1, 5]
fn f(v) = match v:
  n when any(xs, x => x > n) => "below max"
  _                          => "max or more"
;;
print(f(2), f(9))
```

절의 값인 람다를 괄호 없이 쓰면 새 절로 읽혀, 뒤의 절이 돌 수 없다. 아래는 `0` 절의 끝에 람다 `x => x`를 두려던 것이다(포매터도 그 줄을 절로 내어 쓴다).

```fung run exit=2 error=E0235
fn adder(n) = match n:
  0 =>
    print("zero")
  x => x
  _ => (x => x + n)
;;
print(adder(1)(2))
```
