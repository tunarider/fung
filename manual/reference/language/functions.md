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
  <li>호출할 때 위치 인자 뒤에 이름 붙인 인자를 쓴다: <code>exec(cmd, cwd: dir)</code>. 내장 함수도 똑같다.</li>
  <li>인자 개수가 맞지 않거나, 같은 인자를 두 번 주거나, 모르는 이름이면 런타임 에러.</li>
  <li>매개변수 자리에 구조 분해 패턴(<code>_</code>, 튜플, 리스트, 맵, 태그)을 쓸 수 있다: <code>fn dist((x, y)) = …</code>, <code>pairs(m) |&gt; map(((k, v)) =&gt; k)</code>. <code>fn</code>과 람다가 같고, 리터럴(<code>(1) =&gt; …</code>)은 매개변수가 될 수 없다(값으로 고르려면 <code>match</code>).</li>
</ul>

```fung run
fn greet(name, greeting = "hello", ...extra) = "${greeting}, ${name}${join(extra)}"
print(greet("fung"), greet("fung", greeting: "hi"), greet("fung", "yo", "!", "?"))
```

<h3>반환</h3>
<p>마지막 식의 값을 돌려준다. 일찍 빠져나갈 때는 <code>return 식</code>(값 없으면 <code>nil</code>). <code>return</code>은 감싸는 함수나 람다에서 돌아오고, <code>for</code> 본문과 진입 파일 최상위에서는 쓸 수 없다(최상위에서는 <code>exit()</code>).</p>
<h3>부분 적용</h3>
<p>호출 인자의 <code>_</code>는 그 자리를 비운 새 함수를 만든다. <code>f(a, _)</code>는 <code>x =&gt; f(a, x)</code>이고, <code>a</code>는 그 자리에서 한 번 평가된다. 태그도 호출 형태라 같다: <code>map(xs, Ok(_))</code>. 자동 커링은 없다. 파이프 오른쪽 호출의 인자로는 쓸 수 없다(E0227, <a href="operators.html#pipe">파이프</a>).</p>

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

## 경계 사례와 에러 {#edges}

인자 수가 틀리거나 없는 이름 인자를 주면 호출 자리에서 멈춘다.

```fung run exit=1 error=E0407
fn add(a, b) = a + b
print(add(1, 2))
print(add(1))
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
