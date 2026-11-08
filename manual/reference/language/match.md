---
description: match, 패턴의 종류, guard, 구조 분해 대입.
lede: <code>match</code>, 패턴의 종류, guard, 구조 분해 대입.
---
# match와 패턴

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

대안 패턴(`|`)의 갈래들은 같은 이름을 묶어야 한다.

```fung run exit=2 error=E0216
fn f(v) = match v:
  (a, 0) | (0, b) => 1
  _               => 0
;;
print(f((1, 0)))
```
