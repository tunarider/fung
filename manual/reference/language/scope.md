---
description: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
lede: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
---
# 변수와 스코프

<p>대입 <code>x = 식</code>은 이름을 값에 붙인다. 붙인 이름은 바뀌지 않는다. 규칙은 여섯이다.</p>
<ol>
  <li><strong>이름은 위에서 아래로 묶인다.</strong> 대입보다 앞에서 읽으면 컴파일 에러(E0204). prelude 함수의 이름을 묶은 블록에서는 그 블록 전체와 안쪽 블록에서 그 이름이 묶은 것을 뜻하므로, 대입보다 앞에서 읽어도 prelude 함수가 아니라 E0204다(<code>lines = lines(s)</code>도).</li>
  <li><strong>보이는 이름은 다시 묶을 수 없다.</strong> 이름은 한 번만 묶인다. 같은 블록에서 이미 묶은 이름이든, 바깥 블록이나 바깥 함수의 이름, 최상위 이름(상수·<code>fn</code>), import한 이름과 모듈 이름이든, 보이는 이름을 다시 묶으면 묶는 방법(대입, 구조 분해, <code>fn</code>·람다의 매개변수, <code>for</code>의 변수, <code>match</code> 절의 패턴, 지역 <code>fn</code>, 최상위 <code>fn</code>, import)과 상관없이 컴파일 에러 E0238이다. 파일의 맨 바깥에서도 함수 본문에서도 같다. 그래서 이름은 모두 상수이고, 한 자리에서 이름 하나는 한 가지만 뜻한다. 노트가 그 이름이 어디서 왔는지(같은 블록의 몇째 줄, 블록 바깥, 최상위 이름, <code>fn</code>, import, 모듈) 알려 주고, 도움말이 무엇을 쓸지 알려 준다(반복문 안의 <code>total = total + 1</code>이면 식이나 <code>fold</code>, 모듈 이름이면 <code>import … as</code>). 다음 값에는 새 이름을 붙이거나, 단계들을 식 하나(파이프, <code>if</code>, <code>fold</code>)로 쓴다. REPL의 최상위만 입력마다 다시 정의할 수 있다. prelude 함수는 바깥에서 묶은 이름이 아니라 어느 블록에도 묶이지 않은 이름이라, 그 이름은 어느 자리에서나(지역 변수, 매개변수, 패턴, <code>for</code> 변수, 지역·최상위 <code>fn</code>, import 목록) 묶을 수 있고 묶은 블록에서 prelude 함수를 가린다(1번). 같은 모듈을 두 번 import하는 것은 이름이 아니라 모듈의 규칙이라 따로 E0309다.</li>
  <li><strong>최상위 <code>fn</code>은 프로그램 시작 전에 모두 만들어진다.</strong> 그래서 순서와 상관없이 서로 부를 수 있다.</li>
  <li><strong>함수 값은 만들어질 때 보이는 변수의 값을 가진다.</strong> 이름이 한 번만 묶이므로 그 값은 함수를 언제 불러도 같다.</li>
  <li><strong>블록은 바깥 이름을 읽을 수 있고, 블록 안에서 만든 이름은 밖으로 새지 않는다</strong>(E0206). <code>if</code> 가지와 <code>match</code> 절이 블록이고, 절의 패턴이 묶는 이름도 그 블록의 이름이다. <code>for</code>의 본문은 <code>map</code>·<code>each</code>에 넘길 함수의 본문이라(<a href="for.html">for</a>) 이름이 밖으로 새지 않는 점에서 블록과 같고, <code>for</code>의 변수는 그 함수의 매개변수다.</li>
  <li><strong>묶은 이름은 읽어야 한다.</strong> 한 번도 읽지 않는 지역 변수, 매개변수(람다·<code>for</code>·<code>match</code> 패턴이 묶는 이름 포함), import는 컴파일 에러(E0239)다. 파일의 최상위 이름(변수와 <code>fn</code>)은 세지 않는다: <code>_</code>로 시작하지 않는 최상위 이름은 그 파일의 공개 이름, 곧 다른 파일이 읽을 인터페이스다. 파일을 직접 실행하든 import하든 같다. 일부러 쥐기만 하는 이름은 <code>_</code>로 시작한다(<code>_watch = stop_signal()</code>, <code>(a, _b) = pair</code>). 다른 에러가 있으면 E0239는 그것을 고친 뒤에 나온다(쓰지 않는 이름은 다른 에러의 결과일 때가 많다).</li>
</ol>
<p>여기서 따라 나오는 것: 최상위 <code>fn</code>은 만들어질 때 최상위 변수가 아직 없으므로 복사할 것이 없다. 대신 <strong>최상위 이름은 모두 상수라서 최상위 <code>fn</code>이 부를 때 읽는다</strong>(다른 최상위 <code>fn</code>과 모듈이 보이는 것과 같은 이유다). 모듈 상태는 <code>counter = atom(0)</code>처럼 둔다. 상수는 최상위 <code>fn</code> 안 어디서나 보이므로 그 이름을 함수 안에서 다시 묶을 수 없다(본문의 맨 바깥 블록에서도, 매개변수나 <code>for</code> 변수로도 E0238). 파일이 시작하는 중에 상수의 대입보다 먼저 그것을 읽는 <code>fn</code>을 부르면 E0809다. 람다와 익명 함수는 그 자리에서 만들어지므로 그때까지의 변수를 복사한다.</p>

```fung frag
dir = "./logs"
fn clean() = fs.list(dir)       # 된다: 최상위 이름 dir은 상수
level = 1
level = 2                       # E0238: 이름은 한 번만 묶인다

total = 0
for f in files:
  total = total + 1             # E0238: 블록 안에서 바깥 변수에 대입할 수 없다
;;
total = len(files)              # 값을 식으로 만든다

fn count(dir) = len(fs.list(dir)!)  # E0238: 매개변수 dir이 상수 dir을 가린다
for len in [1, 2]: len ;;       # 된다: prelude 함수 len을 이 본문에서 가린다
lines = lines(text)             # E0204: 이 블록의 lines는 이 대입이 묶는 이름이다
```

<p><strong>함수 이름.</strong> 최상위 <code>fn</code>의 이름은 상수다: 같은 파일에 같은 이름의 <code>fn</code>을 둘 두거나 그 이름에 대입하면 E0238이다(REPL만 다시 정의할 수 있다. fung에는 인자 개수로 함수를 고르는 오버로딩이 없다). 블록 안의 지역 <code>fn</code> 이름도 같다. 블록 안의 지역 <code>fn</code>은 그 자리에서 만들어지는 함수 값이라 4번 규칙대로 바깥 인자와 지역 변수를 복사하고, 자기 자신과 앞에서 정의한 이름만 부를 수 있다. 같은 블록의 뒤에서 정의하는 지역 <code>fn</code>을 부르면 E0208이다. 서로 부르는 함수들은 최상위에 둔다.</p>

<p><strong>값의 수명.</strong> 변수가 값을 언제 놓는지는 프로그램의 결과를 바꾸지 않는다. 홀로 쥔 맵·리스트를 제자리에서 고치는 빠르기와, 읽지 않은 태스크의 <code>Err</code>(E0607)가 보고되는 때, 닫지 않은 파일이 닫히는 때에만 보인다. 규칙은 VM 책의 <a href="../../vm/10-values-and-memory.html#lifetime">지역 변수의 수명</a>에 있다. 채널은 <code>close</code>로만 닫히므로(<a href="../prelude/concurrency.html">동시성</a>) 놓는 시점과 상관없다.</p>

## 경계 사례와 에러 {#edges}

정의하기 전에 읽으면 에러다.

```fung run exit=2 error=E0204
print(x)
x = 1
```

이름은 한 번만 묶인다. 바뀐 값에는 새 이름을 붙이거나 식 하나로 계산한다.

```fung run exit=2 error=E0238
count = 1
total = count + 1
print(total)
count = count + 1
```

블록 안에서는 바깥 변수를 바꾸지 못한다. 블록의 값을 받아 대입한다.

```fung run exit=2 error=E0238
total = 0
for x in [1, 2]:
  total = total + x
;;
print(total)
```

최상위 `fn` 안도 같다. 상수 `limit`는 함수 안 어디서나 보이므로 함수 본문의 맨 바깥 블록에서도 다시 대입할 수 없다. 새 값에는 새 이름을 붙인다.

```fung run exit=2 error=E0238
limit = 10
fn clamp(n):
  limit = n > limit then n else limit
  limit
;;
print(clamp(3))
```

보이는 이름은 묶는 방법과 상관없이 다시 묶을 수 없다. 매개변수, 람다의 매개변수, `for`의 변수, `match` 절의 패턴도 같다.

```fung run exit=2 error=E0238
fn shout(word) = upper(word) + "!"
word = "a"
print(shout(word))
```

```fung run exit=2 error=E0238
fn first_word(line):
  match split(line, " "):
    [line, ...] => line
    []          => ""
  ;;
;;
print(first_word("a b"))
```

prelude 함수의 이름은 묶을 수 있다. 묶은 블록에서는 그 이름이 묶은 것이라, 대입보다 앞에서 읽으면 prelude 함수가 아니라 E0204다.

```fung run
fn total(xs):
  sum = 0
  sum + len(xs)
;;
print(total([1]))
```

```fung run exit=2 error=E0204
fn words(text):
  lines = lines(text)
  len(lines)
;;
print(words("a\nb"))
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

```fung run exit=2 error=E0238
fn f() = 1
fn f() = 2
print(f())
```

```fung run exit=2 error=E0238
fn f() = 1
f = 2
print(f)
```

함수 안에서도 이름은 한 번만 묶인다. 다음 값에는 새 이름을 붙인다.

```fung run exit=2 error=E0238
fn total(xs):
  n = len(xs)
  n = n * 2
  n + 1
;;
print(total([1, 2]))
```

```fung run
fn total(xs):
  n = len(xs)
  doubled = n * 2
  doubled + 1
;;
print(total([1, 2]))
```


