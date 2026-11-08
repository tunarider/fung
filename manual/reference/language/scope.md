---
description: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
lede: 대입, 블록 스코프, 바깥 변수, 최상위 이름.
---
# 변수와 스코프

<p>대입 <code>x = 식</code>은 이름을 값에 붙인다. 같은 이름에 다시 대입할 수 있지만 값 자체를 바꾸지는 않는다. 규칙은 넷이고 예외는 없다.</p>
<ol>
  <li><strong>이름은 위에서 아래로 묶인다.</strong> 대입보다 앞에서 읽으면 컴파일 에러(E0204).</li>
  <li><strong>최상위 <code>fn</code>은 프로그램 시작 전에 모두 만들어진다.</strong> 그래서 순서와 상관없이 서로 부를 수 있다.</li>
  <li><strong>함수 값은 만들어질 때 보이는 변수의 값을 복사한다.</strong> 나중에 그 이름에 다시 대입해도 함수가 가진 값은 그대로다.</li>
  <li><strong>블록은 바깥 이름을 읽을 수 있지만 대입할 수 없다</strong>(E0205). 블록 안에서 만든 이름은 밖으로 새지 않는다(E0206).</li>
</ol>
<p>여기서 따라 나오는 것: 최상위 <code>fn</code>은 만들어질 때 최상위 변수가 아직 없으므로 복사할 것이 없다. 대신 <strong>최상위에서 한 번만 대입되는 이름은 상수라서 최상위 <code>fn</code>이 부를 때 읽는다</strong>(다른 최상위 <code>fn</code>과 모듈이 보이는 것과 같은 이유다). 모듈 상태는 <code>counter = atom(0)</code>처럼 둔다. 두 번 이상 대입되는 이름은 보이지 않으니 인자로 넘긴다. 최상위 <code>fn</code>이 자기 본문에서 묶는 이름은 그 함수의 지역이고 같은 이름의 상수를 가린다. 파일이 시작하는 중에 상수의 대입보다 먼저 그것을 읽는 <code>fn</code>을 부르면 E0809다. 람다와 익명 함수는 그 자리에서 만들어지므로 그때까지의 변수를 복사한다.</p>

```fung frag
dir = "./logs"
fn clean() = fs.list(dir)       # 된다: dir은 한 번만 대입되는 상수
level = 1
level = 2
fn show() = level               # E0204: level은 두 번 대입된다. 인자로 넘긴다

total = 0
for f in files:
  total = total + 1             # E0205: 블록 안에서 바깥 변수에 대입할 수 없다
;;
total = len(files)              # 값을 식으로 만든다
```

<p><strong>값의 수명.</strong> 함수 안의 변수(와 진입 파일·모듈 본문의 블록 안에서 묶은 이름)를 마지막으로 읽으면 값이 읽은 쪽으로 넘어가고, 그 길에서 더 읽힐 수 없게 된 첫 문장이 끝나면 변수는 값을 놓는다(람다가 붙잡는 것도 읽기다). 함수 본문의 문장이든 <code>if</code>·<code>match</code> 가지 안의 문장이든 같다. 그래서 홀로 쥔 맵·리스트를 받은 <code>put</code>·<code>+</code>는 복사하지 않고 제자리에서 고친다(<code>fold</code>로 쌓는 맵이 빠른 이유). 그래서 함수 안에서 만든 채널의 보내는 끝을 생산 태스크에 넘기면, 생산이 끝날 때 채널이 닫힌다. 한 번도 읽지 않는 변수(<code>_watch = stop_signal()</code>)는 함수가 돌아올 때까지 산다. 쥐고 있는 것 자체가 뜻일 때 쓴다. 최상위 변수는 프로그램 끝까지 산다.</p>

## 경계 사례와 에러 {#edges}

정의하기 전에 읽으면 에러다.

```fung run exit=2 error=E0204
print(x)
x = 1
```

블록 안에서는 바깥 변수를 바꾸지 못한다. 블록의 값을 받아 대입한다.

```fung run exit=2 error=E0205
total = 0
for x in [1, 2]:
  total = total + x
;;
print(total)
```

블록 안에서 만든 이름은 블록 밖에서 보이지 않는다.

```fung run exit=2 error=E0206
if true:
  inner = 1
  inner
;;
print(inner)
```

대문자로 시작하는 이름은 태그라서 변수 이름이 될 수 없다.

```fung run exit=2 error=E0225
Count = 1
print(Count)
```
