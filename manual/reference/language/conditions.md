---
description: if, then … else, and·or, 값 자리의 조건.
lede: <code>if</code>, <code>then … else</code>, <code>and</code>·<code>or</code>, 값 자리의 조건.
---
# 조건

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
  <li><code>if</code>는 식이다. <code>else</code>가 없고 조건이 거짓이면 값은 <code>nil</code>. 그래서 값을 쓰는 자리(대입의 오른쪽, 인자, <code>return</code>, 연산자, <code>fn f() = …</code>와 람다 <code>=&gt;</code>의 본문 등)의 <code>if</code>는 <code>else</code>가 있어야 한다(없으면 컴파일 에러 E0223, <code>nil</code>이 의도라면 <code>else: nil</code>). 같은 자리의 블록이 비었거나 대입 같은 문장으로 끝나도 E0224다. 함수 본문의 마지막 항목도 값을 쓰는 자리다(반환값이므로, 블록 본문 함수도). 돌려줄 것이 없는 함수는 <code>else: nil</code>이나 마지막 줄 <code>nil</code>로 맺는다. 문장으로 쓴 <code>if</code>, 문장 <code>for</code>의 본문, <code>test</code> 블록의 끝은 <code>else</code> 없이 써도 된다. 문장 <code>for</code>는 리스트를 만들지 않으므로(<a href="for.html">for</a>) 본문의 값이 쓰이지 않는다. 같은 순회를 <code>each(xs, x =&gt; if c: print(x) ;;)</code>로 쓰면 E0223이다: 람다는 값을 돌려주는 함수이고, 그 값을 <code>each</code>가 버린다는 것은 컴파일러가 알 수 없다. 부수효과만 하는 순회는 문장 <code>for</code>로 쓴다.</li>
  <li><code>c then a else b</code>는 한 줄짜리 조건 식이다. <code>else</code>가 반드시 있어야 하고, 사슬로 이을 수 있다.</li>
  <li>조건은 bool이어야 한다. 다른 값이면 런타임 에러(E0505). 리터럴을 바로 쓴 <code>if 1:</code>·<code>not 1</code>·<code>1 and true</code>는 타입이 이미 드러나 있어 컴파일할 때 같은 번호로 멈춘다. 리터럴을 부르는 것(<code>1(2)</code>, E0401), 두 리터럴에 뜻이 없는 연산(<code>1 + "a"</code>, E0501), 리터럴 뒤의 <code>??</code>(E0243)도 같다. 값이 정하는 에러(<code>1 // 0</code>, 정수 넘침)는 실행할 때 난다.</li>
</ul>

## 경계 사례와 에러 {#edges}

`else` 없는 `if`를 값으로 쓰면, 조건이 거짓일 때 `nil`이 되므로 에러다.

```fung run exit=2 error=E0223
x = 5
label = if x > 3: "big" else: "small" ;;
print(label)
bad = if x > 3: "big" ;;
```

조건은 `bool`이어야 한다. `0`이나 `""`를 거짓으로 보지 않는다.

```fung run exit=1 error=E0505
n = 0
print(n == 0 then "zero" else "some")
print(n then "yes" else "no")
```
