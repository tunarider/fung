---
description: 문장과 블록(: … ;;), 줄 이어짐.
lede: 문장과 블록(<code>:</code> … <code>;;</code>), 줄 이어짐.
---
# 문장과 블록

<ul>
  <li>문장은 줄바꿈이나 <code>;</code>로 나눈다. 빈 문장은 괜찮다.</li>
  <li>블록은 <code>:</code>로 열고 <code>;;</code>로 닫는다. 블록의 값은 마지막 식의 값이다.</li>
  <li>값을 아무도 쓰지 않는 자리(마지막이 아닌 식, 그런 자리의 <code>if</code>·<code>match</code> 가지의 끝, 파일의 마지막 식)의 값은 버려진다. 버려지는 값이 <code>Err</code>면 E0606이다(<a href="results.html">결과값과 에러</a>).</li>
  <li>버려지는 자리의 식은 무언가를 해야 한다. 이름, 리터럴, 람다·익명 <code>fn</code>, 이름과 리터럴만 넘긴 부분 적용(<code>f(_, x)</code>), 그리고 그것들만으로 된 연산자·필드·인덱스·튜플·리스트·맵·태그 값(<code>Ok(1)</code>, <code>Red</code>, <code>Point(x, y)</code>)은 계산해도 아무 일이 없으므로 컴파일 에러다(E0234). 호출, 대입, <code>assert</code>, <code>return</code>, 호출이 든 식(<code>Ok(f())</code>, <code>h(g(), _)</code>)은 된다. 다만 효과가 없다고 알려진 내장 함수·표준 모듈 함수의 호출(<code>put(m, k, v)</code>, <code>sort(xs)</code>, <code>path.join(a, b)</code>처럼 값을 계산해 돌려줄 뿐인 것)도 E0234다: 결과를 버리면 아무 일도 없었던 것이다. 함수를 받는 내장 함수(<code>map</code>·<code>each</code> 등)와 사용자 함수는 무엇을 할지 모르므로 된다. 예외는 맨 <code>nil</code> 하나다: 문장으로 쓴 <code>match</code>에서 할 일이 없는 절은 <code>_ =&gt; nil</code>로 쓰는데, 절에는 본문이 있어야 하므로 "아무것도 하지 않는다"를 <code>nil</code>로 적게 둔다. 일부러 버리려면 <code>_ = 식</code>. 실행 중에 계산된 <code>Err</code>를 버리면 여전히 E0606이다.</li>
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
  <li>줄이 식을 끝낼 수 없는 토큰으로 끝날 때: 이항 연산자(<code>+</code> <code>-</code> <code>*</code> <code>/</code> <code>//</code> <code>%</code> <code>**</code> <code>==</code> <code>!=</code> <code>&lt;</code> <code>&gt;</code> <code>&lt;=</code> <code>&gt;=</code> <code>in</code> <code>??</code> <code>|&gt;</code> <code>..</code> <code>..=</code> <code>and</code> <code>or</code>), <code>not</code>, <code>,</code>, <code>=</code>, <code>=&gt;</code>, <code>-&gt;</code>, <code>.</code>, 패턴 대안과 타입 유니온의 <code>|</code>, <code>for</code> 머리의 <code>in</code>, <code>then</code>, <code>else</code>, 여는 괄호, 블록 시작 <code>:</code>.</li>
  <li>다음 줄이 이어짐 전용 토큰으로 시작할 때: <code>|&gt;</code> <code>.</code> <code>??</code> <code>and</code> <code>or</code> <code>then</code> <code>else</code>. 이 토큰들로는 문장을 시작할 수 없으니 모호하지 않다.</li>
  <li>괄호 <code>()</code> <code>[]</code> <code>{}</code> 안: 줄바꿈은 공백이다(<code>not</code>과 <code>in</code> 사이, 이름 인자의 이름과 <code>:</code> 사이도). 단, 괄호 안에서 <code>fn(x):</code>처럼 블록이 열리면 그 <code>;;</code>까지는 줄바꿈이 다시 문장 구분자다.</li>
</ol>

<p><code>-</code>는 이어짐 전용 토큰이 아니다(문장을 <code>-x</code>로 시작할 수 있다). 그래서 앞 줄을 이어 쓰려던 <code>- 2</code> 줄(포매터는 <code>-2</code>로 붙인다)은 새 문장이 되고, 앞 줄의 식은 버려지는 자리에 남아 E0234다. 여러 줄에 걸친 식은 연산자를 줄 끝에 둔다.</p>

```fung run
names = ["b.log", "a.txt", "c.log"]
logs = names
  |> filter(n => ends_with(n, ".log"))
  |> sort
ok = len(logs) > 0
  and all(logs, n => n != "")
print(logs, ok)
```

## 경계 사례와 에러 {#edges}

버려지는 자리에서 아무 일도 하지 않는 식은 에러다. 괄호를 잊은 호출, `=` 대신 쓴 `==`, 줄 앞의 `-`가 흔한 경우다.

```fung run exit=2 error=E0234
fn f(a):
  a
  -2
;;
print(f(5))
```

연산자를 줄 끝에 두면 한 식으로 이어진다.

```fung run
fn f(a) = a -
  2
print(f(5))
```

블록은 `;;`로 닫는다. 닫지 않고 파일이 끝나면 어디서 시작했는지와 함께 멈춘다.

```fung run exit=2 error=E0112
fn f(x):
  x + 1
print(f(1))
```

블록의 값은 마지막 식이다. 마지막 문장이 대입이면 그 블록의 값은 `nil`이라, 값으로 쓰면 에러다(E0224). 빈 블록이나 `assert`·`fn` 정의로 끝나는 블록도 값으로 쓰면 같은 에러다. `nil`이 뜻이면 마지막 줄에 `nil`을 쓴다.

```fung run exit=2 error=E0224
fn f(x):
  y = x + 1
;;
print(f(1))
```

효과가 없다고 알려진 내장 함수를 문장으로 부르는 것도 같다. `put`은 바뀐 사본을 돌려줄 뿐 `m`을 바꾸지 않는다.

```fung run exit=2 error=E0234
m = {a: 1}
put(m, "b", 2)
print(m)
```
