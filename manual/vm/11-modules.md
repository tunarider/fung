---
description: 모듈 적재와 prelude. 실행 전에 import 그래프 전체를 컴파일하는 로더, 처음 쓸 때 한 번만 도는 모듈 본문, 값으로서의 모듈과 모듈마다의 전역 표, 실행 파일에 들어 있는 fung 표준 라이브러리와 네 층.
lede: fung은 진입 파일을 실행하기 전에 그것이 가져오는 모듈 전체를 읽고 컴파일한다. 실행은 각 모듈이 처음 쓰일 때 한 번이고, 그 결과는 불변 값 하나로 남는다.
---
# 모듈 적재와 prelude

<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li>코어</li><li>바이트코드</li><li class="on">실행</li></ol>

:::goals
- 모듈 체계를 설계할 때 정해야 하는 다섯 가지 질문
- 컴파일 시점(그래프 전체를 먼저)과 실행 시점(처음 쓸 때 한 번)을 나눈 이유
- 모듈이 값이라는 것의 뜻과 모듈마다 전역 표가 따로 있는 이유
- prelude가 시작될 때 만들어지는 과정, 그리고 core·native·extras의 네 층
- 내장 fung 코드 안에서 난 에러가 사용자 코드를 가리키는 방법
:::

## 모듈이란 무엇인가 {#concept}

프로그램이 커지면 파일을 나눈다. 파일 하나가 이름 묶음 하나가 되고, 다른 파일은 그 이름을 가져다 쓴다. 이것이 **모듈**(module)이다. 모듈 체계를 만들 때는 생각보다 많은 것을 정해야 한다.

1. **어떻게 찾나.** `import "./servers"`의 `"./servers"`는 어느 파일인가. 가져오는 파일의 디렉터리가 기준인가, 지금 디렉터리가 기준인가.
2. **언제 검사하나.** 가져온 모듈에 없는 이름을 쓰면 언제 알게 되나. 실행 전인가, 그 줄에 닿았을 때인가.
3. **언제, 몇 번 실행하나.** 모듈의 최상위 코드는 언제 도는가. 두 파일이 같은 모듈을 가져오면 두 번 도는가.
4. **무엇을 주나.** `import`가 남기는 것은 이름들인가, 하나의 값인가.
5. **고리는.** `a`가 `b`를 가져오고 `b`가 `a`를 가져오면 어떻게 되나.

:::box other 다른 언어는
- **Python**의 `import`는 실행 중에 일어난다. 처음 가져올 때 모듈의 코드를 실행하고 `sys.modules`에 캐시해 두며, 다음부터는 캐시를 준다. 없는 이름을 쓰면 그 줄에 닿아서야 `AttributeError`다. 고리가 있으면 덜 만들어진 모듈을 받는다.
- **Node.js**의 CommonJS `require`도 실행 중에 파일을 읽고 실행하며 캐시한다. 고리에서는 그때까지 채워진 `exports`만 받는다.
- **JavaScript ES 모듈**은 반대쪽이다. 실행 전에 import 그래프를 모두 읽고 이름을 연결한 뒤에 본문을 실행한다.
- **Java**는 클래스를 처음 쓸 때 클래스 로더가 읽어 들인다. 컴파일러가 이름을 미리 확인하므로 없는 이름은 컴파일 에러다.
:::

:::box choice fung의 선택: 컴파일은 그래프 전체를 먼저, 실행은 처음 쓸 때 한 번
- **실행 전에 그래프 전체를 읽고 컴파일한다.** 순환 import(E0302), 없는 멤버(E0303), 비공개 이름 접근(E0304), 라이브러리 파일 최상위의 단독 문장(E0306)이 모두 실행 전에 걸린다. 모든 모듈의 문법·의미 에러가 종료 코드 2로 모이고, `fung check`가 이 로더를 그대로 쓴다(노트 M5).
- **실행은 처음 쓸 때 한 번.** 모듈 본문은 그 모듈의 `IMPORT`가 처음 실행될 때 한 번 돌고, 결과는 캐시된다. 컴파일은 전부 먼저, 실행은 필요할 때 한 번. 두 시점을 나눈 것이 핵심이다(노트 M5).
- **모듈은 불변 값이다.** 모듈의 최상위가 한 번 돈 뒤 공개 이름들을 읽어 레코드 하나로 굳힌다. 이 "스냅숏"이 안전한 이유는 9장의 규칙이다. 최상위 코드는 한 번만 돌고, 함수는 전역에 대입할 수 없고, 블록은 바깥 변수를 바꿀 수 없다. 그 뒤로 모듈의 전역을 바꿀 사람이 없다. 엄격한 스코프 규칙 덕에 모듈 체계가 단순해진 경우다(노트 M5).
:::

## 따라가기 {#walkthrough}

라이브러리 하나와 그것을 쓰는 두 파일을 만들어 보자. `servers.fung`은 최상위에서 한 줄을 출력하게 해 두었다(대입의 오른쪽에서 부른 `print`). 라이브러리 최상위에는 `import`, `fn`, `type`, 대입만 쓸 수 있기 때문이다.

```file servers.fung
# servers: a small library
_note = print("servers: body runs")

hosts = ["web1", "web2", "db1"]

fn web() = filter(hosts, h => starts_with(h, "web"))
fn _secret() = "hidden"
```

```file helper.fung
import "./servers"

fn count() = len(servers.hosts)
```

```fung run
import "./servers"
import "./helper"

print(servers.web())
print(helper.count())
print(servers)
```

`servers`는 진입 파일과 `helper` 양쪽에서 가져왔지만 본문은 한 번만 돌았다. 일의 순서는 이렇다.

1. **그래프 읽기**(`crates/fung-front/src/loader.rs`의 `Loader`). 진입 파일을 파싱하고, `import` 문만 먼저 훑어 각 모듈을 깊이 우선으로 적재한다. `servers`를 컴파일하고, `helper`로 내려가 다시 `servers`를 만나면 이미 끝난 것을 쓴다(`done`). 아직 적재 중인 모듈을 다시 만나면(`loading`) 고리이므로 경로 전체를 보여 주는 E0302다.
2. **이름을 들고 해석하기.** 모듈을 컴파일하고 나면 공개 이름 목록(`exports`)이 나온다. 진입 파일은 그 목록을 들고 이름 해석(6장)을 한다. 그래서 `servers.exits()` 같은 오타나 `servers._secret()`은 실행 전에 걸린다.
3. **실행.** 진입 파일의 `IMPORT` 명령이 처음 실행될 때 `servers`의 본문이 돈다. 본문은 새 프레임으로 같은 태스크 위에서 돌고(12장에서 볼 이어 가기 상자 `ImportBody`가 그 끝을 기다린다), 끝나면 공개 이름들이 모듈 값으로 굳어 표에 저장된다. `helper`의 `IMPORT`는 그 값을 그대로 받는다.

비공개 이름을 쓰면 실행 전에 이렇게 멈춘다(종료 코드 2).

```file servers.fung
# servers: a small library
_note = print("servers: body runs")

hosts = ["web1", "web2", "db1"]

fn web() = filter(hosts, h => starts_with(h, "web"))
fn _secret() = "hidden"
```

```fung run exit=2 error=E0304
import "./servers"

print(servers._secret())
```

고리는 경로 전체가 보인다.

```file cyc_a.fung
import "./cyc_b"

fn a() = 1
```

```file cyc_b.fung
import "./cyc_a"

fn b() = 2
```

```fung run exit=2 error=E0302
import "./cyc_a"

print(cyc_a.a())
```

### IMPORT 명령 {#import-op}

표준 모듈로 바이트코드를 보자.

```dump bytecode
import fs
import path (join)

fn double(x) = x * 2

print(fs.exists(join(".", "nothing-here")))
print(double(21))
```

- `import fs`는 `IMPORT 0` 하나와 전역 저장이다. 상수 0번은 모듈의 열쇠(`fs`, 사용자 파일이면 절대 경로)다.
- `import path (join)`은 "가져와서 `FIELD`로 꺼내 전역에 저장"일 뿐이다. 이름을 골라 가져오는 문법이 따로 있는 것이 아니다.
- `fs.exists`는 맵의 `m.name`과 같은 `FIELD`다. 모듈이 값이기 때문이다. 그래서 모듈을 변수에 담거나 인자로 넘겨도 된다.
- `import`는 끌어올린 `fn`보다도 앞에 온다(0000~0004번 뒤에 0005번 `CLOSURE`). 함수가 만들어질 때 모듈 이름이 이미 채워져 있다.

```fung run
import fs

m = fs
print(m, type_of(m))
print(m.exists("/nothing/here"))
```

## 모듈마다 전역 표 {#globals}

`a.fung`의 `fn helper()`와 `b.fung`의 `fn helper()`는 둘 다 자기 파일의 전역 0번일 수 있다. `main.fung`이 `b.run()`을 부르고 `b.run`이 `helper()`를 부르면, 그것은 **b의** `helper`여야 한다. 지금 실행 중인 진입 파일의 전역 표를 쓰면 틀린다.

fung의 답은 "**함수 값이 자기 모듈의 전역 표를 들고 다닌다**"이다. 모듈 본문을 실행할 때 그 모듈만의 전역 표(`Globals`, 곧 `Rc<RefCell<Vec<Option<Value>>>>`)를 새로 만든다. 그 안에서 만들어지는 클로저는 그 표를 함께 쥐고(`Closure::globals`), 호출하면 프레임이 그 표를 넘겨받는다(`Frame::globals`). `LOADGLOBAL`은 "지금 프레임의 전역 표"를 본다. 그래서 함수는 어디서 불리든 자기 모듈의 전역을 본다.

다른 길도 있었다. 전역 표를 프로그램에 하나만 두고 모듈마다 시작 번호를 주는 방식이다. 그러면 모듈을 적재하는 순서에 따라 번호가 바뀌어 따로 "링크" 단계가 필요하고, REPL처럼 전역이 계속 자라는 경우가 복잡해진다. 모듈마다 표 하나에는 그런 조정이 없다(노트 M5).

전역 표는 그 모듈의 함수를 담고, 그 함수는 다시 전역 표를 든다. 참조 계수로는 풀리지 않는 고리다(10장). 그래서 런타임이 끝날 때 표를 비워 고리를 끊는다.

## prelude와 네 층 {#layers}

`print`, `map`, `take`처럼 `import` 없이 쓰는 이름의 묶음을 **prelude**라고 한다. fung의 라이브러리는 네 층으로 나뉜다.

<table>
  <thead><tr><th>층</th><th>뜻</th><th>언어</th><th>예</th></tr></thead>
  <tbody>
    <tr><td>core</td><td>언어의 의미와 런타임, 언어가 보장하는 값 연산</td><td>Rust</td><td><code>print</code>, <code>map</code>·<code>filter</code>·<code>fold</code>, <code>split</code>, <code>int</code>, 태스크·채널</td></tr>
    <tr><td>native <code>_native.*</code></td><td>바깥세상과의 다리, 또는 안쪽 루프라 Rust가 필요한 것. 숨겨져 있다</td><td>Rust</td><td><code>_native.fs</code>, <code>_native.process</code>, <code>_native.net</code>, <code>_native.json</code>, <code>_native.regex</code></td></tr>
    <tr><td>extras(내장)</td><td>core와 native 위에 fung으로 쓴 것. 실행 파일 안에 들어 있다</td><td>fung</td><td><code>fs</code>·<code>cli</code>·<code>http</code>·<code>conc</code> 모듈, prelude의 <code>take</code>·<code>sum</code>·<code>exec</code></td></tr>
    <tr><td>extras(프로젝트)</td><td>사용자의 라이브러리</td><td>fung</td><td>상대 경로 <code>import</code></td></tr>
  </tbody>
</table>

예를 들어 `extras/fs.fung`는 이렇게 시작한다.

```fung frag
import _native.fs as _n

read = _n.read
write = _n.write
```

`fs.read`는 Rust로 쓴 원시 함수를 다시 내보낸 것이고, `fs.with_open`이나 `fs.read_lines`는 그 파일 안에 fung으로 쓴 함수다. `http`는 통째로 fung이다. `_native.*`는 내장 파일에서만 가져올 수 있고, 사용자 코드에서 가져오면 E0310이다.

```fung run exit=2 error=E0310
import _native.fs

print(1)
```

:::box choice fung의 선택: 핵심은 작게, 나머지는 fung으로
- 원칙 5는 "언어 핵심은 최소한, 나머지는 fung으로 직접"이다(`background.md`). 런타임은 파일·프로세스·소켓·TLS·시간·정규식 같은 원시 기능만 주고, HTTP 프로토콜·`cli` 등은 fung 라이브러리로 쓴다(open-questions).
- 공개 API의 이름·인자·기본값·에러 모양은 fung 쪽에서 정하고, Rust 쪽은 되도록 작은 원시 기능만 둔다. 그래서 공개 이름은 바꾸지 않고 다리 쪽을 고칠 수 있다(노트 M5).
- prelude에는 확실한 것만 둔다. 나중에 prelude에 더하는 것은 사용자 코드에 경고(W0103) 하나를 낼 뿐이지만, 빼는 것은 호환성을 깨기 때문이다(open-questions).
- fung 소스는 `include_str!`로 실행 파일 안에 들어간다(`crates/fung-front/src/extras.rs`). 배포는 여전히 `fung` 파일 하나다. 내장 모듈은 `import`될 때 컴파일하고, prelude만 시작할 때 컴파일한다. 스크립트 시작 시간을 지키기 위해서다(부록 [내부 구조](internals.html#modules)).
:::

### 시작할 때 prelude 만들기 {#boot}

`fung`이 스크립트를 실행하기 전에 하는 일은 `crates/fung-vm/src/boot.rs`의 `boot`에 있다.

1. Rust로 쓴 core 내장 함수(`prelude::core`)와 동시성 원시 연산(`conc::natives`), 그리고 스크립트 인자 `args`를 prelude 표에 넣는다.
2. `extras/prelude.fung`를 컴파일한다. 이 파일은 내장 파일이라 `_native.process`와 `_native.range`를 가져올 수 있다.
3. 그것을 모듈로 실행하고, 공개 이름들(`take`, `sum`, `exec` …)을 prelude 표에 더한다.

앞단(`fung-front`)은 값을 모른다. 그래서 컴파일러는 prelude 이름을 `Const::Builtin(이름)`이라는 자리 표시로 남기고, VM의 **링크** 단계(`vm/link.rs`의 `link`)가 그 자리에 실제 함수 값을 채운다. 8장에서 `CONST 0 ; <fn print>`가 보인 이유다. 이름 해석은 prelude에 어떤 이름이 있는지만 알면 된다. 그래서 앞단은 `spec/builtins.txt`를 읽는다(`builtins.rs`). 이 파일은 VM의 실제 표에서 시험(`spec_file`)이 만들어 내는 것이라, 두 쪽이 어긋나면 시험이 실패한다.

### 내장 코드 안의 에러 {#builtin-errors}

`take`는 fung으로 쓴 prelude 함수다. 잘못된 인자를 주면 에러는 그 안쪽 깊은 곳에서 난다. 그런데 사용자는 `extras/prelude.fung`를 열어 볼 수 없다. 그래서 내장 파일에서 컴파일된 원형에는 `builtin` 표시가 붙고(`loader.rs`의 `mark_builtin`), VM은 에러의 위치를 사용자 코드의 가장 가까운 호출 자리로 옮긴다. 호출 경로에는 안쪽 프레임이 그대로 보인다.

```fung run exit=1 error=E0506
print(take(5, 2))
```

## 코드에서 찾아보기 {#source}

- `crates/fung-front/src/loader.rs`: `Loader`(`compile`, `load_import`, `load_file`, `load_module`), 순환 검사(`loading`·`stack`), `Loaded`·`Build`, `mark_builtin`.
- `crates/fung-front/src/extras.rs`: 실행 파일에 넣은 fung 소스(`source`, `PRELUDE_FILES`).
- `crates/fung-front/src/builtins.rs`: 이름 해석이 쓰는 내장 이름 표.
- `crates/fung-vm/src/vm/module.rs`: 실행 시점의 모듈 표(`ModuleState`), `IMPORT`(`exec_import`, `begin_import`), 모듈 본문의 이어 가기 `ImportBody`.
- `crates/fung-vm/src/vm/link.rs`: `link`, 원형의 실행판 `RtProto`, prelude 표 `Prelude`.
- `crates/fung-vm/src/boot.rs`: `boot`, `native_prelude`, `boot_file`.
- `crates/fung-vm/src/value.rs`: 모듈 값 `Module`, 전역 표 `Globals`.
- `extras/*.fung`: 내장 모듈의 fung 소스.

:::exercise 비공개 도우미가 있는 라이브러리
`units.fung` 라이브러리를 만들어 보자. 공개 함수 `human(bytes)`는 바이트 수를 `"1.5 KiB"` 같은 문자열로 바꾸고, 계산에 쓰는 도우미 `_scale`은 밖에서 보이지 않게 한다. 진입 파일에서 가져와 몇 가지 크기를 출력하고, `units._scale`을 부르면 무엇이 언제 나는지 생각해 보자.

:::solution
```file units.fung
# units: human-readable sizes
_names = ["B", "KiB", "MiB", "GiB"]

# tenths of the size, scaled down by 1024 while it is big
fn _scale(t, i) = t >= 10240 and i < len(_names) - 1 then _scale(t // 1024, i + 1) else (t, i)

fn human(bytes):
  (t, i) = _scale(bytes * 10, 0)
  i == 0 then "${bytes} B" else "${t // 10}.${t % 10} ${_names[i]}"
;;
```

```fung run
import "./units"

each([512, 1536, 5242880], b => print(units.human(b)))
```

`units._scale(10, 0)`을 쓰면 실행 전에 E0304로 멈춘다(종료 코드 2). 이름 해석이 `units`의 공개 이름 목록을 들고 진입 파일을 보기 때문이다. `human`은 같은 모듈 안이라 `_scale`과 `_names`를 쓸 수 있다. `_names`는 한 번만 대입되므로 최상위 `fn`이 읽을 수 있다(9장). 크기는 정수로만 계산했다. 10을 곱해 소수 첫째 자리까지 들고 다니다가 마지막에 `//`와 `%`로 나눈다.
:::
:::

## 정리 {#summary}

- 로더는 진입 파일을 실행하기 전에 import 그래프 전체를 깊이 우선으로 읽고 컴파일한다. 고리, 없는 멤버, 비공개 접근, 라이브러리 최상위의 단독 문장은 모두 종료 코드 2다.
- 모듈 본문은 그 모듈의 `IMPORT`가 처음 실행될 때 한 번 돌고, 공개 이름의 불변 레코드(모듈 값)로 캐시된다. 멤버 접근은 `FIELD`다.
- 함수 값이 자기 모듈의 전역 표를 들고 다녀서, 어디서 불리든 자기 모듈을 본다.
- 라이브러리는 core·native·extras(내장)·extras(프로젝트)의 네 층이다. `_native.*`는 숨겨져 있고(E0310), 내장 fung 소스는 실행 파일 안에 있다. prelude는 시작할 때 `boot`가 만든다.
- 앞단은 prelude 이름을 자리 표시로 남기고 VM의 링크가 값을 채운다. 내장 코드 안의 에러는 사용자 코드의 호출 자리를 가리킨다.

레퍼런스: [모듈](../reference/language.html#modules), [표준 모듈](../reference/modules/fs.html). 배우기: [10장 모듈](../learn/10-modules.html). 3부는 여기까지다. 4부는 이 VM 위에서 여러 일을 동시에 돌리는 런타임을 본다.
