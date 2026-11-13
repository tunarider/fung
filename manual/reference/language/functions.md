---
description: fn, 람다, 인자 규약, 기본값, 클로저, 꼬리 호출.
lede: <code>fn</code>, 람다, 인자 규약, 기본값, 클로저, 꼬리 호출.
---
# 함수

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
  <li>호출할 때 위치 인자 뒤에 이름 붙인 인자를 쓴다: <code>exec(cmd, cwd: dir)</code>. 내장 함수와 람다도 똑같다(<code>f = (a, b) =&gt; a - b</code>이면 <code>f(b: 1, a: 10)</code>은 9).</li>
  <li>호출의 마지막 위치 인자 자리에 <code>...xs</code>를 하나 쓰면 리스트나 범위 <code>xs</code>의 원소가 그 자리부터의 위치 인자가 된다: <code>f(a, ...xs, k: v)</code>. 다른 위치 인자 앞, 리스트 리터럴(<code>[a, ...xs]</code>는 <code>[a] + xs</code>로), 태그의 값에는 쓸 수 없다(E0133). 리스트·범위가 아닌 값을 펼치면 런타임 에러 E0506.</li>
  <li>인자 개수가 맞지 않거나(E0402·E0407), 같은 인자를 위치와 이름으로 두 번 주거나(E0405), 모르는 이름이면(E0406) 에러다. 호출 대상이 컴파일 때 정해지면(가려지지 않은 최상위 <code>fn</code>, 블록 안의 <code>fn</code>, prelude 함수, import한 모듈의 함수, 그리고 이들의 부분 적용 <code>f(a, _)</code>) 컴파일 때 같은 번호와 문구로 잡고, 그 밖(변수에 든 함수 값, 펼친 인자의 개수)은 호출할 때 잡는다.</li>
  <li>매개변수는 본문의 지역 이름이다. 보이는 바깥 이름(바깥 지역, 최상위 이름, import한 이름, prelude 함수)과 같은 이름이면 E0240이고(공개 최상위 <code>fn</code>의 매개변수만은 prelude 함수와 같은 이름일 수 있다, <a href="modules.html">모듈</a>), 본문과 뒤 매개변수의 기본값 어디서도 읽지 않으면 E0239다(<a href="scope.html">변수와 스코프</a>). 받기만 하는 매개변수는 <code>_</code>로 시작한다: <code>on_fail: _e =&gt; nil</code>. 람다·<code>for</code>·<code>match</code> 패턴이 묶는 이름도 같다.</li>
  <li>매개변수 자리에 구조 분해 패턴(<code>_</code>, 튜플, 리스트, 맵, 태그)을 쓸 수 있다: <code>fn dist((x, y)) = …</code>, <code>pairs(m) |&gt; map(((k, v)) =&gt; k)</code>. <code>fn</code>과 람다가 같고, 대입·<code>for</code>와 같은 <a href="match.html">구조 분해 패턴</a>이라 맨 바깥의 리터럴(<code>(1) =&gt; …</code>, 인자 없는 태그 <code>fn f(Red)</code>)은 매개변수가 될 수 없다(E0112, 값으로 고르려면 <code>match</code>).</li>
</ul>

```fung run
fn greet(name, greeting = "hello", ...extra) = "${greeting}, ${name}${join(extra)}"
print(greet("fung"), greet("fung", greeting: "hi"), greet("fung", "yo", "!", "?"))
fn log(level, ...parts) = print("[${level}]", ...parts)
log("info", "started", 3)
```

<h3>평가 순서</h3>
<p>언제나 <strong>왼쪽에서 오른쪽</strong>이다: 호출의 인자, 리스트·맵·튜플 리터럴의 원소, 이항 연산의 두 피연산자(파이프 <code>x |&gt; f</code>의 <code>x</code>와 <code>f</code>도), 문자열 보간의 조각. <code>and</code>·<code>or</code>·<code>then … else</code>·<code>??</code>는 단락 평가라 오른쪽은 필요할 때만 평가한다.</p>

```fung run
fn say(x):
  print(x)
  x
;;
xs = [say(1), say(2)]
n = say(3) + say(4)
v = say(5) ?? say(6)
w = false and say(7)
print(xs, n, v, w)
```

<h3>람다의 본문</h3>
<p>람다 <code>=&gt;</code>의 본문은 식 하나이고 <strong>가능한 한 길게</strong> 읽는다. <code>then … else</code>보다도 낮으므로 <code>x =&gt; x &gt; 0 then "pos" else "neg"</code>는 조건식 전체가 본문이다. 그래서 람다는 연산자의 피연산자가 될 수 없다: 식의 한 조각으로 쓰려면 괄호로 감싼다(<code>1 + x =&gt; x</code>, <code>v |&gt; x =&gt; x + 1</code>은 E0110). 인자, 리스트 원소, 대입의 오른쪽, <code>then … else</code>의 가지에는 그대로 쓴다.</p>
<p>람다는 일찍 돌아갈 수 없다: 본문의 <code>return</code>과 <code>?</code>는 컴파일 에러 E0237이다. 넘겨받은 함수가 람다의 값을 어떻게 쓰는지에 따라 <code>?</code>가 돌려준 <code>Err</code>가 리스트의 원소가 되거나(<code>map</code>) 조건이 되어(<code>filter</code>) 말없이 섞이기 때문이다. 일찍 돌아가야 하면 익명 함수 <code>fn(x): … ;;</code>로 쓴다. 이름 있는 <code>fn</code>과 익명 <code>fn</code>은 <code>return</code>·<code>?</code>를 쓸 수 있다.</p>

```fung run
f = x => x > 0 then "pos" else "neg"
print(f(1), f(-1))
```

<h3>반환</h3>
<p>마지막 식의 값을 돌려준다. 일찍 빠져나갈 때는 <code>return 식</code>(값 없으면 <code>nil</code>). <code>return</code>은 감싸는 <code>fn</code>(이름 있는 것이나 익명)에서 돌아온다. 람다와 <code>for</code> 본문(E0237), 진입 파일 최상위, 매개변수 기본값 안에서는 쓸 수 없다(최상위에서는 <code>exit()</code>; 기본값은 본문보다 먼저 계산되므로 E0211). 함수에서 일찍 돌아가는 <code>?</code>도 같다.</p>
<p><code>return</code>은 <strong>문장</strong>이거나 <strong>가지의 끝</strong>(<code>if</code>·<code>match</code> 절의 본문, <code>then … else</code>의 가지, <code>fn f() = …</code>의 본문)이다. 연산자의 피연산자, 인자, 리스트·맵의 원소, 대입의 오른쪽에 쓰면 E0132다(<code>x = a ?? return nil</code>은 <code>if a == nil: return nil ;;</code>로). 같은 블록에서 <code>return</code> 문장 뒤의 문장은 결코 실행되지 않으므로 E0131이다.</p>
<h3>부분 적용</h3>
<p>호출 인자의 <code>_</code>는 그 자리를 비운 새 함수를 만든다. <code>f(a, _)</code>는 <code>x =&gt; f(a, x)</code>이고, <code>a</code>는 그 자리에서 한 번, 왼쪽부터 평가된다(새 함수를 부를 때마다가 아니다). 태그도 호출 형태라 같다: <code>map(xs, Ok(_))</code>. 자동 커링은 없다.</p>
<ul>
  <li><code>_</code>가 여럿이면 왼쪽부터 차례로 인자가 된다: <code>sub(_, _)</code>는 <code>(a, b) =&gt; sub(a, b)</code>.</li>
  <li>겹친 호출에서 <code>_</code>는 <strong>가장 안쪽 호출</strong>에 속한다: <code>map(xss, len(_))</code>의 <code>len(_)</code>은 <code>x =&gt; len(x)</code>다.</li>
  <li>이름 붙인 인자 자리(<code>f(a: _)</code>)에는 쓸 수 없다(E0123). 파이프 오른쪽 호출의 인자로도 쓸 수 없다(E0227, <a href="operators.html#pipe">파이프</a>).</li>
</ul>

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

<p><code>return f()</code>의 호출, <code>a ?? f()</code>의 오른쪽, <code>a and f()</code>·<code>a or f()</code>의 오른쪽도 꼬리 위치면 꼬리 호출이다(<code>and</code>·<code>or</code>의 오른쪽은 bool이어야 하는데, 그 검사는 프레임이 이어받아 값이 돌아올 때 한다). 꼬리가 아닌 것: 연산자의 피연산자(<code>1 + f()</code>), <code>f()!</code>·<code>f()?</code>·<code>f() ?? b</code>의 왼쪽, 다른 호출의 인자. 반환 타입을 적은 함수도 꼬리 호출을 한다: 꼬리 호출은 프레임을 재사용하고, 프레임이 그 함수의 반환값 검사를 이어받아 프레임이 돌아올 때 한다(검사가 실패하면 E0404가 반환 타입을 적은 함수를 가리킨다). 이어받은 검사가 이미 있으면 다시 쌓지 않으므로 서로 부르는 함수도 일정한 공간에서 돈다. 꼬리가 아닌 재귀는 깊이 100,000에서 "stack overflow" 에러다.</p>

```fung run
fn even(n) -> bool = n == 0 then true else odd(n - 1)
fn odd(n) -> bool = n == 0 then false else even(n - 1)
print(even(300_000))
```

## 경계 사례와 에러 {#edges}

인자 수가 틀리거나 없는 이름 인자를 주면 호출 자리에서 멈춘다. 호출 대상이 컴파일 때 정해지면 실행하기 전에 잡는다.

```fung run exit=2 error=E0407
fn add(a, b) = a + b
print(add(1, 2))
print(add(1))
```

변수에 든 함수 값이나 펼친 인자는 부를 때 센다.

```fung run exit=1 error=E0402
fn add(a, b) = a + b
xs = [1, 2]
print(add(...xs))
print(add(...xs + [3]))
```

펼칠 수 있는 것은 리스트와 범위뿐이고, 펼침은 마지막 위치 인자 자리에만 쓴다.

```fung run exit=1 error=E0506
print(...1..4)
print(..."abc")
```

```fung run exit=2 error=E0133
fn add(a, b) = a + b
print(add(...[1], 2))
```

같은 이름 인자를 두 번 주면 컴파일할 때 잡는다.

```fung run exit=2 error=E0116
fn f(a) = a
print(f(a: 1, a: 2))
```

꼬리가 아닌 재귀는 깊이 100,000에서 멈춘다. 꼬리 재귀나 `fold`로 쓴다.

```fung run exit=1 error=E0804
fn depth(n) = n == 0 then 0 else 1 + depth(n - 1)
print(depth(1000))
print(depth(200000))
```

부분 적용의 `_` 옆 인자는 새 함수를 만들 때 한 번 평가된다. `_`는 가장 안쪽 호출에 속한다.

```fung run
fn say(x):
  print(x)
  x
;;
fn sub(a, b) = a - b
g = sub(say(10), _)
print("made")
print(g(3), g(4), sub(_, _)(10, 3))
print(map([[1, 2], [3]], len(_)))
```

람다는 일찍 돌아가지 않는다. `return`이나 `?`가 필요하면 익명 `fn`으로 쓴다.

```fung run exit=2 error=E0237
fn ints(xs) = try_map(xs, fn(s): Ok(parse_int(s)? * 2) ;;)
print(ints(["1", "2"]))
fn bad(xs) = map(xs, s => parse_int(s)? * 2)
```

`return` 문장 뒤의 문장은 실행되지 않으므로 에러다.

```fung run exit=2 error=E0131
fn sign(n):
  if n < 0: return -1 ;;
  1
;;
print(sign(-5))
fn head(xs):
  return xs[0]
  print("never")
;;
```

이름 붙인 인자 자리에는 `_`를 쓸 수 없다.

```fung run exit=2 error=E0123
fn f(a, b) = a - b
g = f(1, b: _)
```
