---
description: 함수와 클로저. 호출이 프레임을 쌓는 방법, 인자를 칸에 묶는 방법, 클로저가 바깥 변수를 붙잡는 방법(fung은 복사한다), 최상위 함수와 전역, 꼬리 호출과 스택 한도.
lede: 함수를 부르면 VM 안에서는 프레임 하나가 쌓인다. 함수 안에서 만든 함수는 바깥 변수를 붙잡아야 하는데, fung은 그 값을 만들어질 때 복사하는 규칙 하나로 정했다.
---
# 함수와 클로저

<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li>코어</li><li>바이트코드</li><li class="on">실행</li></ol>

:::goals
- 함수 호출이 하는 일: 돌아올 자리, 인자, 지역 변수, 그리고 이것을 담는 프레임
- 인자를 칸에 묶는 호출 규약과 기본값이 바이트코드로 바뀌는 모습
- 클로저(closure)가 바깥 변수를 붙잡는 여러 방법과 fung이 고른 "복사"
- 최상위 함수가 전역 변수를 보는 규칙과 그 구현상 이유
- 꼬리 호출(tail call)이 스택을 쓰지 않는 원리와 깊이 한도
:::

8장에서 VM이 명령을 하나씩 읽어 값 스택에 쌓고 꺼내는 모습을 봤다. 그때는 `CALL`을 "함수를 부른다"로만 읽고 넘어갔다. 이 장은 그 한 명령 안에서 일어나는 일을 연다.

## 함수 호출은 무엇을 하나 {#calls}

함수를 부르는 일은 생각보다 많은 것을 기억해야 한다.

1. **돌아올 자리.** 함수가 끝나면 부른 쪽의 다음 명령으로 돌아가야 한다.
2. **인자.** 부른 쪽이 넘긴 값을 받는 쪽의 매개변수 이름에 붙여야 한다.
3. **지역 변수.** 함수 안에서 만든 변수는 그 호출에만 속한다. 같은 함수가 재귀로 두 번 돌면 변수도 두 벌이다.

이 셋을 묶은 것을 **프레임**(frame, 활성 레코드라고도 한다)이라고 한다. 함수를 부를 때마다 프레임이 하나 쌓이고, 돌아올 때 하나 빠진다. 쌓인 프레임 전체를 **호출 스택**(call stack)이라고 부른다. 에러 메시지의 "call path"가 바로 이 스택을 위에서부터 읽은 것이다.

책 내내 따라온 예를 다시 보자.

```dump bytecode
fn double(x) = x * 2
print(double(21))
```

`<main>`의 0005번 `CALL 1`이 실행되기 직전, 값 스택에는 아래에서부터 `<fn print>`, `double`, `21`이 쌓여 있다. `CALL 1`은 "맨 위 인자 하나와 그 아래의 함수"를 뜻한다. VM은 `double`의 프레임을 새로 쌓고, 그 함수의 명령 0000번부터 실행한다.

```plain
CALL 1 직전                 double의 프레임이 쌓인 뒤
[2]  21
[1]  <fn double>            [1]  21           ← double 프레임의 슬롯 0(x), base = 1
[0]  <fn print>             [0]  <fn print>   (main이 계산 중인 값)
```

VM은 함수 값이 있던 칸을 빼고, 그 자리부터를 새 프레임의 슬롯으로 삼는다. `double` 안의 `LOADLOCAL 0`은 "내 프레임의 슬롯 0"을 읽는다. 슬롯 0은 값 스택의 `base + 0`번 칸이고, 인자 `21`이 이미 그 자리에 있으므로 따로 복사할 필요가 없다. `MUL`이 42를 올리고, `RETURN`이 42를 꺼낸 뒤 스택을 `base`까지 잘라 내고 42를 부른 쪽 스택에 올린다. 함수가 남긴 임시 값은 이렇게 새지 않는다. 그다음 `<main>`의 `CALL 1`이 다시 `print`를 부른다.

```fung run
fn double(x) = x * 2
print(double(21))
```

## 프레임 {#frames}

fung의 프레임은 Rust 구조체 하나다(`crates/fung-vm/src/vm/frame.rs`의 `Frame`).

<table>
  <thead><tr><th>필드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>proto</code></td><td>실행할 함수의 원형(8장): 명령열, 상수, 매개변수 표</td></tr>
    <tr><td><code>ip</code></td><td>다음에 실행할 명령의 번호. "돌아올 자리"는 부른 쪽 프레임의 이 값이다</td></tr>
    <tr><td><code>base</code></td><td>이 함수의 슬롯이 값 스택의 몇 번째 칸부터인가</td></tr>
    <tr><td><code>closure</code></td><td>실행 중인 함수 값. 붙잡은 값(아래 클로저 절)을 여기서 읽는다</td></tr>
    <tr><td><code>globals</code></td><td>이 함수가 속한 모듈의 전역 표(11장)</td></tr>
    <tr><td><code>tail_calls</code></td><td>꼬리 호출로 이 프레임을 몇 번 다시 썼나(아래 꼬리 호출 절)</td></tr>
  </tbody>
</table>

프레임들은 태스크의 `frames`라는 Rust `Vec`에 들어 있고, 지역 변수와 계산 중인 값은 모두 태스크의 값 스택 `stack` 한 줄에 쌓인다. 둘 다 힙(heap)에 있는 배열이다. 그래서 fung 함수가 fung 함수를 부를 때 Rust 함수 호출은 일어나지 않는다. 실행 루프는 반복문 하나이고, `CALL`은 배열에 원소를 하나 더할 뿐이다. 이 사실이 뒤의 꼬리 호출, 그리고 12장의 스케줄러를 가능하게 한다.

### 호출 규약 {#calling-convention}

**호출 규약**(calling convention)은 "부르는 쪽이 인자를 어디에 어떤 순서로 놓고, 받는 쪽이 그것을 어떻게 가져가는가"의 약속이다. fung의 `CALL n`은 스택에 `[함수, 위치 인자…, (이름 심볼, 값)…]`이 쌓여 있다고 약속한다. 피연산자 하나에 수 둘을 담는데, 아래 12비트가 위치 인자 수, 그 위 12비트가 이름 인자 수다. 그래서 이름 인자 하나가 있으면 피연산자가 4097(= 1 + 4096)이 된다.

```dump bytecode
fn greet(name, greeting = "hello") = "${greeting}, ${name}"

print(greet("fung"))
print(greet("fung", greeting: "hi"))
```

VM이 인자를 묶는 길은 둘이다(`crates/fung-vm/src/vm/call.rs`의 `call_op`).

- **빠른 길.** fung 함수를 위치 인자만으로, 매개변수 개수만큼 정확히 부르면 스택에 놓인 인자가 그 자리에서 새 프레임의 첫 슬롯이 된다. 나머지 지역 변수 칸은 비워 둔 채 이어 붙인다. 힙 할당이 없다. `double(21)`이 이 길이다.
- **일반 길.** 이름 인자, 기본값, `...rest`가 있거나 개수가 맞지 않으면 `bind_args`가 매개변수 표를 보고 칸을 새로 배치한다. 위치 인자는 차례로, 이름 인자는 표에서 찾아 넣고, 남는 위치 인자는 `...rest` 리스트로 모은다. 맞지 않으면 E0402(인자 수가 맞지 않음), E0405(같은 인자 두 번), E0406(없는 이름), E0407(빠진 인자)이다.

```fung run exit=1 error=E0407
fn f(a, b) = a + b

print(f(1))
```

내장 함수도 같은 매개변수 표를 선언하므로(`NativeParam`) 같은 `bind_args`를 지난다. `exec(cmd, cwd: dir)`처럼 내장 함수에 이름 인자를 줄 수 있고, 에러 번호도 fung 함수와 같다.

### 기본값은 함수 서두에서 {#defaults}

위 `greet`의 바이트코드를 다시 보자. 0000~0003번은 본문이 아니라 **서두**(prologue)다. "슬롯 1이 비었으면 `"hello"`를 넣어라"를 명령으로 적은 것이다.

- `LOADLOCAL 1`이 슬롯을 읽고, `JUMPIFSET 4`가 "칸이 차 있으면 4번으로 건너뛰라"고 한다.
- 비어 있으면 `CONST`와 `STORELOCAL 1`이 기본값을 채운다.

여기서 "비어 있음"은 fung의 `nil`이 아니다. 값 스택의 칸은 Rust의 `Option<Value>`이고, 아직 아무것도 들어가지 않은 칸은 `None`, fung의 `nil`은 `Some(Value::Nil)`이다. `greet("x", greeting: nil)`처럼 `nil`을 일부러 넘기면 기본값이 쓰이지 않는다. 기본값 식이 호출할 때마다 다시 평가되고 앞 매개변수를 참조할 수 있는 것도 기본값이 그냥 서두의 명령이기 때문이다. 타입 표기가 있으면 서두 끝에 `CHECKTYPE`이, 반환 타입이 있으면 `RETURN` 앞에 `CHECKRETURN`이 붙는다.

## 클로저 {#closures}

이제 이 장의 중심이다. 함수 안에서 함수를 만들면 무슨 일이 생기나.

```fung run
fn adder(n) = x => x + n

add5 = adder(5)
print(add5(1))
print(map([1, 2, 3], adder(10)))
```

람다 `x => x + n` 안의 `n`은 람다 자신의 매개변수가 아니다. 바깥 함수 `adder`의 매개변수다. 이런 이름을 **자유 변수**(free variable)라고 한다. 문제는 `adder(5)`가 이미 돌아왔다는 점이다. `adder`의 프레임은 사라졌고 `n`이 있던 슬롯도 다른 값이 덮었다. 그런데 `add5(1)`은 여전히 5를 알아야 한다.

그래서 함수 값은 코드만이 아니라 **자유 변수를 함께 들고 다녀야** 한다. 코드와 그것이 본 바깥 변수를 묶은 값을 **클로저**(closure)라고 한다. 언어를 만들 때 정해야 하는 것은 "바깥 변수를 어떻게 들고 다니나"이고, 답에 따라 언어의 의미가 달라진다.

- **변수를 공유한다.** 클로저가 바깥 변수의 "자리"를 가리킨다. 나중에 바깥에서 그 변수를 바꾸면 클로저도 바뀐 값을 본다. 반대로 클로저가 바꾸면 바깥도 본다.
- **값을 복사한다.** 클로저가 만들어지는 순간의 값을 자기 안에 복사해 둔다. 그 뒤 바깥 변수가 무엇이 되든 클로저는 처음 값을 본다.

:::box other 다른 언어는
- **Lua**는 공유한다. 바깥 변수를 **upvalue**라는 작은 상자로 가리킨다. 바깥 함수가 살아 있는 동안 상자는 바깥 프레임의 칸을 가리키다가(열린 upvalue), 바깥 함수가 돌아오면 그 값을 상자 안으로 옮긴다(닫힌 upvalue). 같은 변수를 붙잡은 클로저 여럿은 상자 하나를 나눠 쓴다. 이 방식은 *Crafting Interpreters*의 clox도 그대로 쓴다.
- **CPython**도 공유한다. 바깥 변수는 cell 객체에 담기고, 안쪽 함수는 그 cell을 가리킨다. 안쪽에서 바꾸려면 `nonlocal`을 적어야 한다.
- **JavaScript**도 공유한다. 그래서 유명한 함정이 있다. 예전의 `var`로 반복문을 돌며 콜백을 만들면, 모든 콜백이 변수 하나를 공유해 마지막 값만 본다. `let`이 반복마다 새 변수를 만들게 된 것도 이 때문이다.
- **Java**의 람다는 사실상 복사다. 다만 붙잡는 변수가 "사실상 final"(대입이 한 번뿐)이어야 한다는 조건을 컴파일러가 강제해서, 공유와 복사의 차이가 드러날 일 자체를 막는다.
:::

:::box choice fung의 선택: 만들어질 때 보이는 값을 복사한다
fung의 규칙은 한 문장이다. **함수는 만들어질 때 보이는 변수의 값을 복사한다**(설계 03, "모델 C"). 이 규칙 하나가 함수 정의, 람다, 최상위, 블록 어디서나 같다.

- 처음 구현은 "최상위 `fn`은 최상위 변수를 호출할 때 찾는다"(C·Python식)였다. 그러면 같은 코드가 최상위에서와 함수 안에서 답이 달랐다. 설계 원칙 7(규칙 하나에서 결과가 따라 나오고 예외 조항이 적을 것)에 따라 규칙을 하나로 합쳤다(`background.md`의 기각된 대안, 노트 M3).
- 복사라서 Lua의 upvalue 상자가 필요 없다. fung에서 `x = 2`는 이름을 새 값에 붙이는 일이지 값을 고치는 일이 아니다. 블록은 바깥 변수를 다시 대입할 수 없고(E0205) 함수는 전역에 대입할 수 없으므로, 클로저가 가진 값을 누가 몰래 바꿀 길이 없다. 상자를 두어도 그 안이 바뀌는 것을 볼 사람이 없다(노트 M3).
- 공유 상태가 정말 필요하면 이름 붙은 장치 하나, `atom`(12장)을 쓴다.
:::

복사의 결과를 직접 보자.

```fung run
fn make():
  x = 1
  f = () => x
  x = 2
  [f(), x]
;;

print(make())
```

`f`가 만들어진 순간 `x`는 1이었다. 그 뒤의 `x = 2`는 `f`가 가진 1에 닿지 않는다.

### CLOSURE 명령 {#closure-op}

클로저는 바이트코드 명령 `CLOSURE` 하나로 만들어진다.

```dump bytecode
fn adder(n) = x => x + n

print(adder(5)(1))
```

- 컴파일러는 람다를 `adder`의 **자식 원형**(`== <lambda> ==`)으로 따로 컴파일하고, 그 원형에 "무엇을 붙잡는가" 목록(`captures`)을 적어 둔다. 여기서는 "만드는 프레임의 슬롯 0"(`n`) 하나다.
- 실행 중 `adder`의 `CLOSURE 0`은 자식 원형 0번을 꺼내 그 목록대로 값을 **복사**해 `Closure{원형, 붙잡은 값들, 전역 표}`를 만든다(`crates/fung-vm/src/vm.rs`의 `make_closure`).
- 람다 안의 `LOADCAPTURED 0`은 실행 중인 클로저의 붙잡은 값 0번을 읽는다. 바깥 프레임이 이미 사라졌어도 상관없다. 값은 클로저 안에 있다.

붙잡는 출처는 네 가지다(`fung-front`의 `tree::CaptureKind`).

<table>
  <thead><tr><th>종류</th><th>어디서 복사하나</th><th>예</th></tr></thead>
  <tbody>
    <tr><td><code>Local</code></td><td>만드는 프레임의 슬롯</td><td><code>adder</code>의 <code>n</code></td></tr>
    <tr><td><code>Captured</code></td><td>만드는 클로저가 이미 붙잡은 값</td><td><code>x =&gt; y =&gt; x + y + n</code>의 안쪽 람다가 보는 <code>n</code></td></tr>
    <tr><td><code>SelfFn</code></td><td>만드는 프레임의 클로저 자신</td><td>람다 안에서 바깥 함수 이름을 부를 때</td></tr>
    <tr><td><code>Global</code></td><td>그 순간의 파일 변수</td><td>최상위에서 만든 람다가 보는 최상위 변수</td></tr>
  </tbody>
</table>

두 겹 안쪽의 람다가 바깥의 바깥 변수를 쓰면 **붙잡기 사슬**이 생긴다. 가운데 람다가 그 값을 먼저 붙잡아 두고, 안쪽 람다는 가운데 람다의 붙잡은 값에서 다시 복사한다. 이름이 어느 칸에서 오는지는 6장의 이름 해석이 이미 정해 두었으므로, 실행 중에는 번호만 따라가면 된다.

### 자기 자신 부르기 {#self}

함수가 자기를 부를 때는 한 가지 문제가 있다. 블록 안에서 `fn go(…) = … go(…)`를 만드는 순간, 이름 `go`가 들어갈 슬롯은 **아직 비어 있다**. 클로저를 다 만든 뒤에야 저장되기 때문이다. 복사할 값이 없다. 그래서 자기 참조는 슬롯이 아니라 "지금 실행 중인 클로저를 올려라"라는 명령 `LOADSELF` 하나로 푼다.

```dump bytecode
fn countdown(n):
  fn go(i, acc) = i == 0 then acc else go(i - 1, acc + [i])
  go(n, [])
;;

print(countdown(3))
```

`go`의 0006번이 `LOADSELF`다. 최상위 함수가 자기를 부를 때도 전역을 읽지 않고 같은 명령을 쓴다.

## 최상위 함수와 전역 {#globals}

파일 최상위의 `fn`은 **끌어올려진다**(hoisting). 이름 해석이 최상위 `fn`을 먼저 모아 전역 번호를 붙이고, 파일 본문의 맨 앞에서 `CLOSURE`·`STOREGLOBAL`로 만든다. 그래서 정의보다 위에서 불러도 되고, 두 함수가 서로를 불러도 된다.

그 대신 최상위 `fn`은 **어떤 최상위 문장보다 먼저** 만들어진다. "만들어질 때 보이는 변수를 복사한다"는 규칙대로라면 복사할 최상위 변수가 하나도 없다. 그런데 설정값 하나를 함수마다 인자로 넘기라고 하면 불편하다. 그래서 규칙을 하나 더 두지 않고 관찰 하나를 썼다. **최상위에서 한 번만 대입되는 이름은 바뀌지 않는다.** 그런 이름은 상수와 같으므로, 이름 해석이 그 전역 칸을 미리 잡아 두고 최상위 `fn`이 부를 때 `LOADGLOBAL`로 읽게 한다(2026-10-20 검토의 결정 L9).

```fung run
limit = 3

fn small(n) = n < limit

print(small(2))
```

```dump bytecode
limit = 3

fn small(n) = n < limit

print(small(2))
```

`<main>`의 0000~0001번에서 `small`이 먼저 만들어지고, `limit`은 그다음 0002~0003번에서 채워진다. `small` 안의 `LOADGLOBAL 1`은 부를 때 그 칸을 읽는다. 두 번 대입되는 이름은 상수가 아니므로 보이지 않는다. 이것은 실행 전에 걸리는 컴파일 에러(종료 코드 2)다.

```fung run exit=2 error=E0204
limit = 3
limit = 4

fn small(n) = n < limit

print(small(2))
```

순서가 어긋나서, 대입이 실행되기 전에 함수가 불리면 칸이 아직 비어 있다. 이것은 실행 중에야 알 수 있으므로 런타임 에러 E0809다.

```fung run exit=1 error=E0809
fn small(n) = n < limit

print(small(2))
limit = 3
```

최상위에서 만든 **람다**는 다르다. 최상위 문장이 진행되는 도중에 만들어지므로, 그때까지 대입된 전역을 보통 규칙대로 복사한다(`CaptureKind::Global`).

```fung run
x = 1
f = () => x
x = 2
print(f(), x)
```

## 꼬리 호출 {#tail-calls}

`1 + f(n)`에서 `f(n)`을 부른 뒤에도 할 일이 남는다(`1 +`). 반면 `f(n)`이 함수의 마지막에 오고 그 결과를 그대로 돌려줄 뿐이면, 부른 쪽 프레임에는 더 할 일이 없다. 이런 자리를 **꼬리 위치**(tail position)라고 하고, 거기서 하는 호출을 **꼬리 호출**이라고 한다.

꼬리 호출이라면 부른 쪽 프레임을 남겨 둘 이유가 없다. 새 프레임을 쌓는 대신 **지금 프레임을 다시 쓰면** 재귀가 아무리 깊어도 스택이 자라지 않는다. 이것을 꼬리 호출 최적화(TCO, tail call optimization)라고 한다. 함수형 언어에서는 반복문 대신 재귀를 쓰므로 이것이 사실상 필수다.

:::box other 다른 언어는
- **Scheme**은 언어 명세가 꼬리 호출 최적화를 요구한다. **Lua**도 "proper tail call"을 보장한다.
- **CPython**은 하지 않는다. 재귀 깊이 기본 한도는 1,000이고, 넘으면 `RecursionError`다. 귀도 반 로섬은 에러의 호출 경로가 사라지는 것을 이유로 들었다.
- **JavaScript**는 ES2015 명세에 들어갔지만 실제로 구현한 엔진은 Safari의 JavaScriptCore 정도다.
- **JVM**은 바이트코드 수준에서 지원하지 않아서, Scala·Clojure는 자기 재귀만 반복문으로 바꾸는(`@tailrec`, `recur`) 길을 따로 둔다.
:::

:::box choice fung의 선택: 꼬리 위치의 호출은 모두 TAILCALL
fung은 함수형 언어이고 반복문 대신 재귀와 `fold`를 쓴다. 그래서 꼬리 호출 최적화는 처음부터 요구 사항이었다(설계 03). 트리를 직접 도는 인터프리터 대신 바이트코드 VM을 고른 이유 중 하나도 "꼬리 호출 최적화가 VM 쪽이 자연스럽다"였다(부록 [내부 구조](internals.html)). 대가로 생기는 "호출 경로에서 사라진 프레임"은 횟수로 보여 준다.
:::

컴파일러는 트리를 내려가며 `tail` 표시를 함께 넘긴다(`crates/fung-front/src/compiler.rs`). 함수 본문의 마지막 식은 꼬리이고, `if`·`then … else`의 두 갈래는 꼬리를 물려받으며, `return`의 피연산자도 꼬리다. 연산자의 피연산자와 호출의 인자는 꼬리가 아니다. 꼬리 위치의 호출은 `CALL` 대신 `TAILCALL`이 된다.

```dump bytecode
fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)
fn depth(n) = n == 0 then 0 else 1 + depth(n - 1)

print(count(5, 0) + depth(5))
```

`count`의 0013번은 `TAILCALL`이고, `depth`의 0011번은 그냥 `CALL`이다(`1 +`가 남아 있으므로). VM의 `TAILCALL`은 새 프레임을 쌓지 않는다. 지금 프레임의 슬롯을 지우고 새 인자를 `base` 자리로 옮긴 뒤, 프레임의 원형을 바꾸고 `ip`를 0으로 돌린다(`call.rs`의 `reuse_frame`). 그래서 `count(1000000, 0)`은 프레임 하나로 돈다.

꼬리가 아닌 재귀는 프레임이 쌓인다. 쌓인 프레임이 100,000개를 넘으면 E0804다(`MAX_FRAMES`). 프레임은 힙의 작은 구조체라서 이 깊이도 메모리 수십 MB 수준이지만, 끝없는 재귀를 빨리 알리려고 한도를 둔다.

```fung run exit=1 error=E0804
fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)
fn depth(n) = n == 0 then 0 else 1 + depth(n - 1)

print(count(1000000, 0))
print(depth(1000))
print(depth(200000))
```

에러가 꼬리 재귀 안에서 나면, 다시 쓴 프레임은 경로에서 보이지 않는 대신 횟수가 붙는다.

```fung run exit=1 error=E0502
fn down(n) = n == 0 then 1 / n else down(n - 1)

print(down(5))
```

### 반환 타입과 꼬리 호출 {#return-type}

반환 타입을 적은 함수(`-> int`)는 돌아오는 값을 `CHECKRETURN`으로 검사해야 한다. 그러려면 그 함수의 프레임이 남아 있어야 하므로, 다른 함수를 부르는 꼬리 호출은 `CALL`이 된다. 다만 **자기 자신을 부르는 꼬리 호출**은 예외 없이 `TAILCALL`로 남는다. 자기를 부른 값은 결국 같은 함수의 끝에서 같은 검사를 받기 때문이다(검토 결정 T4). 그래서 `fn count(n, acc) -> int = …`도 백만 번 돈다.

### 정리 장치와 꼬리 호출 {#defer}

많은 언어에 "이 함수가 끝날 때 이것을 실행하라"는 `defer`나 `finally`가 있다. fung은 `defer`를 두지 않았다. 할 일을 프레임에 매달면, 꼬리 호출이 그 프레임을 다시 쓰는 순간 매달린 일이 갈 곳을 잃는다(`background.md`). 정리는 평범한 내장 함수 `with(acquire, use, release)` 하나가 맡는다. 14장에서 자세히 본다.

## 코드에서 찾아보기 {#source}

- `crates/fung-vm/src/vm/frame.rs`: `Frame` 구조체.
- `crates/fung-vm/src/vm.rs`: 실행 루프 `step_loop`(`Op::Call`·`Op::TailCall`·`Op::Closure`·`Op::LoadCaptured`·`Op::LoadSelf`·`Op::JumpIfSet` 갈래), 클로저를 만드는 `make_closure`.
- `crates/fung-vm/src/vm/call.rs`: 호출 규약. `call_op`(빠른 길), `enter`, `bind_args`, `push_frame`, 꼬리 호출의 `reuse_frame`, `ret_with`, 깊이 한도 `MAX_FRAMES`.
- `crates/fung-vm/src/value.rs`: 함수 값 `Closure{proto, captured, globals}`.
- `crates/fung-front/src/tree/fn_info.rs`: 붙잡기 목록 `Capture`와 `CaptureKind`.
- `crates/fung-front/src/compiler.rs`, `compiler/fn_.rs`: `tail` 표시, `call`(CALL·TAILCALL 고르기), `function`(서두와 자식 원형).

:::exercise 반복문 안의 클로저
`map(1..n + 1, i => () => i * 10)`은 람다 셋을 담은 리스트를 만든다. 이것을 하나씩 불러 `[10, 20, 30]`이 나오는지 확인하고, JavaScript의 `var` 반복문 함정이 fung에서 왜 생기지 않는지 설명해 보자. 그리고 `[x, ...rest]` 패턴으로 리스트의 합을 구하는 함수를 꼬리 재귀로 써서 원소 20만 개짜리 리스트에서 돌려 보자.

:::solution
```fung run
fn makers(n):
  fs = map(1..n + 1, i => () => i * 10)
  map(fs, f => f())
;;

fn total(xs, acc) = match xs:
  []           => acc
  [x, ...rest] => total(rest, acc + x)
;;

print(makers(3))
print(total(map(0..200000, i => 1), 0))
```

`i => () => i * 10`의 안쪽 람다는 만들어지는 순간의 `i`를 복사한다. `map`은 원소마다 바깥 람다를 새로 부르고, 그때마다 `i`는 그 원소의 값이다. 공유할 변수가 없으니 마지막 값만 보는 일이 없다. `total`의 재귀 호출은 꼬리 위치라서 `TAILCALL`이 되고, 20만 번 돌아도 프레임은 하나다.
:::
:::

## 정리 {#summary}

- 함수 호출은 프레임 하나를 쌓는다. 프레임은 원형, 다음 명령 번호(`ip`), 슬롯의 시작(`base`), 실행 중인 클로저, 모듈 전역을 든다. 프레임과 값 스택은 힙의 배열이라 fung의 재귀는 Rust의 재귀가 아니다.
- `CALL n`의 피연산자는 위치 인자 수와 이름 인자 수를 담는다. 개수가 맞는 위치 인자는 그 자리에서 슬롯이 되고, 나머지는 `bind_args`가 배치한다. 내장 함수도 같은 규약을 쓴다.
- 기본값은 함수 서두의 `JUMPIFSET`이다. 빈 칸(`None`)과 `nil`은 다르다.
- 클로저는 코드와 붙잡은 값의 묶음이다. fung은 만들어질 때 보이는 값을 **복사**하므로 upvalue 상자가 없다. 자기 참조는 `LOADSELF`다.
- 최상위 `fn`은 끌어올려지고 최상위 변수를 복사할 수 없다. 한 번만 대입되는 이름만 `LOADGLOBAL`로 읽는다(비어 있으면 E0809).
- 꼬리 위치의 호출은 `TAILCALL`이 되어 프레임을 다시 쓴다. 꼬리가 아닌 재귀는 100,000단계에서 E0804다.

레퍼런스: [함수](../reference/language.html#functions), [변수와 스코프](../reference/language.html#scope). 배우기: [5장 함수](../learn/05-functions.html). 다음 장은 이 장에서 "값"이라고만 부른 것, 곧 `Value`가 메모리에 어떻게 놓이는지를 본다.
