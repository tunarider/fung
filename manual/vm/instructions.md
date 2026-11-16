---
description: fung VM의 명령 59가지: 명령마다 피연산자의 뜻, 스택에서 꺼내는 것과 올리는 것, 하는 일.
lede: <code>fung dump bytecode</code>에 나오는 명령을 찾아보는 곳이다. 명령마다 피연산자가 무엇의 번호인지, 스택에서 몇 개를 꺼내고 몇 개를 올리는지, 무슨 일을 하는지 적었다.
---
# 명령 레퍼런스

## 표 읽는 법 {#how-to-read}

`fung dump bytecode`의 한 줄은 이렇게 생겼다.

```dump bytecode
a = 2
print(1 + a * 3)
```

`CONST 0`에서 `CONST`가 명령이고 `0`이 피연산자다. `;` 뒤는 명령의 일부가 아니라 `fung dump`가 그 번호가 가리키는 것을 적어 준 설명이다([8장](08-bytecode.html#reading)). 아래 표의 열은 다음과 같다.

- **피연산자**: 명령 뒤의 수가 무엇인지. 대부분 값이 아니라 **번호**다. 함수 원형이 가진 표(상수 풀, 패턴 표 등)의 칸 번호이거나, 프레임의 칸 번호이거나, 같은 함수 안의 명령 번호다. 수가 둘인 명령은 쉼표로 나눠 차례대로 적었다. 표를 통째로 보려면 [`fung dump proto`](#proto)를 쓴다.
- **스택**: `꺼내는 수 → 올리는 수`. `2 → 1`은 맨 위 둘을 꺼내고 하나를 올린다는 뜻이다. `n`은 첫째 피연산자다. 두 값을 꺼내는 명령에서는 먼저 올라간 값이 왼쪽 피연산자, 맨 위가 오른쪽 피연산자다.
- **하는 일**: 그 명령이 하는 일과, 실패하면 내는 에러.

피연산자의 종류는 열둘이다.

<table>
  <thead><tr><th>적은 말</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td>없음</td><td>피연산자를 쓰지 않는다. 출력에도 나오지 않는다.</td></tr>
    <tr><td>상수 번호</td><td>그 함수의 상수 풀의 칸.</td></tr>
    <tr><td>전역 번호</td><td>그 파일의 전역 변수 표의 칸.</td></tr>
    <tr><td>칸 번호</td><td>실행 중인 프레임의 지역 변수 칸(슬롯).</td></tr>
    <tr><td>함수 번호</td><td>그 함수 안에 적힌 안쪽 함수 원형의 번호.</td></tr>
    <tr><td>붙잡은 값 번호</td><td>실행 중인 클로저가 만들어질 때 복사해 둔 값의 번호.</td></tr>
    <tr><td>타입 번호</td><td>그 함수의 타입 표기 표의 칸.</td></tr>
    <tr><td>패턴 번호</td><td>그 함수의 패턴 표의 칸.</td></tr>
    <tr><td>assert 번호</td><td>그 함수의 <code>assert</code> 표의 칸(실패했을 때 보일 소스 글).</td></tr>
    <tr><td>명령 번호</td><td>같은 함수의 명령 번호. 점프가 갈 곳이다.</td></tr>
    <tr><td>개수</td><td>스택에서 꺼낼 값의 수.</td></tr>
    <tr><td>플래그</td><td>비트마다 뜻이 있는 수.</td></tr>
  </tbody>
</table>

이 표의 명령 이름, 피연산자, 스택 열은 구현의 명령 표(`crates/fung-front/src/bytecode/op.rs`)와 같은지 테스트가 지킨다.

## 값 만들기 {#values}

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>CONST</code></td><td>상수 번호</td><td>0 → 1</td><td>상수 풀의 그 칸에 든 값을 올린다. 수, 문자열, 심볼, 내장 함수, 그리고 원소가 모두 상수인 리스트·맵·튜플·태그 값이 여기서 나온다.</td></tr>
    <tr><td><code>POP</code></td><td>없음</td><td>1 → 0</td><td>맨 위 값을 버린다. 문장으로 쓴 호출처럼 값을 쓰지 않는 식 뒤에 나온다.</td></tr>
    <tr><td><code>LIST</code></td><td>개수</td><td>n → 1</td><td>값 n개를 꺼내 올라간 순서대로 담은 리스트를 올린다.</td></tr>
    <tr><td><code>MAP</code></td><td>개수, 상수 번호</td><td>n → 1</td><td>값 n개를 꺼내 맵을 만든다. 키는 둘째 피연산자가 가리키는 상수(키 이름의 목록)에서 차례대로 가져온다.</td></tr>
    <tr><td><code>TUPLE</code></td><td>개수</td><td>n → 1</td><td>값 n개를 꺼내 튜플을 올린다.</td></tr>
    <tr><td><code>TAG</code></td><td>개수, 상수 번호</td><td>n → 1</td><td>값 n개를 꺼내 태그 값을 만든다(<code>Ok(x)</code>, <code>Point(x, y)</code>). 태그의 이름은 둘째 피연산자가 가리키는 상수(심볼)다.</td></tr>
    <tr><td><code>RANGE</code></td><td>플래그</td><td>2 → 1</td><td>시작과 끝을 꺼내 범위를 올린다. 플래그가 1이면 끝을 포함한다(<code>..=</code>). 정수가 아니면 E0501, 시작이 끝보다 크면 E0522.</td></tr>
    <tr><td><code>CONCAT</code></td><td>개수</td><td>n → 1</td><td>조각 n개를 꺼내 이어 붙인 문자열을 올린다. 문자열 보간이 이 명령이 된다. 문자열이 아닌 조각은 <code>print</code>가 찍는 모양의 글이 된다.</td></tr>
  </tbody>
</table>

## 연산 {#operators}

두 값을 꺼내는 명령은 먼저 올라간 값이 연산자의 왼쪽, 맨 위가 오른쪽이다. 연산자마다의 규칙과 에러는 언어 명세의 [연산자](../reference/language/operators.html)가 정한다. 명령은 그 연산을 하고 결과를 올릴 뿐이다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>ADD</code></td><td>없음</td><td>2 → 1</td><td><code>a + b</code>.</td></tr>
    <tr><td><code>SUB</code></td><td>없음</td><td>2 → 1</td><td><code>a - b</code>.</td></tr>
    <tr><td><code>MUL</code></td><td>없음</td><td>2 → 1</td><td><code>a * b</code>.</td></tr>
    <tr><td><code>DIV</code></td><td>없음</td><td>2 → 1</td><td><code>a / b</code>. 결과는 언제나 실수다.</td></tr>
    <tr><td><code>FLOORDIV</code></td><td>없음</td><td>2 → 1</td><td><code>a // b</code>. 몫을 내림한다.</td></tr>
    <tr><td><code>MOD</code></td><td>없음</td><td>2 → 1</td><td><code>a % b</code>.</td></tr>
    <tr><td><code>POW</code></td><td>없음</td><td>2 → 1</td><td><code>a ** b</code>.</td></tr>
    <tr><td><code>NEG</code></td><td>없음</td><td>1 → 1</td><td>단항 <code>-a</code>.</td></tr>
    <tr><td><code>NOT</code></td><td>없음</td><td>1 → 1</td><td><code>not a</code>. 불이 아니면 에러다.</td></tr>
    <tr><td><code>EQ</code></td><td>없음</td><td>2 → 1</td><td><code>a == b</code>. 불을 올린다.</td></tr>
    <tr><td><code>NE</code></td><td>없음</td><td>2 → 1</td><td><code>a != b</code>.</td></tr>
    <tr><td><code>LT</code></td><td>없음</td><td>2 → 1</td><td><code>a &lt; b</code>.</td></tr>
    <tr><td><code>GT</code></td><td>없음</td><td>2 → 1</td><td><code>a &gt; b</code>.</td></tr>
    <tr><td><code>LE</code></td><td>없음</td><td>2 → 1</td><td><code>a &lt;= b</code>.</td></tr>
    <tr><td><code>GE</code></td><td>없음</td><td>2 → 1</td><td><code>a &gt;= b</code>.</td></tr>
    <tr><td><code>IN</code></td><td>없음</td><td>2 → 1</td><td><code>x in xs</code>. 찾을 값과 그 위의 컬렉션을 꺼내, 들어 있는지를 불로 올린다.</td></tr>
    <tr><td><code>CHECKBOOL</code></td><td>없음</td><td>1 → 1</td><td>맨 위가 불인지 확인하고 그대로 둔다. 불이 아니면 에러다. <code>and</code>·<code>or</code>의 오른쪽 값에 쓴다.</td></tr>
  </tbody>
</table>

## 접근 {#access}

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>INDEX</code></td><td>없음</td><td>2 → 1</td><td><code>v[i]</code>. 값과 그 위의 인덱스를 꺼내 원소를 올린다.</td></tr>
    <tr><td><code>SLICE</code></td><td>플래그</td><td>1~3 → 1</td><td><code>v[a..b]</code>. 플래그의 1은 시작이 있음, 2는 끝이 있음, 4는 끝을 포함함(<code>..=</code>)이다. 값과, 적힌 시작·끝을 꺼내 조각을 올린다.</td></tr>
    <tr><td><code>FIELD</code></td><td>상수 번호</td><td>1 → 1</td><td><code>v.name</code>. 값을 꺼내, 상수(이름)로 찾은 항목을 올린다. 모듈이면 그 이름의 멤버다(없으면 E0303, 비공개면 E0304).</td></tr>
  </tbody>
</table>

## 변수 {#variables}

지역 변수는 프레임의 칸에, 최상위 이름은 파일의 전역 표에 산다. 이름은 컴파일할 때 번호로 바뀌었고([6장](06-resolve.html)), `fung dump`가 `;` 뒤에 그 이름을 적어 준다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>LOADLOCAL</code></td><td>칸 번호</td><td>0 → 1</td><td>칸의 값을 올린다. 칸은 그대로다.</td></tr>
    <tr><td><code>MOVELOCAL</code></td><td>칸 번호</td><td>0 → 1</td><td>칸의 값을 올리고 칸을 비운다. 그 변수를 마지막으로 읽는 자리에 나온다. 값을 복사하지 않고 넘긴다.</td></tr>
    <tr><td><code>STORELOCAL</code></td><td>칸 번호</td><td>1 → 0</td><td>맨 위를 꺼내 칸에 넣는다.</td></tr>
    <tr><td><code>CLEARLOCAL</code></td><td>칸 번호</td><td>0 → 0</td><td>칸을 비워 값을 놓는다. 더 읽히지 않는 변수의 값을 일찍 놓는 자리에 나온다(문장 뒤, 가지가 시작할 때, 마지막으로 붙잡은 <code>CLOSURE</code> 뒤).</td></tr>
    <tr><td><code>LOADGLOBAL</code></td><td>전역 번호</td><td>0 → 1</td><td>전역 표의 칸에 든 값을 올린다. 아직 대입되지 않은 칸이면 E0809(REPL에서는 E0808).</td></tr>
    <tr><td><code>STOREGLOBAL</code></td><td>전역 번호</td><td>1 → 0</td><td>맨 위를 꺼내 전역 표의 칸에 넣는다.</td></tr>
    <tr><td><code>LOADCAPTURED</code></td><td>붙잡은 값 번호</td><td>0 → 1</td><td>실행 중인 클로저가 만들어질 때 복사해 둔 값을 올린다([9장](09-closures.html)).</td></tr>
    <tr><td><code>LOADSELF</code></td><td>없음</td><td>0 → 1</td><td>실행 중인 클로저 자신을 올린다. 블록 안의 이름 있는 <code>fn</code>이 자기를 부를 때 쓴다.</td></tr>
  </tbody>
</table>

## 흐름 {#flow}

점프는 다음에 실행할 명령의 번호를 바꾼다. `if`, `and`·`or`, `match`의 절, 매개변수의 기본값이 이 명령들이 된다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>JUMP</code></td><td>명령 번호</td><td>0 → 0</td><td>그 번호의 명령으로 간다.</td></tr>
    <tr><td><code>JUMPIFFALSE</code></td><td>명령 번호</td><td>1 → 0</td><td>맨 위를 꺼내, 거짓이면 그 번호로 가고 참이면 다음 명령으로 간다. 불이 아니면 에러다.</td></tr>
    <tr><td><code>JUMPIFTRUE</code></td><td>명령 번호</td><td>1 → 0</td><td>맨 위를 꺼내, 참이면 그 번호로 가고 거짓이면 다음 명령으로 간다. 불이 아니면 에러다.</td></tr>
    <tr><td><code>JUMPIFSET</code></td><td>명령 번호, 칸 번호</td><td>0 → 0</td><td>둘째 피연산자의 매개변수 칸에 값이 있으면 첫째 피연산자의 번호로 간다. 인자를 받은 매개변수의 기본값 계산을 건너뛰는 데 쓴다.</td></tr>
  </tbody>
</table>

## 함수 {#functions}

호출할 때 스택에는 아래부터 함수, 위치 인자들, 그리고 이름 인자마다 (이름 심볼, 값) 둘이 놓인다. 그래서 호출 명령이 꺼내는 수는 `1 + 위치 인자 수 + 2 × 이름 인자 수`다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>CLOSURE</code></td><td>함수 번호</td><td>0 → 1</td><td>그 번호의 안쪽 함수 원형으로 함수 값(클로저)을 만들어 올린다. 그 함수가 붙잡는 바깥 값은 이때 복사한다.</td></tr>
    <tr><td><code>CALL</code></td><td>개수, 개수</td><td>1 + 인자 → 1</td><td>함수를 부르고 결과를 올린다. 첫째 피연산자는 위치 인자 수, 둘째는 이름 인자 수다. fung 함수면 새 프레임을 쌓고, 내장 함수면 그 자리에서 부른다.</td></tr>
    <tr><td><code>TAILCALL</code></td><td>개수, 개수</td><td>1 + 인자 → 1</td><td><code>CALL</code>과 같되 새 프레임을 쌓지 않고 지금 프레임을 다시 쓴다. 함수의 마지막 일이 호출일 때(꼬리 자리) 나온다. 이 프레임의 코드는 여기서 끝난다.</td></tr>
    <tr><td><code>CALLSPREAD</code></td><td>개수, 개수</td><td>1 + 인자 → 1</td><td><code>CALL</code>과 같되 마지막 위치 인자가 리스트나 범위이고, 그 원소들이 그 자리의 인자가 된다(<code>f(a, ...xs)</code>).</td></tr>
    <tr><td><code>TAILCALLSPREAD</code></td><td>개수, 개수</td><td>1 + 인자 → 1</td><td><code>TAILCALL</code>과 같되 <code>CALLSPREAD</code>처럼 펼친다.</td></tr>
    <tr><td><code>RETURN</code></td><td>없음</td><td>1 → 0</td><td>맨 위를 꺼내 프레임을 버리고, 그 값을 부른 쪽의 스택에 올린다.</td></tr>
    <tr><td><code>CHECKTYPE</code></td><td>칸 번호, 타입 번호</td><td>0 → 0</td><td>매개변수 칸의 값이 타입 표기에 맞는지 본다. 맞지 않으면 E0403.</td></tr>
    <tr><td><code>CHECKRETURN</code></td><td>타입 번호</td><td>1 → 1</td><td>맨 위(돌려줄 값)가 반환 타입에 맞는지 보고 그대로 둔다. 맞지 않으면 E0404.</td></tr>
    <tr><td><code>TAILCHECK</code></td><td>타입 번호</td><td>0 → 0</td><td>반환 타입이 있는 함수의 <code>TAILCALL</code> 앞에 나온다. 프레임에 반환 타입을 실어 보내, 그 프레임이 나중에 돌아올 때 값을 검사하게 한다(E0404).</td></tr>
    <tr><td><code>TAILCHECKBOOL</code></td><td>없음</td><td>0 → 0</td><td><code>and</code>·<code>or</code>의 오른쪽이 꼬리 호출일 때 나온다. 프레임이 돌아올 때 값이 불인지 검사하게 한다.</td></tr>
  </tbody>
</table>

## 결과값 {#results}

`Ok`·`Err`와 `nil`을 다루는 세 연산자의 명령이다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>UNWRAPABORT</code></td><td>없음</td><td>1 → 1</td><td><code>x!</code>. 맨 위가 <code>Ok(v)</code>면 <code>v</code>로 바꾼다. <code>Err</code>면 E0603, <code>nil</code>이면 E0604, 그 밖의 값이면 E0605.</td></tr>
    <tr><td><code>UNWRAPTEST</code></td><td>명령 번호</td><td>1 → 1</td><td><code>x?</code>. 맨 위가 <code>Ok(v)</code>면 <code>v</code>로 바꾼다. <code>Err</code>면 그대로 둔 채 그 번호(함수 끝에 따로 둔 <code>RETURN</code>)로 가서 함수가 그 <code>Err</code>를 돌려준다. <code>nil</code>이면 E0604, 그 밖의 값이면 E0605.</td></tr>
    <tr><td><code>COALESCE</code></td><td>명령 번호</td><td>1 → 0</td><td><code>x ?? y</code>. 맨 위가 <code>nil</code>이나 <code>Err</code>면 버리고 다음 명령(오른쪽 식)으로 간다. 그 밖의 값이면 스택에 둔 채 그 번호로 가고, <code>Ok(v)</code>는 <code>v</code>로 바꿔서 간다.</td></tr>
  </tbody>
</table>

## 패턴 {#patterns}

패턴은 명령으로 펼치지 않는다. 함수 원형의 패턴 표에 트리로 두고, 명령 하나가 값을 그 트리에 맞춰 본다. `fung dump`는 패턴이 묶는 칸을 `$1`처럼, 전역을 `@1`처럼 적는다.

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>MATCH</code></td><td>명령 번호, 패턴 번호</td><td>1 → 0</td><td>값을 꺼내 패턴에 맞춰 본다. 맞으면 패턴의 이름들을 칸에 묶고 다음 명령으로, 맞지 않으면 첫째 피연산자의 번호(다음 절)로 간다.</td></tr>
    <tr><td><code>MATCHORFAIL</code></td><td>패턴 번호</td><td>1 → 0</td><td>구조 분해 대입(<code>(a, b) = pair</code>). 값을 꺼내 패턴에 맞춰 이름을 묶는다. 맞지 않으면 E0602.</td></tr>
    <tr><td><code>NOMATCH</code></td><td>없음</td><td>1 → 0</td><td>어느 절도 맞지 않았을 때 닿는다. 값을 꺼내 E0601을 낸다.</td></tr>
  </tbody>
</table>

## 모듈과 테스트 {#modules-and-tests}

<table>
  <thead><tr><th>명령</th><th>피연산자</th><th>스택</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>IMPORT</code></td><td>상수 번호</td><td>0 → 1</td><td>상수(모듈의 열쇠: 내장 모듈의 이름이나 파일의 절대 경로)가 가리키는 모듈 값을 올린다. 처음 가져올 때 그 모듈의 본문을 실행하고, 다음부터는 기억해 둔 값을 쓴다([11장](11-modules.html)).</td></tr>
    <tr><td><code>ASSERT</code></td><td>assert 번호</td><td>1~2 → 0</td><td><code>assert</code>. 조건을 꺼내 참인지 본다. 조건이 비교면 두 쪽을 따로 꺼내 견주어, 실패했을 때 두 값을 보일 수 있게 한다. 거짓이면 E0807.</td></tr>
    <tr><td><code>TEST</code></td><td>상수 번호</td><td>1 → 0</td><td><code>test</code> 블록. 맨 위의 함수(블록의 본문)를 꺼내, 상수(테스트 이름)와 함께 테스트 목록에 올린다. 실행은 <code>fung test</code>가 나중에 한다([17장](17-test-runner.html)).</td></tr>
  </tbody>
</table>

## 표를 통째로 보기: `dump proto` {#proto}

명령의 피연산자는 대부분 함수 원형이 가진 표의 번호다. `fung dump bytecode`는 그 번호가 가리키는 것을 `;` 뒤에 적어 줄 뿐 표 자체는 보이지 않는다. `fung dump proto`는 함수마다 표를 모두 적은 뒤에 같은 코드를 보인다.

```dump proto
fn scale(x, by: int = 2):
  k = x * by
  y => y + k
;;
print(scale(3)(1))
```

함수마다 나오는 줄은 다음과 같다. 비어 있는 표는 나오지 않는다.

<table>
  <thead><tr><th>줄</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>params</code></td><td>매개변수 표. 번호가 곧 그 매개변수의 칸 번호다. <code>= ?</code>는 기본값이 있다는 표시이고, 기본값을 채우는 코드는 함수의 맨 앞에 있다.</td></tr>
    <tr><td><code>returns</code></td><td>반환 타입.</td></tr>
    <tr><td><code>slots</code></td><td>프레임이 쓰는 칸의 수.</td></tr>
    <tr><td><code>held</code></td><td>한 번도 읽지 않는 이름의 칸. 함수가 돌아갈 때까지 값을 쥔다.</td></tr>
    <tr><td><code>locals</code></td><td>칸마다의 이름과, 그 이름이 시작하는 명령 번호. 뒤 블록이 같은 칸을 다시 쓰면 그 칸이 한 줄 더 나온다. <code>$</code>로 시작하는 이름은 desugar가 만든 것이다([5장](05-desugar.html)).</td></tr>
    <tr><td><code>captures</code></td><td>붙잡은 값의 이름과, 클로저를 만드는 쪽의 어디에서 복사하는지(<code>local 3</code>은 만드는 프레임의 3번 칸).</td></tr>
    <tr><td><code>constants</code></td><td>상수 풀. <code>CONST n</code>이 가리키는 표다.</td></tr>
    <tr><td><code>types</code></td><td>타입 표기 표.</td></tr>
    <tr><td><code>patterns</code></td><td>패턴 표.</td></tr>
    <tr><td><code>asserts</code></td><td><code>assert</code>가 실패했을 때 보일 소스 글.</td></tr>
    <tr><td><code>functions</code></td><td>안쪽 함수 원형. <code>CLOSURE n</code>이 가리키는 표다.</td></tr>
    <tr><td><code>globals</code></td><td>파일의 전역 변수 표(<code>&lt;main&gt;</code>에만).</td></tr>
    <tr><td><code>exports</code></td><td>다른 파일이 가져갈 수 있는 이름(<code>&lt;main&gt;</code>에만).</td></tr>
    <tr><td><code>body</code></td><td>기본값을 채우는 서두가 끝나는 명령 번호. 모든 매개변수를 위치 인자로 받은 호출은 여기서 시작한다.</td></tr>
    <tr><td><code>code</code></td><td>명령 목록. <code>fung dump bytecode</code>가 보이는 것과 같다.</td></tr>
  </tbody>
</table>
