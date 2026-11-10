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
  <li>guard 없는 절의 패턴이 모든 값에 맞으면(<code>_</code>, 맨 이름, 그런 갈래가 하나라도 있는 <code>|</code> 대안, 예: <code>1 | _</code>) 그 뒤의 절은 결코 돌지 않으므로 컴파일 에러다(E0235). 그런 절은 마지막에 둔다.</li>
  <li>절 본문의 문장이 <code>패턴 =&gt;</code>로 읽히면 새 절이다(파서는 문장마다 먼저 "<code>패턴 =&gt;</code>인가"를 본다). 절의 값인 람다는 괄호로 감싼다: <code>1 =&gt; (y =&gt; y + 1)</code>. 괄호 없이 쓴 람다 문장은 모든 값에 맞는 새 절이 되므로, 뒤에 절이 있으면 E0235로 드러난다.</li>
  <li>guard의 맨 바깥에 오는 <code>이름 =&gt;</code>·<code>(이름들) =&gt;</code>는 람다가 아니라 절의 화살표다: <code>y when y == x =&gt; …</code>. guard 안에서 람다가 필요하면 괄호 안(호출 인자 등)에 쓴다: <code>n when any(xs, x =&gt; x &gt; n) =&gt; …</code>.</li>
  <li>대안(<code>|</code>)에 붙은 guard는 맞는 대안마다 차례로 본다. <code>(a, _) | (_, a) when a &gt; 1</code>은 <code>(0, 5)</code>에서 첫 대안(<code>a</code>가 0)의 guard가 거짓이라 둘째 대안(<code>a</code>가 5)으로 맞는다. guard는 그만큼 여러 번 돌 수 있다.</li>
  <li><code>|</code> 대안들은 같은 이름들을 묶어야 한다. 한 패턴에 같은 이름을 두 번 쓸 수 없다.</li>
  <li>괄호 없는 태그 패턴 <code>Ok</code>는 심볼 <code>'Ok</code>에만 맞고 <code>Ok(1)</code> 같은 태그 값에는 맞지 않는다. 태그 값은 <code>Ok(_)</code>처럼 인자 모양까지 쓴다.</li>
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
  n when n > 0 => "plus"
;;
print(sign(1))
print(sign(-1))
```

구조 분해 대입이 맞지 않으면 에러다.

```fung run exit=1 error=E0602
(a, b) = (1, 2)
print(a + b)
[x, y] = [1, 2, 3]
```

모든 값에 맞는 절 뒤의 절은 돌 수 없어 에러다. 패턴의 이름은 바깥 변수와 비교하지 않고 새로 묶으므로, 맨 이름 패턴도 모든 값에 맞는다. 비교는 guard로 한다.

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

절 본문 안에 람다를 쓰려던 `y => y + 1` 줄은 람다가 아니라 새 절(모든 값에 맞는 `y`)로 읽힌다. `fung fmt`는 그 줄을 절의 들여쓰기로 옮겨 보여 준다. 같은 줄의 `1 => y => y + 1`은 E0112다. 절의 값인 람다는 괄호로 감싼다.

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
