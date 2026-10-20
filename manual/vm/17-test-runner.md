---
description: 테스트 러너. 테스트를 언어 안에 두는 방법, test 블록이 함수로 컴파일되어 등록되는 과정, 비교의 양쪽 값을 보여 주는 assert, 파일마다 새 런타임, exit와 교착 뒤에 나머지를 건너뛰는 규칙, 출력과 종료 코드.
lede: test 블록은 그 자리에서 만들어지는 인자 없는 함수다. fung test는 파일마다 새 런타임에서 최상위를 먼저 실행하고, 등록된 블록을 차례로 돌린다.
---
# 테스트 러너

:::goals
- 테스트를 언어 밖(프레임워크)에 두는 것과 언어 안에 두는 것의 차이
- `test` 블록이 컴파일되고 등록되고 실행되는 과정
- 실패한 `assert`가 비교의 양쪽 값을 보여 주는 방법과 그 한계
- 파일마다 새 런타임을 쓰는 이유, 그리고 `exit`·교착 뒤에 나머지를 건너뛰는 규칙
:::

책의 마지막 장이다. 테스트 러너는 앞에서 본 거의 모든 장치(로더, 컴파일러, 런타임, 스케줄러, 취소)를 "여러 번 나눠 실행하기" 위해 다시 조합한다.

## 테스트를 어디에 두나 {#where}

**단위 테스트**(unit test)는 함수 하나가 기대대로 동작하는지 확인하는 작은 코드다. 언어마다 테스트를 두는 자리가 다르다.

- **언어 밖의 프레임워크.** 테스트는 보통 함수이고, 이름 규칙(`test_`로 시작)이나 표시로 찾는다. 실행기는 따로 설치하는 도구다.
- **언어 안.** 컴파일러가 테스트를 알고, 테스트 코드는 평소 빌드에서 빠진다.

:::box other 다른 언어는
- **Python**의 pytest는 `test_`로 시작하는 함수를 찾아 돌린다. 평범한 `assert a == b`가 실패하면 `a`와 `b`의 값을 보여 주는데, 이것은 pytest가 테스트 파일을 불러올 때 `assert` 문의 트리를 **고쳐 쓰기** 때문이다.
- **Rust**는 `#[test]` 표시를 컴파일러가 안다. `cargo test`는 테스트를 포함한 별도 실행 파일을 만들고, 평소 빌드에서는 `#[cfg(test)]` 코드가 빠진다. `assert_eq!` 매크로가 양쪽 값을 보여 준다.
- **Elixir**의 ExUnit은 `assert`가 매크로라서 컴파일 때 식의 모양을 보고 양쪽 값을 보여 준다.
:::

:::box choice fung의 선택: 같은 파일의 test 블록, 컴파일러가 아는 assert
- `test "이름": … ;;` 블록을 코드와 **같은 파일**에 둔다. 따로 설치할 것이 없고, 함수 옆에 그 함수의 테스트가 있다(설계 08).
- 블록의 본문은 **인자 없는 함수**다. 그 자리에서 만들어지므로(9장의 규칙, 그때 보이는 파일 변수를 복사) `return`과 `?`를 쓸 수 있고, 하나가 실패해도 다음 블록이 돈다.
- `assert`는 함수가 아니라 **키워드**다. 컴파일러가 식의 모양을 알아야 양쪽 값을 보여 줄 수 있기 때문이다.
- `fung run`은 `test` 블록을 무시하고, `fung test`만 돌린다.
:::

## test 블록이 함수가 되기까지 {#compile}

`test` 블록은 최상위에만 쓸 수 있다(E0114). 로더의 `tests` 선택(`crates/fung-front/src/loader.rs`)이 꺼져 있으면, 곧 `fung run`·`fung dump`에서는 블록이 아예 컴파일되지 않는다. 그래서 테스트가 있는 파일도 평소처럼 실행된다.

```fung run
fn double(x) = x * 2

servers = ["web1"]

test "sees one server":
  assert len(servers) == 1
;;

servers = servers + ["web2"]
print("top level ran with", len(servers), "servers")

test "sees two servers":
  assert len(servers) == 2
  assert double(21) == 42
;;
```

`fung test`는 `tests`를 켜고 컴파일한다. 그러면 블록마다 두 가지가 생긴다.

1. 블록의 본문이 **자식 원형 하나**로 컴파일된다. 람다와 똑같다.
2. 블록이 있던 자리에 `CLOSURE`(함수 값 만들기)와 <strong>`TEST` 명령</strong>이 들어간다. `TEST`는 만든 함수를 이름과 함께 런타임의 테스트 목록(`World::tests`)에 **등록**만 한다(`crates/fung-vm/src/vm/m6.rs`의 `exec_m6`, `TestCase`). 실행하지는 않는다.

그래서 최상위가 끝까지 돌고 나면 블록들이 순서대로 등록되어 있다. 각 블록은 **자기 자리에서** 만들어졌으므로, 첫 블록은 `servers`가 하나일 때의 값을, 둘째 블록은 둘일 때의 값을 복사해 쥐고 있다.

```plain
$ fung test order_test.fung
top level ran with 2 servers
ok   order_test.fung: sees one server
ok   order_test.fung: sees two servers
2 passed, 0 failed
```

## assert {#assert}

`assert` 식이 거짓이면 E0807이다. `fung run` 중이면 보통 런타임 에러(종료 코드 1)이고, `fung test` 안에서는 실행기가 그 에러를 `FAIL`로 기록하고 다음 블록으로 넘어간다.

실패 메시지가 쓸모 있으려면 `x == 5`가 거짓이었다는 것만이 아니라 <strong>`x`가 무엇이었는지</strong>를 보여야 한다. 그런데 실행이 끝난 뒤에는 `x == 5`의 결과 `false`만 남는다. 그래서 컴파일러가 미리 식을 쪼갠다(`crates/fung-front/src/compiler/m6.rs`). 식의 **맨 바깥이 비교 연산**(`==`·`!=`·`<`·`>`·`<=`·`>=`·`in`·`not in`)이면,

- 비교를 하지 않고 왼쪽과 오른쪽 값을 각각 스택에 올린 뒤 `ASSERT` 한 명령을 낸다.
- 원형의 `asserts` 표에 연산자와 각 쪽의 **소스 글**(`double(21)`)을 적어 둔다. 리터럴인 쪽(`42`)은 글이 곧 값이라 적지 않는다.
- 실행 중 `ASSERT`가 두 값을 꺼내 비교하고, 실패하면 소스 글과 값을 짝지어 메모로 붙인다.

```dump bytecode
fn double(x) = x * 2

assert double(21) == 42
print("ok")
```

`<main>`의 0006번 `ASSERT 0` 앞에 `EQ`가 없다. 두 값이 그대로 `ASSERT`에 간다. 실패하면 이렇게 보인다.

```fung run exit=1 error=E0807
fn double(x) = x * 2

assert double(2) == 5
```

맨 바깥이 비교가 아니면(`assert is_valid(x)`) 식 전체의 값만 보인다. 더 깊이 쪼개지 않는 이유가 있다. 안쪽 식을 하나하나 보여 주려면 그 식들을 다시 평가하거나 모든 중간값을 붙잡아야 하는데, 부수효과가 있는 식(`assert pop(queue) == 1`)을 두 번 평가하면 결과가 달라진다(설계 08). 맨 바깥 비교의 두 쪽은 원래도 한 번씩 평가되므로 공짜로 얻을 수 있다.

## 실행기 {#runner}

`fung test [경로]`(`crates/fung-cli/src/test.rs`의 `run_test`)는 이렇게 돈다.

1. **파일 고르기.** 경로가 디렉터리면 그 아래 `.fung` 파일을 모두 바이트 순서로 고른다. `_`나 `.`으로 시작하는 디렉터리(픽스처, `.git`)는 들어가지 않는다. `fung fmt`와 같은 규칙이다(`paths.rs`의 `fung_files`). 인자가 없으면 지금 디렉터리다.
2. **파일마다**(`test_file`):
   1. `tests`를 켜고 그래프 전체를 컴파일한다(11장). 실패하면 `FAIL 경로: <load>`. 여러 파일이 같은 라이브러리 때문에 깨지면 그 에러는 한 번만 찍는다.
   2. 진입 원형에 `TEST` 명령이 하나도 없으면 테스트가 없는 파일이다. **실행하지도 목록에 넣지도 않는다**(`has_tests`). 그래서 스크립트가 섞인 디렉터리에서 `fung test`를 돌려도 스크립트가 실행되지 않는다.
   3. **새 런타임**을 만들고 prelude를 부팅한 뒤 최상위를 실행한다. 이때 블록들이 등록된다.
   4. 등록된 블록을 순서대로 하나씩 실행한다(`Runtime::call_test`). 블록마다 루트 아래 새 취소 노드에서 자기 실행(`run_root`, 12장)으로 돈다. 블록이 끝나면 그 노드를 취소한다. 블록이 범위 밖에 남긴 것은 없지만(14장), 있어도 함께 끝난다.
   5. 파일이 끝나면 루트 아래의 남은 것을 취소한다(`cancel_all`). 실행마다 그 태스크가 모두 끝나야 돌아오므로(12장) 실제로 기다릴 것은 남아 있지 않다. 처리되지 않은 태스크 실패(취소된 `supervise`의 대기열)가 있었으면 `FAIL 경로: <tasks>`.
3. **요약.** `N passed, M failed`를 찍고 종료 코드를 정한다. 컴파일 실패가 있으면 2, 실패한 테스트가 있으면 1, 아니면 0.

```plain
$ fung test slug.fung
ok   slug.fung: spaces become hyphens
FAIL slug.fung: keeps digits
  error[E0807]: assertion failed: slugify("Release 0.4") == "release-0-4"
    --> slug.fung:8:3
    |
  8 |   assert slugify("Release 0.4") == "release-0-4"
    |   ^
    |
    slugify("Release 0.4") = "release-0.4"
   call path (innermost first):
     slug.fung:8 test: keeps digits
FAIL slug.fung: trims
  error[E0807]: assertion failed: slugify(" a ") == "a"
    --> slug.fung:12:3
     |
  12 |   assert slugify(" a ") == "a"
     |   ^
     |
    slugify(" a ") = "-a-"
   call path (innermost first):
     slug.fung:12 test: trims
1 passed, 2 failed
```

호출 경로의 `test: keeps digits`가 블록이 함수라는 흔적이다.

### 파일 하나에 런타임 하나 {#runtime-per-file}

`fung test`가 파일마다 새 `Runtime`을 만드는 것은 한 파일의 상태가 다음 파일로 새지 않게 하려는 것이다(노트 R5-tools). 새면 안 되는 것은 전역에 담은 atom, 요청된 `exit`, 보고된 교착, 처리되지 않은 실패 같은 것이다. 한 파일 안의 블록들은 같은 런타임을 쓴다. 최상위에서 정의한 것을 블록들이 함께 쓰기 때문이다.

### exit와 교착 뒤에는 나머지를 건너뛴다 {#skip}

테스트 파일 안에서 `exit(n)`이 불리거나(띄운 태스크에서도) 교착이 보고되면, 그 파일의 프로그램은 루트가 취소된 상태다(14장). 남은 블록을 그 위에서 돌리면 호출이 있는 블록은 E0701로 실패하고 호출이 없는 블록은 통과하는, 이상한 결과가 난다. 그래서 규칙을 두었다(설계 08, 사용자 결정).

- 돌던 블록은 평소대로 `ok`/`FAIL` 줄을 낸다.
- 남은 블록이 있으면 메모 한 줄을 붙이고 **나머지를 건너뛴다.**
- 건너뛴 블록은 세지 않지만 **건너뛰기 자체를 실패 하나로 센다.** 메모 앞의 블록이 통과했으면 `FAIL 경로: <rest of file>` 줄을 따로 낸다. "조용히 끝나는 테스트 실행은 없다"는 원칙 때문이다.
- 명령의 종료 코드는 n이 되지 않는다. 테스트 안의 `exit`는 테스트의 실패다.

```plain
$ fung test exit_test.fung
ok   exit_test.fung: first
FAIL exit_test.fung: calls exit
  exit(3) called inside a test file; the rest of the file was skipped
1 passed, 1 failed
```

교착도 같다. 블록 안에서 채널을 기다리는데 보내는 끝을 그 블록의 지역 변수가 쥐고 있으면 교착이고(10장: 한 번도 읽지 않는 변수는 함수가 돌아올 때까지 산다), 보고 뒤에 나머지를 건너뛴다.

```plain
$ fung test dead_test.fung
error[E0702]: deadlock: all tasks are waiting
  --> dead_test.fung:3:7
  |
3 |   recv(rx)
  |       ^
  |
 call path (innermost first):
   dead_test.fung:3 test: waits forever
 help: this task waits to receive, but it still holds that channel's sending end itself (a variable of `test: waits forever`), so the channel cannot close: close it with `close` first, or leave the sending end to the producers only
FAIL dead_test.fung: waits forever
  error[E0702]: deadlock: all tasks are waiting
    --> dead_test.fung:3:7
    |
  3 |   recv(rx)
    |       ^
    |
   call path (innermost first):
     dead_test.fung:3 test: waits forever
  a deadlock was reported; the rest of the file was skipped
0 passed, 1 failed
```

### 신호 {#signals}

`fung test`가 도는 중에 첫 SIGINT·SIGTERM이 오면 그것은 지금 도는 파일에 대한 정지 요청이다(14장). 그 파일이 끝나면 다음 파일은 돌지 않고, 명령은 128 + 신호 번호로 끝난다. 끊긴 블록에는 `SIGINT stopped the run; the rest of the file was skipped` 같은 메모가 붙는다.

## 언어 자체의 테스트 {#golden}

fung 저장소는 이 책의 장치들을 어떻게 시험하나. 언어 기능의 시험은 fung 파일로 쓴다(설계 09). `tests/lang/*.fung` 파일에 `# expect:` 주석으로 기대 출력을 적어 두면, `crates/fung-cli/tests/golden.rs`가 실제 실행 결과와 비교한다(골든 테스트). `dump`·실행·도구의 기대 출력은 `crates/*/tests/expected/` 아래 파일로 얼려 두고 시험이 사례 수까지 확인한다. 이 매뉴얼의 예제도 같은 방식이다. 페이지를 만들 때 모든 `run` 예제를 두 번 실행해 출력이 같은지 보고, `dump` 출력은 실제 명령의 결과다.

## 코드에서 찾아보기 {#source}

- `crates/fung-cli/src/test.rs`: 명령 `run_test`, 파일 하나 `test_file`, 테스트가 있는지 보는 `has_tests`, 요약 `Tally`.
- `crates/fung-cli/src/paths.rs`: 파일 고르기 `test_files`, `fung_files`.
- `crates/fung-front/src/compiler/m6.rs`: `assert`의 비교 쪼개기(`top_comparison`), `test` 블록 컴파일.
- `crates/fung-front/src/loader.rs`: `Loader::tests`(진입 파일의 블록만 남긴다).
- `crates/fung-vm/src/vm/m6.rs`: `ASSERT`·`TEST` 실행 `exec_m6`, 등록된 블록 `TestCase`, `Runtime::tests`, `Runtime::call_test`.
- `crates/fung-cli/tests/golden.rs`, `tests/lang/`: 언어의 골든 테스트.

:::exercise slugify 고치기
위 `slug.fung`의 두 테스트가 실패한다. `slugify`를 고쳐 세 테스트가 모두 통과하게 하자. 고친 파일은 `fung run`으로도 돌려 보자. `test` 블록은 무시되고 최상위만 돈다.

:::solution
```fung run
fn slugify(s) = s |> trim |> lower |> replace(" ", "-") |> replace(".", "-")

test "spaces become hyphens":
  assert slugify("Hello World") == "hello-world"
;;

test "keeps digits":
  assert slugify("Release 0.4") == "release-0-4"
;;

test "trims":
  assert slugify(" a ") == "a"
;;

print(slugify(" Release 0.4 "))
```

```plain
$ fung test fixed.fung
release-0-4
ok   fixed.fung: spaces become hyphens
ok   fixed.fung: keeps digits
ok   fixed.fung: trims
3 passed, 0 failed
```

`fung test`도 최상위를 먼저 실행하므로 `print`의 출력이 맨 앞에 나온다. 공백을 바꾸기 전에 `trim`해야 앞뒤 공백이 `-`가 되지 않는다.
:::
:::

## 정리 {#summary}

- `test` 블록은 같은 파일의 최상위에 둔다. `fung run`은 블록을 컴파일하지 않고, `fung test`는 블록을 인자 없는 함수로 컴파일해 그 자리의 `TEST` 명령으로 등록한다. 블록은 만들어질 때 보이는 파일 변수를 복사한다.
- `assert`는 키워드다. 맨 바깥 비교는 컴파일러가 쪼개어 `ASSERT`가 양쪽 값을 보여 준다. 더 깊이 쪼개지 않는 것은 부수효과 있는 식을 두 번 평가하지 않으려는 것이다.
- 실행기는 파일마다 새 런타임에서 최상위를 먼저 돌리고, 블록을 차례로 각자의 실행으로 돌린다. 테스트가 없는 파일은 실행하지 않는다.
- `exit`나 교착 뒤에는 그 파일의 나머지를 건너뛰되 그것을 실패로 센다. 조용히 끝나는 테스트 실행은 없다.

레퍼런스: [테스트와 디버깅](../reference/language.html#testing), [명령줄 도구](../reference/cli.html#test). 배우기: [14장 테스트](../learn/14-testing.html). 이것으로 책을 마친다. 전체를 한눈에 보는 요약은 부록 [내부 구조](internals.html)에 있다.
