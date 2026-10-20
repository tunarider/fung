---
description: 바이트코드와 스택 머신.
lede: 트리를 평평한 명령 목록으로 바꾸고, 값을 쌓았다 꺼내는 작은 기계로 실행한다. 명령 하나는 32비트, 기계의 상태는 값 스택과 프레임 목록이 전부다.
---
# 바이트코드와 스택 머신

<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li>코어</li><li class="on">바이트코드</li><li>실행</li></ol>

:::goals
- 스택 머신이 계산하는 방법과, 레지스터 머신과의 차이
- fung 명령의 생김새: 32비트 한 단어에 명령과 피연산자
- 함수 원형(Proto): 명령 목록, 상수 풀, 위치 표
- 컴파일러가 코어 트리를 명령으로 바꾸는 법, 점프와 나중에 채우기
- VM의 실행 루프, 프레임과 슬롯, 그리고 `fung dump bytecode` 읽기
:::

## 트리에서 목록으로 {#tree-to-list}

7장까지 소스는 이름이 모두 정해진 코어 트리가 되었다. 2장에서 본 것처럼 트리를 그대로 걸어 다니며 실행할 수도 있지만, fung은 트리를 한 번 더 바꿔 **바이트코드**라는 평평한 명령 목록을 만들고, 그것을 하나씩 실행하는 가상 기계(VM)를 둔다.

명령 목록으로 바꾸려면 "트리의 계산 순서"를 "명령의 순서"로 펴야 한다. 이때 가장 단순한 방법이 **스택 머신**(stack machine)이다.

## 스택 머신 {#stack-machine}

**스택**(stack)은 접시 더미 같은 자료구조다. 맨 위에 올리고(push), 맨 위에서 꺼낸다(pop). 스택 머신의 명령은 대부분 이 스택만 다룬다.

- 값을 만드는 명령은 값을 스택 **위에 올린다**. `CONST 2`는 2를 올린다.
- 계산하는 명령은 필요한 값을 스택 **위에서 꺼내** 계산하고, 결과를 **다시 올린다**. `MUL`은 둘을 꺼내 곱한 값을 올린다.

명령에 "어느 값을"이 적혀 있지 않다. 늘 스택의 맨 위다. 그래서 명령이 짧고, 컴파일러는 트리를 **자식 먼저, 부모 나중**의 순서(후위 순회, post-order)로 걸으며 명령을 내기만 하면 된다. `1 + 2 * 3`을 보자.

```dump bytecode
print(1 + 2 * 3)
```

`print`와 세 수를 올린 뒤의 스택 변화는 이렇다(오른쪽이 맨 위).

<table>
  <thead><tr><th>명령</th><th>하는 일</th><th>스택</th></tr></thead>
  <tbody>
    <tr><td><code>CONST 0</code></td><td><code>print</code> 함수를 올린다</td><td><code>print</code></td></tr>
    <tr><td><code>CONST 1</code></td><td>1을 올린다</td><td><code>print 1</code></td></tr>
    <tr><td><code>CONST 2</code></td><td>2를 올린다</td><td><code>print 1 2</code></td></tr>
    <tr><td><code>CONST 3</code></td><td>3을 올린다</td><td><code>print 1 2 3</code></td></tr>
    <tr><td><code>MUL</code></td><td>2와 3을 꺼내 곱한 6을 올린다</td><td><code>print 1 6</code></td></tr>
    <tr><td><code>ADD</code></td><td>1과 6을 꺼내 더한 7을 올린다</td><td><code>print 7</code></td></tr>
    <tr><td><code>CALL 1</code></td><td>함수와 인자 하나를 꺼내 부르고 결과를 올린다</td><td><code>nil</code></td></tr>
  </tbody>
</table>

트리에서 `*`가 `+`의 자식이었으므로(4장) `MUL`이 `ADD`보다 먼저 나왔다. 괄호도 우선순위 표도 필요 없다. 트리 모양이 이미 순서를 정해 두었다. 이렇게 연산자를 피연산자 뒤에 적는 표기를 후위 표기(역폴란드 표기)라고 하는데, 스택 머신의 바이트코드는 그 표기를 그대로 실행하는 것이다.

### 레지스터 머신 {#register-machine}

다른 길도 있다. **레지스터 머신**(register machine)은 실제 CPU처럼 번호 붙은 칸(레지스터)을 두고, 명령이 칸 번호를 직접 말한다. `MUL r3, r1, r2`는 "1번 칸과 2번 칸을 곱해 3번 칸에". 같은 계산을 더 적은 명령으로 할 수 있어서 명령을 읽고 갈래를 타는 비용이 준다. 대신 컴파일러가 "어느 값을 어느 칸에 둘지" 배정해야 해서 복잡해지고, 명령 하나가 칸 번호를 여럿 담느라 길어진다.

## 명령 하나의 생김새 {#encoding}

fung의 명령 하나는 **32비트 단어 하나**다. 위 8비트가 명령의 종류(opcode), 아래 24비트가 피연산자(operand)다.

```plain
 31        24 23                                 0
┌────────────┬────────────────────────────────────┐
│   명령     │            피연산자                │
│  (8비트)   │            (24비트)                │
└────────────┴────────────────────────────────────┘
 CONST 5  =  명령 0번(CONST) | 피연산자 5
```

8비트면 명령을 256가지까지 둘 수 있고, fung은 지금 53가지를 쓴다. 24비트면 피연산자로 1,600만 남짓까지 적을 수 있어서 상수 번호, 슬롯 번호, 점프할 명령 번호에 충분하다. 명령을 꺼낼 때는 비트 연산 둘(오른쪽으로 24칸 밀기, 아래 24비트 걸러내기)이면 된다.

피연산자에 수 두 개가 필요한 명령은 24비트를 나눠 쓴다. `CALL`은 아래 12비트에 위치 인자 수, 그 위 12비트에 이름 인자 수를 담는다. 역어셈블 출력의 `; 1 positional, 0 named`가 그것을 풀어 보인 것이다.

명령은 무리로 나눠 보면 다음과 같다.

<table>
  <thead><tr><th>무리</th><th>명령</th></tr></thead>
  <tbody>
    <tr><td>값</td><td><code>CONST</code> <code>POP</code> <code>LIST</code> <code>MAP</code> <code>TUPLE</code> <code>TAG</code> <code>RANGE</code> <code>CONCAT</code></td></tr>
    <tr><td>연산</td><td><code>ADD</code> <code>SUB</code> <code>MUL</code> <code>DIV</code> <code>FLOORDIV</code> <code>MOD</code> <code>POW</code> <code>NEG</code> <code>NOT</code> <code>EQ</code> <code>NE</code> <code>LT</code> <code>GT</code> <code>LE</code> <code>GE</code> <code>IN</code> <code>CHECKBOOL</code></td></tr>
    <tr><td>접근</td><td><code>INDEX</code> <code>SLICE</code> <code>FIELD</code></td></tr>
    <tr><td>변수</td><td><code>LOADLOCAL</code> <code>STORELOCAL</code> <code>CLEARLOCAL</code> <code>LOADGLOBAL</code> <code>STOREGLOBAL</code> <code>LOADCAPTURED</code> <code>LOADSELF</code></td></tr>
    <tr><td>흐름</td><td><code>JUMP</code> <code>JUMPIFFALSE</code> <code>JUMPIFSET</code></td></tr>
    <tr><td>함수</td><td><code>CLOSURE</code> <code>CALL</code> <code>TAILCALL</code> <code>RETURN</code> <code>CHECKTYPE</code> <code>CHECKRETURN</code></td></tr>
    <tr><td>결과값</td><td><code>UNWRAPABORT</code>(<code>!</code>) <code>UNWRAPTEST</code>(<code>?</code>) <code>COALESCE</code>(<code>??</code>)</td></tr>
    <tr><td>패턴</td><td><code>MATCH</code> <code>MATCHORFAIL</code> <code>NOMATCH</code></td></tr>
    <tr><td>기타</td><td><code>IMPORT</code> <code>ASSERT</code> <code>TEST</code></td></tr>
  </tbody>
</table>

이 장에서는 값·연산·변수·흐름·호출의 기본 명령을 본다. 클로저와 기본값, 꼬리 호출은 9장, 모듈은 11장에서 다시 만난다.

## 함수 원형: Proto {#proto}

컴파일러는 **함수마다** 명령 목록을 하나씩 만든다. 파일 본문도 `<main>`이라는 이름의 함수다. 그 결과물을 **함수 원형**(prototype, 코드에서 `Proto`)이라고 한다. 원형에는 명령만 있는 게 아니다.

- `code`: 명령 목록.
- `consts`: **상수 풀**(constant pool). 명령에 큰 값을 직접 넣을 수 없으니, 수·문자열·내장 함수 같은 상수는 여기 두고 명령은 번호로 가리킨다. `CONST 1`은 "상수 풀 1번을 올려라"다. 같은 상수는 한 번만 넣고 다시 쓴다.
- `pos`: 명령마다 소스 위치 하나. `code`와 나란한 배열이다(7장).
- `num_locals`: 이 함수가 쓰는 슬롯 수(6장).
- `params`: 매개변수 표(이름, 기본값이 있는가, `...rest`인가, 타입 표기).
- `protos`: 안쪽 함수들의 원형. `CLOSURE n`이 n번 자식을 가리킨다.
- 그 밖에 붙잡을 값 목록(9장), 타입 표기, 패턴 트리 등.

**원형**이라고 부르는 이유는 이것이 아직 실행할 수 있는 함수 값이 아니라 함수의 "틀"이기 때문이다. 실행 중에 `CLOSURE` 명령이 이 틀과 붙잡은 값을 묶어 진짜 함수 값(클로저)을 만든다(9장).

## dump bytecode 읽기 {#reading}

이제 이 책의 두 줄을 읽어 보자.

```dump bytecode
fn double(x) = x * 2
print(double(21))
```

- `== <main> ==`, `== double ==`: 원형 하나에 절 하나. 파일 본문 다음에 안쪽 함수들이 차례로 나온다.
- 첫 열 `0000`: 명령 번호. 점프는 이 번호로 간다.
- 둘째 열 `1:16`: 그 명령의 소스 위치(줄:칸). 호출(`CALL`)은 여는 괄호 `(`의 자리, 연산은 연산자의 자리를 갖는다. 그래서 `double(21)`의 호출은 2:13, `print(…)`의 호출은 2:6이다.
- 셋째, 넷째 열: 명령 이름과 피연산자.
- `;` 뒤: 피연산자를 풀어 쓴 설명. 상수 번호면 그 상수의 값(`; 21`, `; <fn print>`), 전역 번호면 그 이름(`; double`), `CALL`이면 인자 수.

`print`가 `CONST 0 ; <fn print>`인 것을 눈여겨보자. 6장에서 `print`는 `(prelude print)`로 해석되었고, 컴파일러는 그 내장 함수 자체를 상수 풀에 넣었다. 실행 중에 "print라는 이름의 함수"를 찾는 일이 없다.

## 따라가기: 명령 하나씩 {#follow}

VM은 `<main>`의 0번 명령부터 시작한다. 스택은 비어 있다.

<table>
  <thead><tr><th>번호</th><th>명령</th><th>하는 일</th><th>스택(오른쪽이 맨 위)</th></tr></thead>
  <tbody>
    <tr><td>0000</td><td><code>CLOSURE 0</code></td><td>자식 원형 0번(<code>double</code>)으로 함수 값을 만들어 올린다</td><td><code>&lt;fn double&gt;</code></td></tr>
    <tr><td>0001</td><td><code>STOREGLOBAL 0</code></td><td>꺼내서 전역 칸 0번에 둔다</td><td>(빔)</td></tr>
    <tr><td>0002</td><td><code>CONST 0</code></td><td>상수 0번 <code>print</code>를 올린다</td><td><code>print</code></td></tr>
    <tr><td>0003</td><td><code>LOADGLOBAL 0</code></td><td>전역 0번 <code>double</code>을 올린다</td><td><code>print double</code></td></tr>
    <tr><td>0004</td><td><code>CONST 1</code></td><td>21을 올린다</td><td><code>print double 21</code></td></tr>
    <tr><td>0005</td><td><code>CALL 1</code></td><td><code>double</code>을 인자 하나로 부른다(아래)</td><td><code>print 42</code></td></tr>
    <tr><td>0006</td><td><code>CALL 1</code></td><td><code>print</code>를 인자 하나로 부른다. 42를 찍고 <code>nil</code>을 돌려준다</td><td><code>nil</code></td></tr>
    <tr><td>0007</td><td><code>POP</code></td><td>문장의 값을 버린다</td><td>(빔)</td></tr>
    <tr><td>0008</td><td><code>CONST 2</code></td><td><code>nil</code>을 올린다</td><td><code>nil</code></td></tr>
    <tr><td>0009</td><td><code>RETURN</code></td><td>파일 본문이 끝난다. 값은 <code>nil</code></td><td></td></tr>
  </tbody>
</table>

`CALL`은 피연산자를 보고 스택에서 "함수, 그 위의 인자들"을 찾는다. 인자가 하나니까 맨 위가 인자, 그 아래가 함수다. 호출할 함수가 fung 함수(`double`)면 VM은 새 **프레임**(frame)을 쌓는다. 프레임은 "지금 실행 중인 함수 하나"의 기록이다.

```plain
Frame {
  proto  실행할 원형(double)
  ip     다음에 실행할 명령 번호(0부터)
  base   이 함수의 슬롯이 스택의 몇째 칸부터 시작하나
  …      클로저, 전역 표, 꼬리 호출 횟수
}
```

fung 함수를 매개변수 수 그대로 부르는 흔한 경우, 스택에 이미 놓인 인자가 **그 자리에서** 새 프레임의 첫 슬롯이 된다. 인자를 따로 복사하지 않는다. `double`의 프레임은 `base`가 인자 21이 놓인 칸이고, 슬롯 0(`x`)이 곧 그 21이다.

```plain
스택:  [ print | 21 ]
              ↑
          base(슬롯 0 = x)
```

이제 VM은 `double`의 0번 명령으로 간다.

<table>
  <thead><tr><th>번호</th><th>명령</th><th>하는 일</th><th>스택(<code>double</code>의 칸부터)</th></tr></thead>
  <tbody>
    <tr><td>0000</td><td><code>LOADLOCAL 0</code></td><td>슬롯 0(<code>x</code>)을 복사해 올린다. 실제 위치는 <code>base + 0</code></td><td><code>21 │ 21</code></td></tr>
    <tr><td>0001</td><td><code>CONST 0</code></td><td><code>double</code>의 상수 0번, 2를 올린다</td><td><code>21 │ 21 2</code></td></tr>
    <tr><td>0002</td><td><code>MUL</code></td><td>둘을 꺼내 곱한 42를 올린다</td><td><code>21 │ 42</code></td></tr>
    <tr><td>0003</td><td><code>RETURN</code></td><td>맨 위 42를 꺼내고, 스택을 <code>base</code>까지 자르고, 프레임을 버리고, 부른 쪽 스택에 42를 올린다</td><td>→ <code>print 42</code></td></tr>
  </tbody>
</table>

(`│` 왼쪽이 슬롯, 오른쪽이 계산 중인 값이다.) 슬롯과 계산 중인 값이 **같은 스택 하나**에 이어서 산다. 함수를 부를 때마다 그 위에 새 프레임의 칸이 이어 붙고, 돌아오면 잘려 나간다. `RETURN`이 `base`까지 잘라 내므로 함수가 남긴 임시 값이 새어 나가지 않는다.

상수 풀이 원형마다 따로라는 점도 보자. `<main>`의 `CONST 0`은 `print`, `double`의 `CONST 0`은 2다.

`print`는 Rust로 쓴 내장 함수라 프레임을 쌓지 않는다. VM은 인자를 꺼내 Rust 함수를 바로 부르고, 돌려준 값을 스택에 올린다.

## 실행 루프 {#loop}

VM의 심장은 반복문 하나다.

```plain
반복:
  f = 맨 위 프레임
  명령 = f.proto.code[f.ip]
  f.ip = f.ip + 1
  명령의 종류에 따라:
    CONST     → 상수 풀[피연산자]를 올린다
    LOADLOCAL → 스택[f.base + 피연산자]를 올린다
    MUL       → 둘을 꺼내 곱하고 올린다(넘치거나 타입이 틀리면 에러)
    JUMP      → f.ip = 피연산자
    CALL      → 새 프레임을 쌓거나 내장 함수를 부른다
    RETURN    → 프레임을 버리고 값을 부른 쪽에 넘긴다
    …
```

명령을 가져오고(fetch), 무엇인지 보고(decode), 실행한다(execute). 실제 CPU가 하는 일과 같은 순환이다. 프레임은 VM의 목록에 쌓일 뿐이라, fung 함수가 함수를 불러도 이 반복문이 Rust 함수를 재귀로 부르지 않는다(2장).

정수끼리의 덧셈·곱셈에는 빠른 길이 있다. 두 값이 모두 정수면 바로 계산하고, 넘칠 때만 값 층의 일반 함수로 넘겨 `E0503 integer overflow`를 만든다. fung은 정수가 조용히 넘쳐 돌아가는 것을 허용하지 않는다.

```fung run exit=1 error=E0503
fn double(x) = x * 2
print(double(9223372036854775807))
```

## 점프: if는 어떻게 되나 {#jumps}

명령 목록에는 가지가 없다. 조건에 따라 다른 곳을 실행하려면 **점프**(jump)를 쓴다.

- `JUMPIFFALSE n`: 맨 위 값을 꺼내 거짓이면 n번 명령으로 간다. 참이면 다음 명령으로. 불이 아니면 `E0505`.
- `JUMP n`: 무조건 n번으로 간다.

`if c then a else b`는 이렇게 펴진다.

```plain
      c 계산
      JUMPIFFALSE  else로
      a 계산
      JUMP         끝으로
else: b 계산
끝:
```

```dump bytecode
fn sign(n) = n < 0 then "minus" else "plus"
print(sign(-3))
```

`sign`의 3번 `JUMPIFFALSE 6`은 조건이 거짓이면 6번(`"plus"`)으로, 참이면 4번(`"minus"`)을 올린 뒤 5번 `JUMP 7`로 끝(`RETURN`)에 간다. `<main>`의 `CONST 1 ; 3` 다음 `NEG`도 보자. 3장에서 말한 대로 `-3`은 토큰 둘이고, "3을 올리고 부호를 바꿔라"가 되었다.

컴파일러가 `JUMPIFFALSE`를 낼 때는 else가 몇 번에서 시작할지 아직 모른다. 그래서 피연산자를 0으로 두고 그 명령의 번호를 기억해 두었다가, else를 낼 차례가 되면 그때의 번호로 **나중에 채운다**(backpatching). 코드의 `Proto::patch`가 그 일이다.

## 문장과 지역 변수 {#statements}

블록 안의 문장들은 차례로 컴파일된다. 대입은 "값 계산, 그다음 `STORELOCAL`(블록 안) 또는 `STOREGLOBAL`(최상위)"이다. 식 문장은 값을 계산한 뒤 `POP`으로 버린다. 버리는 값이 `Err`이면 `POP`이 `E0606`을 낸다. 실패를 조용히 버리지 않게 하는 장치다.

```dump bytecode
fn report(xs):
  total = sum(xs)
  avg = total / len(xs)
  "total ${total}, avg ${avg}"
;;
print(report([3, 4, 5]))
```

몇 가지가 보인다.

- `<main>`의 `CONST 1 ; [3, 4, 5]`: 원소가 모두 상수인 리스트는 컴파일할 때 상수 하나가 된다.
- `STORELOCAL 1`, `STORELOCAL 2`: `total`과 `avg`의 슬롯(6장에서 정한 번호 그대로).
- `CLEARLOCAL 0`: `xs`(슬롯 0)는 3줄 뒤로 다시 읽히지 않는다. 컴파일러가 그것을 보고 그 칸을 비운다. 큰 리스트가 함수가 끝날 때까지 메모리에 남지 않게 하려는 것이다(10장).
- `CONCAT 4`: 보간 조각 넷을 꺼내 하나의 문자열로 잇는다(5장의 `concat`).
- 마지막 식은 `POP` 없이 `RETURN`이 된다. 함수의 값이기 때문이다.

:::box other 다른 언어는
- **CPython**: 스택 머신이다. 3.6부터 명령 하나가 2바이트(명령 1바이트 + 피연산자 1바이트, 더 크면 앞에 확장 명령) 단위인 "wordcode"를 쓴다.
- **JVM**: 스택 머신이다. 명령은 가변 길이 바이트 열이다.
- **WebAssembly**: 스택 머신 형식으로 적지만, 실행할 때는 대부분 기계어로 다시 컴파일된다.
- **Lua 5**: 레지스터 머신이다. Lua 저자들의 논문은 레지스터 머신으로 바꾼 뒤 명령 수가 줄고 빨라졌다고 보고했다.
- **Dalvik**(옛 안드로이드 VM): 같은 Java 프로그램을 JVM의 스택 바이트코드에서 레지스터 바이트코드로 바꿔 실행했다.
:::

:::box choice fung의 선택
- **스택 머신.** 설계 문서 09의 구조 그대로 "스택 VM"이다. 레지스터 머신과 인라인 캐시(같은 자리의 이름 조회 결과를 기억하는 기법)는 "측정을 보고 정한다"로 미뤄 두었다(open-questions "성능"). 2026-10-14 측정에서는 호출의 빠른 길(인자가 그 자리에서 슬롯이 되기)과 범위를 리스트로 펼치지 않는 반복으로 함수 호출과 반복이 2.3~2.5배 빨라졌고, 남은 시간은 명령 해석 루프 자체라고 기록했다.
- **고정 폭 32비트 = 8비트 명령 + 24비트 피연산자.** 디코딩과 점프 계산이 단순하고 역어셈블러가 쉽다. 24비트면 상수·슬롯·점프 위치에 충분하다. 가변 길이 바이트 열은 매 명령의 길이를 따져야 해서 택하지 않았다(설계 문서 09, 내부 구조 부록).
- **내장 함수는 상수로 직접.** prelude 이름은 컴파일러가 함수 값을 상수 풀에 넣는다. 실행 중에 이름을 찾지 않는다(6장).
- **명령마다 소스 위치.** 런타임 에러가 정확한 줄과 칸을 가리키는 이유다(7장).
:::

## 코드에서 찾아보기 {#source}

- `crates/fung-front/src/bytecode/op.rs`: 명령 종류 `Op`(번호가 고정된 `#[repr(u8)]` 열거형), 32비트 단어 `Instr`(`Instr::new`, `Instr::op`, `Instr::arg`), `CALL` 피연산자를 싸고 푸는 `encode_call`·`decode_call`, 이름 `Op::name`.
- `crates/fung-front/src/bytecode.rs`: 함수 원형 `Proto`와 `Proto::emit`(명령과 위치를 함께 넣는다), `Proto::patch`(점프를 나중에 채운다), `Proto::add_const`(같은 상수 다시 쓰기).
- `crates/fung-front/src/bytecode/disasm.rs`: `disassemble`, 곧 `fung dump bytecode`의 출력.
- `crates/fung-front/src/compiler.rs`: `compile`과 `Compiler::block`(문장, `POP`, `CLEARLOCAL`), `Compiler::expr`(코어 형태마다 명령 내기), `Compiler::op`, `Compiler::if_expr`(점프 둘), `Compiler::let_stmt`. 함수와 호출은 `crates/fung-front/src/compiler/fn_.rs`의 `Compiler::function`과 `Compiler::call`.
- `crates/fung-vm/src/vm/link.rs`: 원형을 실행용 `RtProto`로 바꾸며 상수를 실제 값으로 채우는 `link`.
- `crates/fung-vm/src/vm.rs`: `Task`(값 스택 `stack`과 프레임 목록 `frames`)와 실행 루프 `Task::step_loop`, 정수 빠른 길 `Task::binary`.
- `crates/fung-vm/src/vm/frame.rs`: `Frame { proto, ip, base, closure, globals, tail_calls, … }`.
- `crates/fung-vm/src/vm/call.rs`: `Task::call_op`(인자가 그 자리에서 슬롯이 되는 빠른 길), `Task::enter`, `Task::push_frame`, `Task::ret_with`(`RETURN`).

:::exercise 점프 따라가기
아래 함수의 바이트코드를 예상해 보자. `then … else`가 두 번 겹쳐 있으니 `JUMPIFFALSE`와 `JUMP`가 각각 두 개씩 나올 것이다. 각 점프가 몇 번 명령으로 가는지 적은 뒤 확인하자.

```fung frag
fn grade(score) = score >= 90 then "A" else score >= 80 then "B" else "C"
```

:::solution
바깥 `then`의 else 가지가 안쪽 `then` 전체다. 그래서 안쪽 조건이 바깥 `JUMPIFFALSE`의 도착점이 된다.

```dump bytecode
fn grade(score) = score >= 90 then "A" else score >= 80 then "B" else "C"
print(grade(95), grade(85), grade(10))
```

`grade`에서 3번 `JUMPIFFALSE 6`은 90 미만이면 6번(안쪽 조건)으로, 5번 `JUMP`는 맨 끝 `RETURN`으로 간다. 9번 `JUMPIFFALSE 12`는 80 미만이면 12번(`"C"`)으로, 11번 `JUMP 13`은 안쪽의 끝으로 간다. 안쪽의 끝이 곧 바깥의 끝이다.

```fung run
fn grade(score) = score >= 90 then "A" else score >= 80 then "B" else "C"
print(grade(95), grade(85), grade(10))
```

:::

:::

## 정리 {#summary}

- 스택 머신의 명령은 스택 맨 위의 값을 꺼내 계산하고 결과를 올린다. 컴파일러는 트리를 자식 먼저 걸으며 명령을 낸다.
- 레지스터 머신은 명령이 칸 번호를 직접 말한다. 명령 수는 줄지만 컴파일러가 복잡해진다.
- fung 명령은 32비트 한 단어(8비트 명령 + 24비트 피연산자)이고, 53가지다.
- 함수마다 원형(`Proto`)이 하나: 명령 목록, 상수 풀, 명령마다 위치, 슬롯 수, 매개변수 표, 자식 원형들.
- VM은 반복문 하나로 명령을 가져와 실행한다. 프레임마다 `base`가 있고, 슬롯 n은 `stack[base + n]`이다. 인자는 그 자리에서 슬롯이 된다.
- `if`는 `JUMPIFFALSE`와 `JUMP`로 펴지고, 점프 위치는 나중에 채운다.

다음 장에서는 함수 값이 어떻게 만들어지는지, 클로저가 바깥 변수를 어떻게 붙잡는지, 기본값과 꼬리 호출이 바이트코드에서 어떻게 보이는지 본다. 명령줄 사용법은 [명령줄 레퍼런스](../reference/cli.html)에 있다.
