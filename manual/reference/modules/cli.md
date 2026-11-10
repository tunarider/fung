---
lede: <code>import cli</code>. 명령줄 인자를 옵션(<code>--name value</code>)과 나머지로 나눈다. 일부러 작게 만들었다. 값의 검사와 변환은 보통의 fung 코드로 한다. fung으로 썼다.
description: fung 모듈 cli — 명령줄 인자를 옵션과 나머지로 나누는 cli.parse.
---
# cli — 명령줄 옵션

## 명령줄 인자와 옵션 {#concepts}

터미널에서 `fung deploy.fung --env prod --dry-run web-1 web-2`처럼 스크립트를 실행하면, 스크립트 이름 뒤의 단어들이 **명령줄 인자**(command-line arguments)다. fung은 이것을 prelude 값 [`args`](../prelude/program.html#fn-args)에 문자열 리스트로 넣어 준다: `["--env", "prod", "--dry-run", "web-1", "web-2"]`. 셸이 따옴표를 풀고 공백에서 나눈 뒤의 모습이다.

인자 중 `--`로 시작하는 것은 관례상 **옵션**(option, 플래그)이다. 옵션은 값을 하나 받거나(`--env prod`, `--env=prod`), 켜고 끄는 스위치일 수 있다(`--dry-run`). 옵션이 아닌 인자(`web-1 web-2`)는 보통 작업 대상이다. `cli.parse`는 이 관례대로 `args`를 옵션 맵과 나머지 리스트로 나눈다.

<table>
  <thead><tr><th>인자</th><th>결과</th></tr></thead>
  <tbody>
    <tr><td><code>--name value</code></td><td><code>flags.name = "value"</code>(다음 인자가 <code>--</code>로 시작하지 않을 때)</td></tr>
    <tr><td><code>--name=value</code></td><td><code>flags.name = "value"</code>(값에 <code>=</code>가 더 있어도 첫 <code>=</code>에서만 나눈다). <code>bools</code>에 있는 이름이면 값은 <code>true</code>·<code>false</code>만 받아 불리언이 된다</td></tr>
    <tr><td><code>--name</code></td><td><code>flags.name = true</code>(다음 인자가 없거나 <code>--</code>로 시작할 때, 또는 <code>bools</code>에 있는 이름일 때)</td></tr>
    <tr><td><code>--</code></td><td>그 뒤는 <code>--</code>로 시작해도 모두 <code>rest</code></td></tr>
    <tr><td>그 밖(<code>file.txt</code>, <code>-v</code>, <code>-</code>)</td><td>순서대로 <code>rest</code></td></tr>
  </tbody>
</table>

옵션 이름의 `-`는 `_`로 바뀐다(`--dry-run` → `flags.dry_run`). `flags.dry_run`처럼 점으로 바로 읽을 수 있게 하려는 것이다. 같은 옵션이 두 번 나오면(`--a-b`와 `--a_b`처럼 쓴 꼴만 달라도) 실패다. 어느 값을 뜻했는지 말없이 고르지 않는다. 한 글자 옵션(`-v`)과 묶음(`-xvf`)은 따로 해석하지 않고 `rest`에 둔다.

명령줄은 스크립트 바깥에서 오는 입력이라, 사용자가 잘못 친 명령줄은 런타임 에러가 아니라 `Err` 값이다. 그래서 `cli.parse`는 `Ok({flags, rest})`나 `Err({kind: 'invalid, message})`를 돌려준다. 잘못된 명령줄에 그냥 멈추려면 `cli.parse(args)!`, 사용자에게 안내를 보이려면 `match`로 `Err`를 받는다.

:::box choice 왜 이렇게 작은가

`cli`는 도움말 생성, 필수 옵션 검사, 타입 변환을 하지 않는다. 그런 기능은 규칙이 많아 배울 것이 늘고, 스크립트마다 원하는 모양이 다르다. 대신 결과가 평범한 맵과 리스트이므로 `get(flags, "port", "8080")`, `int(...)`, [`parse_dur`](../prelude/convert.html#fn-parse_dur), `match`로 필요한 만큼 직접 쓴다.

:::

:::fn cli.parse(argv, bools = []) fung

명령줄 인자를 옵션 맵과 나머지 리스트로 나눈다.

#### 설명

`argv`를 앞에서부터 읽으며 [위의 표](#concepts)대로 나눈다. 값은 언제나 **문자열**(값 없는 옵션은 `true`)이므로 숫자나 기간이 필요하면 직접 바꾼다. `bools`에 적은 이름(앞의 `--` 없이)은 값을 받지 않는 스위치로 다룬다. 명령줄에 쓴 꼴(`"dry-run"`)로도 결과의 키 꼴(`"dry_run"`)로도 적을 수 있다. `bools`가 없으면 `--dry-run web-1`의 `web-1`이 `dry_run`의 값이 되어 버린다. 스위치는 `--dry-run=false`처럼 `=true`·`=false`로 값을 줄 수 있고, 그 밖의 값은 실패다.

없는 옵션을 `flags.port`로 읽으면 E0508(없는 키)이다. 선택 옵션은 [`get`](../prelude/maps.html#fn-get)의 기본값으로 읽는다(`get(opts.flags, "port", "8080")`). `opts.flags.port ?? "8080"`은 `??`에 닿기 전에 E0508로 멈춘다.

#### 매개변수

- `argv`: 문자열 리스트. 보통 [`args`](../prelude/program.html#fn-args)를 그대로 넘긴다.
- `bools`: 값을 받지 않는 옵션 이름의 리스트.

#### 돌려주는 값

`Ok({flags, rest})`. `flags`는 이름(문자열 키) → 문자열 또는 불리언, `rest`는 옵션이 아닌 인자의 리스트(순서 유지).

명령줄이 잘못되었으면 `Err({kind: 'invalid, message})`. `message`는 사용자에게 그대로 보여 줄 수 있는 문장이다.

- 이름이 없는 옵션(`--=x`): ``the flag `--=x` has no name``
- 같은 옵션이 두 번: ``the flag `--x` is given twice``
- 스위치에 `true`·`false`가 아닌 값(`--dry-run=no`): ``the switch `--dry-run` takes true or false, got `no` ``

#### 에러

- `argv`나 `bools`가 문자열 리스트가 아니면 런타임 에러 E0403. 에러는 그 인자의 이름을 말한다. 이것은 스크립트의 잘못이라 `Err`가 아니다.

#### 예제

**스크립트의 인자 나누기**

```fung run args="--env prod --dry-run web-1 web-2"
import cli

opts = cli.parse(args, bools: ["dry-run"])!
print(opts)
print(opts.flags.env, opts.rest)
```

**`bools`가 없으면**

```fung run
import cli

print(cli.parse(["--dry-run", "web-1"])!)
print(cli.parse(["--dry-run", "web-1"], bools: ["dry-run"])!)
```

**기본값과 변환**

```fung run args="--retries 5 --timeout 2s report.csv"
import cli

opts = cli.parse(args)!
retries = int(get(opts.flags, "retries", "3"))
wait = parse_dur(get(opts.flags, "timeout", "30s"))!
verbose = get(opts.flags, "verbose", false)
print(retries + 1, wait, verbose, opts.rest)
```

**`=`와 `--`**

```fung run
import cli

print(cli.parse(["--label=env=prod", "--", "--not-a-flag", "-v"])!)
```

**잘못된 명령줄 알리기**

```fung run args="--env prod --env dev"
import cli

match cli.parse(args):
  Ok(opts) => print(opts.flags.env)
  Err(e)   => print("usage error:", e.message)
;;
```

#### 같이 보기

[`args`](../prelude/program.html#fn-args), [`get`](../prelude/maps.html#fn-get), [`parse_int`](../prelude/convert.html#fn-parse_int), [`exit`](../prelude/program.html#fn-exit), [명령줄 옵션과 도움말](../../recipes/cli-options.html)

:::

배우는 순서로는 [11장 파일·외부 명령·CLI](../../learn/11-files-and-commands.html)를 본다. 스크립트를 실행하는 `fung` 명령 자체의 옵션은 [명령줄 도구](../cli.html)에 있다.
