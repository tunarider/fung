---
description: 문장과 블록(: … ;;), 줄 이어짐.
lede: 문장과 블록(<code>:</code> … <code>;;</code>), 줄 이어짐.
---
# 문장과 블록

<ul>
  <li>문장은 줄바꿈이나 <code>;</code>로 나눈다. 빈 문장은 괜찮다.</li>
  <li>블록은 <code>:</code>로 열고 <code>;;</code>로 닫는다. 블록의 값은 마지막 식의 값이다.</li>
  <li>값을 아무도 쓰지 않는 자리(마지막이 아닌 식, 그런 자리의 <code>if</code>·<code>match</code> 가지의 끝, 파일의 마지막 식)의 값은 버려진다. 버려지는 값이 <code>Err</code>면 E0606이다(<a href="results.html">결과값과 에러</a>).</li>
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

블록은 `;;`로 닫는다. 닫지 않고 파일이 끝나면 어디서 시작했는지와 함께 멈춘다.

```fung run exit=2 error=E0112
fn f(x):
  x + 1
print(f(1))
```

블록의 값은 마지막 식이다. 마지막 문장이 대입이면 그 블록의 값은 `nil`이라, 값으로 쓰면 에러다(E0224). 빈 블록이나 `assert`·`fn` 정의로 끝나는 블록도 값으로 쓰면 에러다(E0231). `nil`이 뜻이면 마지막 줄에 `nil`을 쓴다.

```fung run exit=2 error=E0224
fn f(x):
  y = x + 1
;;
print(f(1))
```
