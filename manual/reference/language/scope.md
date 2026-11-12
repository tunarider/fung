---
description: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
lede: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
---
# 변수와 스코프

<p>대입 <code>x = 식</code>은 이름을 값에 붙인다. 블록(함수 본문, <code>if</code> 가지 등) 안에서는 같은 블록의 이름에 다시 대입할 수 있지만 값 자체를 바꾸지는 않는다. 규칙은 일곱이다.</p>
<ol>
  <li><strong>이름은 위에서 아래로 묶인다.</strong> 대입보다 앞에서 읽으면 컴파일 에러(E0204).</li>
  <li><strong>최상위 이름은 한 번만 묶인다.</strong> 파일의 맨 바깥에서 이미 묶은 이름에 다시 대입하거나 패턴으로 다시 묶으면 컴파일 에러(E0238)다. 그래서 최상위 이름은 모두 상수다(REPL만 입력마다 다시 정의할 수 있다).</li>
  <li><strong>최상위 <code>fn</code>은 프로그램 시작 전에 모두 만들어진다.</strong> 그래서 순서와 상관없이 서로 부를 수 있다.</li>
  <li><strong>함수 값은 만들어질 때 보이는 변수의 값을 복사한다.</strong> 나중에 그 이름에 다시 대입해도 함수가 가진 값은 그대로다.</li>
  <li><strong>블록은 바깥 이름을 읽을 수 있지만 대입할 수 없다</strong>(E0205). 블록 안에서 만든 이름은 밖으로 새지 않는다(E0206). <code>if</code> 가지, <code>match</code> 절, <code>for</code>가 모두 블록이고, 절의 패턴과 <code>for</code>의 변수가 묶는 이름도 그 블록의 이름이다.</li>
  <li><strong>보이는 이름은 다시 묶을 수 없다.</strong> 바깥 블록이나 바깥 함수의 이름, 최상위 이름(상수·<code>fn</code>), import한 이름, prelude 함수와 같은 이름을 새로 묶으면 묶는 방법(대입, 구조 분해, <code>fn</code>·람다의 매개변수, <code>for</code>의 변수, <code>match</code> 절의 패턴, 지역 <code>fn</code>, 최상위 <code>fn</code>, import)과 상관없이 컴파일 에러다. 바깥 변수에 대입하는 것은 5번 규칙의 E0205이고, 나머지는 E0240이다(어디서 온 이름인지 노트로 알려 준다). 모듈 이름은 E0308이다. 그래서 한 자리에서 이름 하나는 한 가지만 뜻한다.</li>
  <li><strong>묶은 이름은 읽어야 한다.</strong> 한 번도 읽지 않는 지역 변수, 매개변수(람다·<code>for</code>·<code>match</code> 패턴이 묶는 이름 포함), import는 컴파일 에러(E0239)다. 다시 대입하기만 하는 것은 읽는 것이 아니다. 진입 파일의 최상위 이름도 지역으로 센다(아무도 import하지 않으므로). import된 파일의 공개 이름은 그 모듈의 인터페이스라 세지 않고, 최상위 <code>fn</code>은 변수가 아니라 세지 않는다. 일부러 쥐기만 하는 이름은 <code>_</code>로 시작한다(<code>_watch = stop_signal()</code>, <code>(a, _b) = pair</code>). 다른 에러가 있으면 E0239는 그것을 고친 뒤에 나온다(쓰지 않는 이름은 다른 에러의 결과일 때가 많다).</li>
</ol>
<p>여기서 따라 나오는 것: 최상위 <code>fn</code>은 만들어질 때 최상위 변수가 아직 없으므로 복사할 것이 없다. 대신 <strong>최상위 이름은 모두 상수라서 최상위 <code>fn</code>이 부를 때 읽는다</strong>(다른 최상위 <code>fn</code>과 모듈이 보이는 것과 같은 이유다). 모듈 상태는 <code>counter = atom(0)</code>처럼 둔다. 상수는 최상위 <code>fn</code> 안 어디서나 보이므로 그 이름을 함수 안에서 다시 묶을 수 없다(본문의 맨 바깥 블록에서도 E0205, 매개변수나 <code>for</code> 변수로는 E0240). 파일이 시작하는 중에 상수의 대입보다 먼저 그것을 읽는 <code>fn</code>을 부르면 E0809다. 람다와 익명 함수는 그 자리에서 만들어지므로 그때까지의 변수를 복사한다.</p>

```fung frag
dir = "./logs"
fn clean() = fs.list(dir)       # 된다: 최상위 이름 dir은 상수
level = 1
level = 2                       # E0238: 최상위 이름은 한 번만 묶인다

total = 0
for f in files:
  total = total + 1             # E0205: 블록 안에서 바깥 변수에 대입할 수 없다
;;
total = len(files)              # 값을 식으로 만든다

fn count(dir) = len(fs.list(dir)!)  # E0240: 매개변수 dir이 상수 dir을 가린다
for len in [1, 2]: len ;;       # E0240: prelude 함수 len을 가린다
```

<p><strong>함수 이름.</strong> 최상위 <code>fn</code>의 이름은 상수다: 같은 파일에 같은 이름의 <code>fn</code>을 둘 두면 E0210, 그 이름에 대입하면 E0209다(REPL만 다시 정의할 수 있다). 블록 안의 지역 <code>fn</code>은 그 자리에서 만들어지는 함수 값이라 4번 규칙대로 바깥 인자와 지역 변수를 복사하고, 자기 자신과 앞에서 정의한 이름만 부를 수 있다. 같은 블록의 뒤에서 정의하는 지역 <code>fn</code>을 부르면 E0208이다. 서로 부르는 함수들은 최상위에 둔다.</p>

<p><strong>값의 수명.</strong> 함수 안의 변수(와 진입 파일·모듈 본문의 블록 안에서 묶은 이름)를 마지막으로 읽으면 값이 읽은 쪽으로 넘어가고, 그 길에서 더 읽힐 수 없게 되면 변수는 값을 놓는다: 그 문장이 끝날 때, 가지(<code>if</code>·<code>else</code>, <code>??</code>·<code>and</code>·<code>or</code>의 양쪽, 패턴과 가드를 지난 <code>match</code> 갈래)가 시작할 때(다른 가지나 가드에서만 읽히는 변수, <code>match</code>의 주제), 람다가 붙잡는 것이 마지막 읽기면 람다를 만든 바로 뒤(붙잡기도 읽기다). 함수 본문이든 가지 안이든, 문장이 하나뿐인 가지든 같다. 그래서 홀로 쥔 맵·리스트를 받은 <code>put</code>·<code>+</code>는 복사하지 않고 제자리에서 고친다(<code>fold</code>로 쌓는 맵이 빠른 이유). 그래서 함수 안에서 만든 채널의 보내는 끝을 생산 태스크에 넘기면, 생산이 끝날 때 채널이 닫힌다. 한 번도 읽지 않는 변수(<code>_watch = stop_signal()</code>)는 함수가 돌아올 때까지 산다. 쥐고 있는 것 자체가 뜻일 때 쓴다. 최상위 변수는 프로그램 끝까지 산다.</p>

## 경계 사례와 에러 {#edges}

정의하기 전에 읽으면 에러다.

```fung run exit=2 error=E0204
print(x)
x = 1
```

최상위 이름은 한 번만 묶인다. 바뀐 값에는 새 이름을 붙이거나 식 하나로 계산한다.

```fung run exit=2 error=E0238
count = 1
total = count + 1
print(total)
count = count + 1
```

블록 안에서는 바깥 변수를 바꾸지 못한다. 블록의 값을 받아 대입한다.

```fung run exit=2 error=E0205
total = 0
for x in [1, 2]:
  total = total + x
;;
print(total)
```

최상위 `fn` 안도 같다. 상수 `limit`는 함수 안 어디서나 보이므로 함수 본문의 맨 바깥 블록에서도 다시 대입할 수 없다. 새 값에는 새 이름을 붙인다.

```fung run exit=2 error=E0205
limit = 10
fn clamp(n):
  limit = n > limit then n else limit
  limit
;;
print(clamp(3))
```

보이는 이름은 묶는 방법과 상관없이 다시 묶을 수 없다. 매개변수, 람다의 매개변수, `for`의 변수, `match` 절의 패턴도 같다.

```fung run exit=2 error=E0240
fn shout(word) = upper(word) + "!"
word = "a"
print(shout(word))
```

```fung run exit=2 error=E0240
fn first_word(line):
  match split(line, " "):
    [line, ...] => line
    []          => ""
  ;;
;;
print(first_word("a b"))
```

prelude 함수의 이름도 다시 묶을 수 없다. 이름이 겹치면 다른 이름을 짓는다.

```fung run exit=2 error=E0240
fn total(xs):
  sum = 0
  sum + len(xs)
;;
print(total([1]))
```

한 번도 읽지 않는 이름은 에러다. 일부러 쥐기만 하는 이름은 `_`로 시작한다.

```fung run exit=2 error=E0239
fn area(w, h) = w * w
print(area(2, 3))
```

```fung run
fn area(w, _h) = w * w
(name, _size) = ("a.txt", 3)
print(area(2, 3), name)
```

블록 안에서 만든 이름은 블록 밖에서 보이지 않는다.

```fung run exit=2 error=E0206
if true:
  inner = 1
  print(inner)
;;
print(inner)
```

`for`의 변수도 `for`가 끝나면 사라진다.

```fung run exit=2 error=E0206
for line in ["a", "b"]:
  print(line)
;;
print(line)
```

대문자로 시작하는 이름은 태그라서 변수 이름이 될 수 없다.

```fung run exit=2 error=E0225
Count = 1
print(Count)
```

지역 `fn`은 바깥 인자를 복사하고 자기 자신을 부를 수 있지만, 같은 블록의 뒤에서 정의하는 `fn`은 부를 수 없다.

```fung run
fn times(s, n):
  fn go(i) = i == 0 then "" else s + go(i - 1)
  go(n)
;;
print(times("ab", 3))
```

```fung run exit=2 error=E0208
fn outer():
  fn a() = b()
  fn b() = 1
  a()
;;
print(outer())
```

최상위 `fn`의 이름은 상수라서 다시 정의하거나 대입할 수 없다.

```fung run exit=2 error=E0210
fn f() = 1
fn f() = 2
print(f())
```

```fung run exit=2 error=E0209
fn f() = 1
f = 2
print(f)
```

가지는 다른 가지에서만 읽히는 변수를 시작할 때 놓는다. `else` 쪽은 보내는 끝을 쥐지 않으므로 채널이 닫혀 `for`가 끝난다.

```fung run
fn drain(flag):
  (tx, rx) = channel(1)
  if flag:
    send(tx, 1)
  else:
    for x in rx: x ;;
  ;;
;;
print(drain(false))
```
