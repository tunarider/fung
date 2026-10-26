---
description: 에러 메시지 만들기.
lede: 좋은 에러 메시지는 어디서, 무엇이, 어떻게 고치면 되는지를 말한다. 그러려면 위치가 토큰에서 바이트코드까지 끊기지 않고 따라와야 하고, 메시지는 코드와 떨어진 카탈로그에 있어야 한다.
---
# 에러 메시지 만들기

:::goals
- 좋은 에러 메시지의 세 요소: 위치, 설명, 고치는 법
- 위치가 토큰에서 트리, 바이트코드, 실행까지 따라오는 방법
- 소스 발췌와 밑줄을 그리는 법
- 에러 카탈로그와 번호 체계, 종료 코드
- 컴파일 에러와 런타임 에러의 보고 방식 차이
:::

## 좋은 에러 메시지 {#good}

언어 구현이 내는 에러 메시지를 **진단**(diagnostic)이라고 부른다. 오래된 컴파일러들은 `syntax error`나 `line 12: parse error` 한 줄만 찍곤 했다. 무엇이 틀렸는지 찾는 일은 사람 몫이었다. 지금은 대부분의 언어가 메시지에 정성을 들인다. 사용자가 언어와 가장 많이 대화하는 곳이 에러 메시지이기 때문이다.

fung의 에러 하나를 뜯어보자.

```plain
error[E0204]: undefined name `doubel`          ← 머리줄: 종류, 번호, 설명
  --> example.fung:2:7                          ← 위치: 파일:줄:칸
  |
2 | print(doubel(21))                           ← 그 줄의 발췌
  |       ^^^^^^ not found in this scope        ← 밑줄과 라벨
  |
 help: a similar name exists: `double`          ← 고치는 법
```

1. **어디서**: 파일, 줄, 칸, 그리고 그 줄을 그대로 보여 주고 밑줄을 긋는다. 사람이 줄 번호를 세며 파일을 뒤질 필요가 없다.
2. **무엇이**: 번호(`E0204`)와 한 줄 설명. 번호는 검색하거나 문서에서 찾을 때 쓴다.
3. **어떻게**: 도움말(`help`). 가능하면 구체적인 고칠 거리를 준다.

이 장에서는 이 세 가지를 어떻게 만드는지 본다. 단계 띠가 없는 것은 진단이 모든 단계에 걸쳐 있기 때문이다.

## 위치를 끝까지 들고 가기 {#positions}

에러가 정확한 칸을 가리키려면, 에러가 나는 곳에서 **그 칸을 알고 있어야** 한다. 렉서는 쉽다. 글자를 읽고 있으니 지금 칸을 안다. 그러나 이름 해석이나 VM은 글자를 보지 않는다. 그래서 위치를 각 단계의 결과물에 **같이 실어 보낸다**.

- 렉서: 토큰마다 위치(3장).
- 파서: 트리 노드마다 위치(`Node`의 `pos`, 4장). 보통 그 노드를 대표하는 토큰의 위치다. 이항 연산 노드는 연산자의 위치를 갖는다.
- desugar와 이름 해석: 새 노드를 만들 때 원래 노드의 위치를 그대로 물려준다.
- 컴파일러: **명령마다 위치 하나**. 함수 원형(`Proto`)에 명령 목록 `code`와 나란한 위치 목록 `pos`가 있다.
- VM: 명령을 실행하다 실패하면 그 명령의 위치를 에러에 붙인다.

바이트코드 출력의 둘째 열이 바로 그 위치다.

```dump bytecode
fn double(x) = x * 2
print(double(21))
```

`double`의 `MUL`은 1:18, 곧 `*`가 있던 자리를 기억한다. 그래서 곱셈이 실패하면 VM은 정확히 그 `*`를 가리킬 수 있다.

```fung run exit=1 error=E0501
fn double(x) = x * 2
print(double("21"))
```

재미있는 점은 곱셈을 실제로 하는 함수(값 층의 `value::mul`)는 자기가 소스의 어디에서 불렸는지 모른다는 것이다. 그 함수는 "str과 int는 곱할 수 없다"는, 위치 없는 에러를 돌려준다. VM이 그것을 받아 방금 실행한 명령의 위치를 채운다. 값을 다루는 코드는 소스를 몰라도 되고, 위치는 한 곳(VM)에서만 붙인다.

### 칸과 밑줄 {#span}

위치의 칸은 글자 단위다(3장). 밑줄의 길이는 에러를 만드는 쪽이 정한다. 이름 해석은 이름의 글자 수만큼 밑줄을 긋는다(`doubel`은 6칸). 길이를 주지 않으면 한 칸이다.

발췌를 그릴 때는 탭을 공백 네 칸으로 펴서 보여 주고, 밑줄도 펴진 쪽에 맞춘다. 칸 번호 자체는 탭을 한 글자로 센다.

```plain
error[E0204]: undefined name `y`
  --> example.fung:2:9
  |
2 |     return y
  |            ^ not found in this scope
  |
```

이 소스의 둘째 줄은 탭 하나로 시작한다. `y`는 아홉째 글자라 위치는 2:9지만, 화면에서는 탭이 네 칸으로 펴졌으므로 밑줄은 열두째 칸에 그어진다. 탭을 펴지 않으면 터미널마다 탭 너비가 달라 밑줄이 엉뚱한 곳을 가리킨다.

## 카탈로그 {#catalog}

메시지 글은 에러를 내는 코드에 직접 쓰지 않는다. 모든 에러와 경고의 글은 **카탈로그**(catalog) 한 곳에 모여 있다. 카탈로그의 한 줄은 넷이다.

```plain
번호    메시지 틀                                    밑줄 라벨                 도움말
E0204   undefined name `%s`                          not found in this scope
E0205   cannot reassign `%s` here: it was defined…                             a block cannot change an outer variable; …
E0502   division by zero
```

에러를 내는 코드는 번호와 **인자**만 준다. `error("E0204", 위치, ["doubel"])`. 메시지 틀의 `%s`가 인자로 채워진다. `%q`는 인자를 fung 문자열 리터럴 모양(따옴표와 이스케이프)으로, `%d`는 정수로 채운다. 그래서 `malformed number "30abc"`처럼 따옴표가 붙는다.

카탈로그로 분리하면 좋은 점이 여럿이다.

- **메시지를 한 곳에서 다듬는다.** 같은 에러가 여러 곳에서 나도 문장이 하나다.
- **번역할 수 있다.** 지금은 영어뿐이지만, 다른 언어는 카탈로그를 하나 더 두면 된다(설계 문서 08).
- **목록이 문서가 된다.** 저장소의 `spec/errors.txt`는 이 카탈로그에서 생성한 파일이고, 테스트가 둘이 같은지 지킨다. [에러 레퍼런스](../reference/errors.html)도 같은 표에서 메시지와 도움말을 가져온다.

도움말은 카탈로그의 기본값이 있고, 에러를 내는 쪽이 상황에 맞는 문장으로 바꿀 수 있다. `E0204`의 "a similar name exists: `double`"은 이름 해석이 비슷한 이름을 찾았을 때만 붙인다.

### 번호 체계 {#numbering}

번호는 `E` + 네 자리다. 앞 두 자리가 분류, 뒤 두 자리가 일련번호다(설계 문서 08).

<table>
  <thead><tr><th>분류</th><th>내용</th><th>내는 곳</th></tr></thead>
  <tbody>
    <tr><td><code>E01xx</code></td><td>문법</td><td>렉서·파서</td></tr>
    <tr><td><code>E02xx</code></td><td>이름·스코프</td><td>이름 해석</td></tr>
    <tr><td><code>E03xx</code></td><td>모듈</td><td>모듈 적재, 이름 해석</td></tr>
    <tr><td><code>E04xx</code></td><td>호출·타입 표기</td><td>VM(인자 묶기, 타입 검사)</td></tr>
    <tr><td><code>E05xx</code></td><td>런타임 연산</td><td>VM, 값 층</td></tr>
    <tr><td><code>E06xx</code></td><td>패턴·결과값</td><td>VM</td></tr>
    <tr><td><code>E07xx</code></td><td>동시성</td><td>런타임</td></tr>
    <tr><td><code>E08xx</code></td><td>한도·환경, 내부 오류</td><td>여러 곳</td></tr>
    <tr><td><code>W01xx</code></td><td>경고</td><td>이름 해석</td></tr>
  </tbody>
</table>

번호는 **한 번 붙이면 바꾸지 않는다**. 골든 테스트와 사용자가 번호에 기대기 때문이다. 쓸모가 없어진 번호는 폐기하고 다시 쓰지 않는다. 예를 들어 `E0207`은 스코프 모델이 바뀌어 날 수 없게 되어 폐기되었다(설계 문서 08).

## 컴파일 에러와 런타임 에러 {#kinds}

### 보고하는 범위 {#scope}

- **문법 에러**(렉서·파서): 첫 번째 하나만. 문법이 어긋난 뒤의 구조는 믿을 수 없다(4장).
- **의미 에러**(이름 해석): 파일 전체를 검사한 뒤 모두(6장).
- **런타임 에러**: 하나. 에러가 나면 그 태스크의 실행이 되감기며 끝난다. fung에는 런타임 에러를 잡는 구문이 없다. 실패할 수 있는 일은 값으로(`Ok`/`Err`) 돌려준다.

### 호출 경로 {#call-path}

런타임 에러에는 컴파일 에러에 없는 정보가 하나 더 있다. **호출 경로**(call path), 곧 "어느 함수가 어느 함수를 불러 여기까지 왔나"다. VM이 에러를 만들 때 지금 쌓인 프레임(실행 중인 함수들, 8장)을 안쪽부터 읽어 적는다. 각 프레임의 줄은 그 프레임이 지금 실행 중인 명령의 위치에서 온다.

```fung run exit=1 error=E0502
fn half(x) = x / 0
fn twice(x) = half(x) + 1
print(twice(21))
```

안쪽(에러가 난 곳)이 맨 위다. 프레임이 열두 개를 넘으면 안쪽 여덟 개와 바깥 두 개만 보이고 사이는 `… N more`로 줄인다. 깊은 재귀에서 경로가 수만 줄이 되지 않게 하려는 것이다(2장의 `E0804` 예).

꼬리 호출(2장)은 프레임을 새로 쌓지 않고 재사용하므로, 재사용된 호출은 경로에 나타나지 않는다. 대신 "이 프레임을 몇 번 재사용했나"를 세어 두었다가 보여 준다.

```fung run exit=1 error=E0502
fn countdown(n) = n == 0 then 10 / n else countdown(n - 1)
print(countdown(5))
```

### 경고 {#warnings}

경고는 에러와 같은 모양으로 찍되 머리가 `warning[W0102]`이다. 프로그램은 그대로 실행된다.

```fung check
fn f():
  unused = 1
  2
;;

print(f())
```

```expect check
warning[W0102]: unused variable `unused`
  --> example.fung:2:3
  |
2 |   unused = 1
  |   ^^^^^^
  |
 help: prefix it with `_` if it is intentional
```

경고는 `fung check`와 `fung test`에서만 보인다. `fung run`은 경고를 내지 않는다. 매일 도는 스크립트가 표준 에러에 경고를 뿌리면 cron 메일과 로그가 지저분해지기 때문이다(설계 문서 08).

### 패닉은 사용자에게 가지 않는다 {#panic}

VM이나 내장 함수 자체에 버그가 있어 Rust 코드가 패닉(panic, Rust의 "더는 진행할 수 없음" 중단)하면, Rust는 보통 내부 스택 추적을 찍고 프로세스를 끝낸다. fung은 Rust에서 fung 코드로 들어가는 입구에서 패닉을 잡아(`catch_unwind`) `E0801 internal error`로 바꾼다. 사용자는 구현 언어의 스택 추적을 보지 않는다(설계 문서 00 "구현 언어와의 관계").

### 종료 코드 {#exit-codes}

`fung`은 실패의 종류를 종료 코드로도 알린다. 쉘 스크립트(`set -e`, `&&`)와 테스트 실행기가 실패 종류를 구분할 수 있도록 고정해 두었다(설계 문서 08).

<table>
  <thead><tr><th>코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td>0</td><td>정상 종료</td></tr>
    <tr><td>1</td><td>실행 중 실패(런타임 에러, <code>!</code>가 <code>Err</code>를 만남, <code>exit(1)</code> 등)</td></tr>
    <tr><td>2</td><td>컴파일 실패(문법·의미 에러). <code>fung check</code>가 문제를 찾았을 때도 2</td></tr>
    <tr><td>3</td><td><code>fung</code> 명령 자체의 사용 오류(모르는 하위 명령, 잘못된 인자)</td></tr>
  </tbody>
</table>

```shell
fung check deploy.fung && fung deploy.fung    # 검사를 통과해야 실행
```

:::box other 다른 언어는
- **Elm과 Rust**: 친절한 에러 메시지로 이름난 두 언어다. 소스 발췌, 밑줄, 도움말, 에러 번호를 붙이는 fung의 형식은 이 둘을 따랐다(설계 문서 08).
- **Python**: 런타임 에러의 호출 경로(traceback)를 **바깥쪽부터** 찍고 에러가 난 곳을 맨 아래에 둔다("most recent call last"). fung은 반대로 안쪽부터 찍는다.
- **C 컴파일러**: 오래도록 짧은 메시지가 전통이었다. Clang이 밑줄과 고칠 거리를 보여 주는 메시지로 바꾸면서 GCC도 따라왔다.
- **에러 번호**: Rust(`E0382`)와 C#(`CS0103`)이 번호를 쓴다. 번호로 문서를 찾을 수 있다.
:::

:::box choice fung의 선택
설계 문서 08에서 정한 것들이다.
- **모든 명령이 소스 위치를 가진다.** 처음부터 설계에 넣었다. 런타임 에러가 정확한 줄과 칸을 가리키는 이유다. 위치는 명령 목록과 나란한 배열에 담는다(압축은 나중).
- **메시지는 카탈로그로 분리한다.** 처음에는 영어만, 다른 언어는 카탈로그 추가로.
- **번호는 바꾸지 않고, 폐기한 번호는 재사용하지 않는다.** 골든 테스트와 사용자가 번호에 기댄다.
- **문법 에러는 첫 번째 하나만, 의미 에러는 모두.** 단, 같은 원인에서 따라 나오는 에러는 억제한다.
- **경고는 검사와 테스트에서만** 보인다(`fung check`, `fung test`).
- **종료 코드는 0·1·2·3으로 고정한다.**
:::

## 코드에서 찾아보기 {#source}

- `crates/fung-front/src/diag.rs`: 진단 하나 `Diag { code, pos, len, args, help, notes, call_path }`. 만드는 함수 `error(code, pos, args)`와 꾸미는 메서드 `Diag::span`(밑줄 길이), `Diag::with_help`, `Diag::with_pos`(위치가 없을 때만 채운다), `Diag::with_call_path`.
  - `message`: 카탈로그의 틀을 인자로 채운다(`%s` `%q` `%d` `%v`). `headline`: `error[E0204]: …` 머리줄.
  - `format`: 진단 전체를 찍는다. 발췌는 `write_excerpt`(탭 펴기 `TAB_WIDTH`), 호출 경로는 `write_call_path`(`CALL_PATH_HEAD` 8, `CALL_PATH_TAIL` 2).
  - 종료 코드 `EXIT_OK`·`EXIT_RUNTIME`·`EXIT_COMPILE`·`EXIT_USAGE`, 그리고 `is_compile_error`.
- `crates/fung-front/src/diag/catalog.rs`: 카탈로그 표 `CATALOG`(번호, 틀, 라벨, 도움말)와 찾기 `entry`. `spec/errors.txt`는 `FUNG_UPDATE_SPEC=1 cargo test spec_file`로 이 표에서 생성한다.
- `crates/fung-front/src/source.rs`: `Pos`, 파일 번호 `FileId`, 파일 이름과 내용을 모아 두는 `SourceMap`(발췌를 그릴 때 쓴다).
- `crates/fung-front/src/bytecode.rs`: `Proto`의 `pos`(명령마다 위치)와 `Proto::emit`(명령과 위치를 함께 넣는다).
- `crates/fung-vm/src/vm.rs`: `at`(값 층의 위치 없는 에러에 명령 위치를 채움), `Task::call_path`(프레임을 안쪽부터 읽어 `CallSite` 목록으로), 패닉을 잡는 `catch_unwind`.

:::exercise 호출 경로 읽기
아래 프로그램은 실패한다. 실행하기 전에 에러 번호, 가리키는 줄과 칸, 호출 경로의 줄들을 예상해 보자.

```fung frag
fn avg(xs) = sum(xs) // len(xs)
fn report(name, xs) = "${name}: ${avg(xs)}"
print(report("web", [3, 5]))
print(report("db", []))
```

:::solution
첫 `print`는 `web: 4`를 찍는다. 둘째는 빈 리스트라 `len(xs)`가 0이고 `//`에서 `E0502 division by zero`가 난다. 위치는 1줄의 `//`. 호출 경로는 안쪽부터 `avg`(1줄), `report`(2줄), `<entry>`(4줄)다. `report`의 마지막 일이 `avg` 호출이 아니라 문자열 잇기라서 `avg` 호출은 꼬리 호출이 아니고, `report`의 프레임이 경로에 남는다.

```fung run exit=1 error=E0502
fn avg(xs) = sum(xs) // len(xs)
fn report(name, xs) = "${name}: ${avg(xs)}"
print(report("web", [3, 5]))
print(report("db", []))
```

:::

:::

## 정리 {#summary}

- 좋은 에러는 어디서(위치와 발췌), 무엇이(번호와 설명), 어떻게(도움말)를 말한다.
- 위치는 토큰 → 트리 노드 → 명령마다 하나씩으로 끝까지 따라온다. 값 층의 에러에는 VM이 실패한 명령의 위치를 채운다.
- 칸은 글자 단위, 발췌의 탭은 네 칸으로 펴고 밑줄도 맞춘다.
- 메시지는 카탈로그에 있고, 코드는 번호와 인자만 준다. 번호는 분류별로 붙이고 한 번 붙으면 바꾸지 않는다.
- 문법 에러는 하나, 의미 에러는 모두, 런타임 에러는 하나와 호출 경로. 종료 코드는 0·1·2·3이다.

다음 장부터 3부, 뒷단이다. 코어 트리가 바이트코드가 되고 스택 머신이 그것을 실행한다. 모든 에러의 설명은 [에러 레퍼런스](../reference/errors.html)에 있다.
