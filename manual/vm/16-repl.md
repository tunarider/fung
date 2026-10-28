---
description: REPL. 한 줄씩 읽고 실행하고 출력하는 루프를 바이트코드 언어 위에 만드는 법. 입력이 끝났는지 판단하기, 입력마다 따로 컴파일하되 전역은 하나로 잇기, 재정의, 에러와 교착 뒤에도 세션을 이어 가기, 직접 만든 줄 편집기.
lede: REPL은 입력마다 따로 컴파일하지만 실행은 런타임 하나에서 한다. 앞 입력의 전역 이름을 같은 번호로 미리 묶어 두어서, 정의는 남고 다시 정의할 수 있다.
---
# REPL

:::goals
- REPL이 무엇이고, 컴파일하는 언어가 REPL을 만들 때 부딪히는 문제
- "입력이 아직 끝나지 않았다"를 파서로 판단하는 법
- 입력마다 따로 컴파일하면서 전역을 하나로 잇는 법, 그리고 재정의의 규칙
- 에러·교착·Ctrl-C 뒤에도 세션을 이어 가는 법
- 터미널의 raw 모드와 직접 만든 줄 편집기
:::

## REPL이란 {#what}

**REPL**(read-eval-print loop)은 이름 그대로 한 입력을 **읽고**(read), **실행하고**(eval), 결과를 **출력하고**(print), 다시 읽는 **반복**(loop)이다. 1960년대 Lisp에서 시작했고, 지금은 Python, Node.js, Erlang 셸처럼 대부분의 동적 언어에 있다. 인자 없이 `fung`을 실행하면 fung의 REPL이 뜬다.

인터프리터가 트리를 바로 도는 언어라면 REPL은 쉽다. 트리 하나를 읽어 돌리면 된다. fung은 소스를 바이트코드로 **컴파일한 뒤** 실행하는 언어다(2부, 3부). 그래서 몇 가지를 따로 정해야 한다.

1. 사용자가 엔터를 쳤을 때, 입력이 끝났나, 아직 블록 한가운데인가.
2. 입력마다 컴파일하면 이름 해석(6장)은 앞 입력에서 정의한 이름을 어떻게 아나. 전역의 번호는 어떻게 맞추나.
3. 같은 이름을 다시 정의하면 어떻게 되나. 파일에서는 `fn`을 두 번 정의하면 에러(E0210)다.
4. 한 입력이 에러, 교착, Ctrl-C로 끝나도 세션은 살아 있어야 한다.
5. 줄 편집(커서 이동, 이전 입력 불러오기)은 누가 하나.

## 입력이 끝났는가 {#complete}

fung의 블록은 `:` … `;;`이고, 줄이 이항 연산자로 끝나면 다음 줄로 이어진다. 그래서 `fn f():`에서 엔터를 쳤다면 입력은 아직 끝나지 않았다.

이것을 위해 따로 규칙을 만들지 않았다. **파서에게 물어본다**(`crates/fung-cli/src/repl/mod.rs`의 `needs_more`). 지금까지의 글을 렉싱하고 파싱해서, 파서가 **입력의 맨 끝**에서 실패했으면 "더 읽어야 한다"로 본다. 끝이 아닌 곳에서 실패했으면 그냥 문법 에러다. 이어 받는 줄의 프롬프트는 `. `이다.

```plain
> map(hosts, h =>
.   upper(h))
["WEB1", "WEB2"]
```

파서가 이미 아는 것을 다시 적지 않으므로, 문법이 바뀌어도 REPL의 판단이 따라간다.

## 입력마다 컴파일, 전역은 하나 {#globals}

각 입력은 그 자체로 작은 진입 파일처럼 컴파일된다. 다만 로더에 세 가지 선택을 켠다(`crates/fung-front/src/loader.rs`의 `Loader`).

- <strong>`globals`</strong>: 앞 입력들이 정의한 전역 이름을 **같은 번호로 미리 묶어 둔다.** 이름 해석은 그 이름을 이미 있는 전역으로 알고, 컴파일러는 같은 번호의 `LOADGLOBAL`·`STOREGLOBAL`을 낸다.
- <strong>`redefine`</strong>: 이미 있는 이름에 `fn`이나 `import`를 다시 묶을 수 있다.
- <strong>`keep_value`</strong>: 입력의 마지막 식을 버리지 않고 입력의 값으로 남긴다. 파일에서는 마지막 식도 `POP`으로 버리고 `nil`을 돌려주지만(8장), REPL은 그 값을 출력해야 한다.

실행은 런타임 하나에서 한다. 런타임은 세션의 **전역 표 하나**를 만들어 두고(`Runtime::prepare_repl`), 입력마다 그 표를 필요한 길이로 늘려 그 위에서 입력을 돌린다(`Runtime::run_repl`). 표는 `Rc` 하나라서, 앞 입력에서 만든 함수 값도 같은 표를 쥐고 있다(11장의 "모듈마다 전역 표"). 그래서 앞에서 정의한 최상위 `fn`이 뒤에서 다시 정의된 함수를 부른다.

```plain
> fn double(x) = x * 2
> fn quad(n) = double(double(n))
> quad(1)
4
> fn double(x) = x * 3
> quad(1)
9
```

REPL의 최상위 `fn`은 전역을 **부를 때** 읽는다. 파일에서는 최상위 `fn`이 한 번만 대입되는 이름만 읽을 수 있었다(9장). REPL에서는 이름을 언제든 다시 대입할 수 있으니 그 구분이 의미가 없다(설계 08).

```plain
> y = 10
> fn gety() = y
> y = 20
> gety()
20
```

람다는 다르다. 9장의 규칙 그대로 **만들어질 때 보이는 값을 복사한다.** 그래서 람다로 만든 `quad`는 재정의 전의 `double`을 쥐고 있다.

```plain
> fn double(x) = x * 2
> quad = n => double(double(n))
> fn double(x) = x * 3
> quad(1)
4
```

### 위치와 발췌 {#positions}

에러 메시지는 소스의 줄을 발췌해 보여 준다(7장). 그런데 REPL의 "소스"는 입력마다 따로다. 그래서 입력마다 소스 지도에 **파일 하나**(`<repl>`)를 더한다. 각 입력은 앞서 있던 파일들 뒤에 이어서 번호를 매겨 컴파일하고(`Loader::compile_after`), 런타임도 같은 파일들을 받아 둔다(`Runtime::add_session_files`). 그래서 세 번째 입력에서 정의한 함수가 열 번째 입력에서 실패해도, 에러는 세 번째 입력의 줄을 보여 준다.

### 무엇을 출력하나 {#print}

입력의 마지막이 **식**이면 그 값을 fung 리터럴 모양으로 출력한다. 대입, `fn` 정의, `import`, `test`, `assert`로 끝나면 출력하지 않는다(`is_statement`). 출력은 그대로 코드에 붙여도 동작하는 모양이다(`"a"`는 따옴표와 함께, 맵은 `{name: "x"}`). 입력의 마지막 식은 버려지는 자리가 아니므로, 거기서 나온 `Err`도 E0606이 아니라 그냥 출력된다.

## 실수해도 세션은 이어진다 {#errors}

REPL은 실수하라고 있는 곳이다. 실수 하나에 세션이 죽으면 안 된다(노트 R5-tools).

- **런타임 에러**: 에러를 보고하고 다음 입력으로 간다. 전역은 남는다. 대입이 실패한 이름을 나중에 읽으면 E0808이다.
- **교착**: 그 입력만 실패시킨다. 보고는 평소와 같다. 그다음 `Runtime::recover_deadlock`이 그 입력의 태스크가 모두 되감기기를 기다리고, 교착 표시와 그것이 요청한 `exit(1)`을 지운 뒤, **새 실행 루트**를 단다. 전역(전역에 담은 atom 포함)은 남고, 세션의 종료 코드도 그대로 0이다(사용자 결정).
- <strong>`exit(n)`</strong>: 프롬프트에서 친 것이든 띄운 태스크가 부른 것이든 세션을 코드 n으로 끝낸다.
- **Ctrl-C**: 평가 중이면 그 평가를 취소한다(14장의 취소, 세션은 이어진다). 취소된 평가의 `release`가 끝나지 않아 다시 Ctrl-C를 누르면(첫 Ctrl-C 뒤 스케줄러가 적어도 한 바퀴 돈 다음에 온 것만 센다. 한 묶음에 둘이 오면 첫 취소가 돌 틈도 없이 버리게 되기 때문이다), 스케줄러가 그 평가의 태스크를 되감지 않고 **있는 그대로 버린다**(`Sched::abandon`). 버려지는 기다림들은 Rust의 `Drop`에서 채널의 줄을 떠나고, 기다리던 외부 명령은 SIGKILL로 끝낸다. "정리 없이 버린다"는 fung 쪽 정리를 건너뛴다는 뜻이지, 운영체제 자원을 흘려도 된다는 뜻은 아니다. 그리고 한 줄을 알린다: `interrupted again: the cleanup did not finish; the input's tasks were dropped`.

```plain
> hosts = ["web1", "web2"]
> 1 / 0
error[E0502]: division by zero
  --> <repl>:1:3
  |
1 | 1 / 0
  |   ^
  |
 call path (innermost first):
   <repl>:1   <entry>
> hosts
["web1", "web2"]
> (tx, rx) = channel()
> recv(rx)
error[E0702]: deadlock: all tasks are waiting
  --> <repl>:1:5
  |
1 | recv(rx)
  |     ^
  |
 call path (innermost first):
   <repl>:1   <entry>
 help: this task waits to receive, but it still holds that channel's sending end itself (the global `tx`), so the channel cannot close: close it with `close(tx)` first, or leave the sending end to the producers only
> len(hosts)
2
```

다음 입력을 기다리는 동안은 어떤 fung 코드도 돌지 않는다. 각 입력은 자기 실행(`run_root`)이고, 그 안에서 띄운 태스크는 태스크 범위 안에서만 살므로(14장) 입력이 끝나면 함께 끝나 있다. 그래서 "다음 입력을 기다리는 사이"가 교착으로 판정될 일이 없다. 세션이 끝날 때(`:quit`, 입력 끝, `exit(n)`)는 남은 것을 취소하고 되감기가 `with`의 `release`를 지난다.

## 줄 편집기 {#editor}

터미널은 보통 **표준 모드**(canonical mode)다. 사용자가 엔터를 칠 때까지 커널이 한 줄을 모아 두고, 친 글자를 화면에 다시 찍어 주고(echo), Ctrl-C를 SIGINT 신호로 바꾼다. 이 모드에서는 왼쪽 화살표가 `^[[D`라는 글자로 찍힐 뿐이다. 줄 편집을 하려면 터미널을 **raw 모드**로 바꿔, 키 하나하나를 프로그램이 바로 받고 화면도 프로그램이 직접 그려야 한다.

:::box other 다른 언어는
- **Python**의 REPL은 GNU readline이나 libedit 라이브러리를 쓴다(있으면). 3.13부터는 Python으로 쓴 새 REPL이 기본이다.
- **Node.js**는 자체 `readline` 모듈을 쓴다.
- 많은 C 프로그램이 readline을 링크하거나, 수백 줄짜리 linenoise 같은 작은 라이브러리를 넣는다.
:::

:::box choice fung의 선택: 직접 만든 작은 줄 편집기
fung의 의존성 정책은 "기본은 Rust 표준 라이브러리, REPL 줄 편집 같은 것도 직접 구현"이다. 크레이트는 허용 목록에 있는 것만 쓴다. 그래서 줄 편집기를 직접 만들었다. 운영체제 호출(`tcgetattr`·`tcsetattr`)만 `libc`를 쓰는 `fung-vm`의 `term.rs`에 두었다. `fung-cli`는 작업 공간 크레이트에만 기댈 수 있기 때문이다(노트 R5-tools).
:::

- **터미널일 때만.** 표준 입력에 터미널 설정이 있으면(`term::is_terminal`: `tcgetattr`가 성공) 줄 편집기(`TermReader`)를 쓰고, 아니면(파이프, 파일, `/dev/null`) 평범한 줄 읽기(`PlainReader`)를 쓴다. 그래서 `echo 'print(1)' | fung`이 된다.
- **raw 모드는 한 줄을 읽는 동안만.** `read_line`이 시작할 때 raw 모드를 켜는 가드(`RawMode`)를 만들고, 돌아오거나 패닉으로 되감길 때 `Drop`이 원래 설정을 되돌린다. 그래서 입력을 **실행하는** 동안 터미널은 늘 원래 모드다. `exit(n)`이나 내부 오류가 나도 터미널이 raw로 남지 않는다.
- **키.** raw 모드에서는 ISIG가 꺼져 있어 Ctrl-C가 신호가 아니라 키로 온다. 줄을 읽는 중의 Ctrl-C는 그 줄을 지운다(`^C`). Ctrl-D는 빈 줄에서 세션을 끝낸다. 좌우 이동, Home/End, Backspace/Delete, 위아래 기록, Ctrl-L(화면 지우기)이 있다.
- **편집 상태**(`Editor`)는 터미널과 떨어진 순수한 자료구조다. 글자 배열, 커서, 기록, 기록을 넘기는 중의 위치. 그래서 키 순서만으로 시험할 수 있다. 화면은 `\r` + 프롬프트 + 글 + 줄 끝 지우기(`\x1b[K`) + 커서 되돌리기로 다시 그린다. 커서는 **화면 칸 수**로 옮긴다(한글은 2칸, 15장의 포맷터와 같은 표).
- **기록 파일.** `~/.fung_history`에 받아들인 줄을 하나씩 덧붙이고, 시작할 때 읽고, 최근 1,000줄만 남긴다(`MAX_HISTORY`). 빈 줄과 바로 앞과 같은 줄은 저장하지 않는다. 쓸 수 없으면 조용히 끈다. 기록은 편의 기능이라 REPL을 멈추게 해서는 안 된다.

명령은 셋뿐이다. `:help`, `:quit`, `:load 파일`. `:load`는 파일을 세션의 입력으로 실행하므로 그 정의가 세션에 남는다.

## 코드에서 찾아보기 {#source}

- `crates/fung-cli/src/repl/mod.rs`: 시작 `start`, 세션 `Session`(`run`, `eval`, `command`, `finish`), 입력이 끝났는지 보는 `needs_more`, 출력하지 않는 형태 `is_statement`.
- `crates/fung-cli/src/repl/line.rs`: `LineReader`, 줄 편집기 `TermReader`, 평범한 읽기 `PlainReader`.
- `crates/fung-cli/src/repl/editor.rs`: 편집 상태 `Editor`, 키 `Key`.
- `crates/fung-cli/src/repl/history.rs`: 기록 파일 `History`, `MAX_HISTORY`.
- `crates/fung-vm/src/term.rs`: `is_terminal`, raw 모드 가드 `RawMode`.
- `crates/fung-vm/src/vm/runtime.rs`: `prepare_repl`, `run_repl`, `add_session_files`, `recover_deadlock`, `recover_interrupt`, `take_abandoned`.
- `crates/fung-front/src/loader.rs`: `Loader`의 `globals`·`redefine`·`keep_value`, `compile_after`.

:::exercise 재정의 맞히기
REPL에 아래를 차례로 입력하면 마지막 두 줄은 무엇을 출력할까? 먼저 예측하고 `fung`을 띄워 확인해 보자.

```plain
> fn double(x) = x * 2
> quad = n => double(double(n))
> fn quad2(n) = double(double(n))
> fn double(x) = x * 3
> quad(1)
> quad2(1)
```

:::solution
```plain
> quad(1)
4
> quad2(1)
9
```

`quad`는 람다라서 만들어질 때 보이는 값, 곧 처음의 `double`을 복사해 쥐고 있다. `quad2`는 REPL의 최상위 `fn`이라 부를 때 전역 표에서 `double`을 읽고, 그때는 다시 정의된 `x * 3`이 들어 있다. 파일에서도 람다의 규칙은 같다. 최상위에서 만든 람다는 그때까지 대입된 값을 복사한다.

```fung run
x = 1
f = () => x
x = 2
print(f(), x)
```
:::
:::

## 정리 {#summary}

- REPL은 읽기·실행·출력의 반복이다. 컴파일하는 언어에서는 입력의 끝, 입력 사이의 이름, 재정의, 실패 뒤의 회복을 따로 정해야 한다.
- 입력이 끝났는지는 파서가 입력의 맨 끝에서 실패했는지로 판단한다.
- 입력마다 따로 컴파일하되, 앞 입력의 전역을 같은 번호로 미리 묶고(`globals`) 다시 정의를 허락한다(`redefine`). 실행은 세션의 전역 표 하나 위에서 한다. REPL의 최상위 `fn`은 부를 때 전역을 읽고, 람다는 만들 때 복사한다.
- 런타임 에러와 교착은 그 입력만 실패시키고 세션은 새 실행 루트에서 이어진다. 두 번째 Ctrl-C는 정리가 멈춘 평가의 태스크를 버린다.
- 줄 편집기는 직접 만들었다. raw 모드는 한 줄을 읽는 동안만 켜고, 가드의 `Drop`이 되돌린다.

레퍼런스: [명령줄 도구](../reference/cli.html#repl). 배우기: [2장 첫 프로그램](../learn/02-first-program.html). 다음 장은 마지막 도구, 테스트 러너를 본다.
