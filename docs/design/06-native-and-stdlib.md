# 06. 네이티브 연동 · 표준 라이브러리

## 내장 함수 연결 방식

- 내장 함수는 **손으로 감싼다.** fung 값 검사 → Rust 호출 → fung 값으로 변환.
- 인자 검사 등 반복 작업은 도우미 함수로 줄인다.
- 리플렉션 자동 연결은 쓰지 않는다 (구현 언어의 이름·타입·에러 방식이 새어 나오므로).
- **내장 함수도 fung 함수와 같은 호출 규약을 따른다.** 매개변수 목록(이름, 기본값, `...rest`)을 선언하고,
  위치·이름 인자·기본값·개수 검사는 VM이 한 곳에서 한다. `exec(cmd, cwd: d)`가 되는 이유이며,
  "내장 함수는 이름 인자가 안 된다" 같은 예외 조항을 두지 않는다 (배경 원칙 7).
  값에 따라 달라지는 기본값(예: 현재 디렉터리)은 `nil`로 받아 Rust 쪽에서 채운다.
- `exit(n)`은 프로세스 종료(`std::process::exit`)를 직접 부르지 않는다. VM 안에서 **종료 신호**로 올라가고(에러와 같은 경로, 호출 경로·메시지 없음),
  되감기가 `with`의 `release`를 모두 지난 뒤 `fung` 명령이 받아 그 코드로 끝낸다. 같은 프로세스 안의 테스트·REPL이 깨지지 않기 위해서다.

## 네 층: core · native · extras

| 층 | 뜻 | 언어 | 예 |
|------|------|------|------|
| **core** | 언어의 의미와 런타임. 없으면 다른 언어가 된다. "언어가 보장하는 값 연산"을 포함한다 | Rust | 렉서~VM, 값·에러, 태스크·채널, `exit`·`at_exit`, `print`, `map`·`filter`·`fold`·`sort`, `split`·`join`, `int`·`str` |
| **native** (`_native.*`) | 바깥세상과의 다리. fung으로 **쓸 수 없는** 것, 또는 쓸 수는 있지만 **안쪽 루프라 성능이 나오지 않는** 것. 숨김, 최소 | Rust | `_native.fs`, `_native.path`, `_native.process`, `_native.conc`, `_native.json`, `_native.regex`, `_native.time`, `_native.net`, `_native.range`(prelude의 범위 함수가 `len` 없이 끝을 알게) |
| **extras (내장)** | core·native 위에 **fung으로** 쓴 것. 없어도 언어는 같지만 불편하다 | fung | `fs`·`path`·`cli`·`conc`·`http` 모듈, `with_open`, `take`·`drop`·`lines`, `until` |
| **extras (프로젝트)** | 사용자 패키지. 같은 메커니즘(맨 경로 import), 의존성 관리는 추후 | fung | — |

- 기준: core는 "최소한 언어가 이 정도는 보장한다"; extras는 "core만으로 만들 수 있고(성능도 충분) 없으면 너무 불편해서 넣어 둔
  것"이며 **가급적 Rust를 쓰지 않는다**. native는 그 둘 사이에서 Rust가 꼭 필요한 좁은 다리다.
- **Rust native를 쓰는 때** (2026-10-14 확정): 가급적 쓰지 않되, **성능이 이유면 extras에서도** `_native.*`에 원시를 두고 fung은 얇게
  감쌀 수 있다. 글자 단위로 도는 파서(`json`)는 `split`·`join`을 core에 둔 것과 같은 이유로 Rust에 둔다. 경계는 측정으로 옮긴다.
- 우선순위(이름 해석): core > 내장 extras > 프로젝트 extras.
- 의존 방향: **core는 extras를 import하지 않는다.** fung으로 쓴 extras는 core의 prelude와 `_native.*` 다리만 쓴다. 내장 파일은
  `crates/fung-front/src/extras.rs`가 이름 → fung 소스로 담고(prelude 파일 목록 `PRELUDE_FILES`), `_native.*` 다리 표는
  `fung_vm::boot::NATIVE_MODULES` 하나다. 크레이트 의존은 `fung-cli` → `fung-vm` → `fung-front` 한 방향이고 앞단은 외부 크레이트가 0개다
  (`crates/fung-cli/tests/deps.rs`가 확인한다).
- 원소를 도는 함수(`map` `filter` `fold` `each` `sort` `find`…)와 문자열·변환 원시는 안쪽 루프라 core(Rust)에 둔다. 그 위의 조합
  (`take` `drop` `first` `sum` `min` `lines` `pad_*` `is_ok`…)은 fung extras다. 경계는 측정해서 옮길 수 있고 의미는 바뀌지 않는다(골든).
  측정으로 돌아온 예: `flatten`은 fung에서 리스트를 누적하면(`acc + x`) 제곱 시간이라 core에 남았다(M7.5 노트).
- fung extras의 인자 검사는 `fail`(E0803)로 한다. E0403 같은 호출 규약 에러는 Rust 원시만 낸다.
- 내장 extras 안에서 난 런타임 에러의 위치는 **사용자 코드의 가장 가까운 프레임**이다(사용자는 내장 파일을 열 수 없다). 호출 경로에는
  내장 프레임이 그대로 보인다. 로더가 내장 파일의 `Proto`에 `Builtin`을 표시하고 VM이 그것을 본다.
- fung으로 쓴 extras는 실행 파일에 내장한다 (`include_str!`, `extras/*.fung`). 배포는 여전히 `fung` 하나.
- Rust 내장 함수가 사용자 람다를 다시 호출할 수 있어야 한다 (태스크별 VM 상태 분리로 가능).
- fung으로 쓴 모듈은 **import될 때** 컴파일한다 (prelude만 시작 시). 스크립트 시작 시간을 지키기 위해서다.

## 사용자 네이티브 확장

- 지금은 열지 않는다. 필요한 네이티브 기능은 fung 본체에 추가한다.
- `_native.*`도 사용자 코드(프로젝트 extras)에 열지 않는다 (2026-10-14 확정). 공개 모듈이 원시를 **같은 네이티브 함수 값으로** 재내보내므로
  (`read = _n.read`) 사용자 라이브러리가 `import fs`로 쓰는 것과 비용이 같다. 숨겨진 것은 정리를 강제하려고 감싼 `temp_dir`과
  HTTP 서버의 내부 도우미뿐이다. 열면 원시의 이름·인자가 공개 API로 굳는다.
- 외부 기능은 `exec` / `sh`로 다른 프로그램을 호출한다.
- 네이티브 플러그인(.so)은 쓰지 않는다 (Rust에는 안정된 ABI가 없어 같은 컴파일러 버전이 필요하고, 플랫폼마다 불안정).

## 에러 규칙

- **외부 요인으로 실패할 수 있는 것** (파일, 네트워크, 파싱) → `Ok` / `Err` 반환.
- **코드가 틀린 것** (타입 오류, 범위 밖 인덱스, 0으로 나누기) → 런타임 에러로 중단.
- 변환 함수 `int`·`float`·`str`·`sym`은 잘못된 입력에 **런타임 에러** (코드 오류 취급). 그래서 `int("5") + 3`이 그대로 된다.
  사용자 입력처럼 믿을 수 없는 문자열은 `parse_int(s)`·`parse_float(s)`가 `Ok`/`Err`로 돌려준다.
  문자열의 문법은 리터럴과 같다(01, 앞의 부호 하나 허용): 정수는 10진, 실수는 소수점 양쪽 숫자·지수·숫자 사이 `_`이고 정수 꼴(`"42"`)도 받는다.
  `.5`·`1.`·16진 실수·`inf`·`nan`은 거부한다.
  "이 문자열은 숫자여야 한다"(단언)와 "숫자인지 확인한다"(검사)를 이름으로 구분한다.
- 사용자 코드가 "여기 오면 버그다"를 말할 때는 `fail(msg)`: 메시지와 함께 런타임 에러를 일으킨다.
  `x ?? fail("config.port is required")`처럼 `??`와 짝이 맞는다. (`panic`이라는 이름은 쓰지 않는다.)
- **닫힌 표준 출력·에러** (2026-10-18 정리): 표준 출력이나 에러에 쓰는데 읽는 쪽이 없으면(`fung run x | head -1`) 프로세스는 SIGPIPE(신호 13)로 끝난다.
  보고·종료 처리기 없이 어느 태스크에서든 곧바로 끝나고, 부모가 SIGPIPE를 무시로 둔 채 띄웠어도 같다(파이프라인의 한 단계는 파이프라인이 끝나면 함께 끝나야 한다).
  신호로 끝나지 못하면 셸이 보여 줄 값 141로 끝난다. 다른 파일·자식 표준 입력·소켓에 쓰다 읽는 쪽이 없으면 에러 값이다.
- 표준 라이브러리 `Err`의 내용은 맵으로 통일한다: `kind`(심볼), `message`(사람이 읽는 문장), 그 외 상황별 필드.
- `kind` 어휘 (처음 목록, 알파에서 추가):
  `'not_found` `'exists` `'permission` `'io` `'parse` `'invalid` `'timeout` `'closed` `'cancelled` `'error` `'exit` `'network`

```
match fs.read(p):
  Ok(text)                => ...
  Err({kind: 'not_found}) => ...
  Err(e)                  => print(e.message)
;;
```

## prelude (1차안, 알파 버전에서 써보며 조정)

- 입출력: `print`, `eprint`, `read_line`, `read_lines`, `dbg`
  - `print(a, b, …)`: 공백으로 이어서 한 줄에. 문자열은 따옴표 없이, 나머지는 리터럴 형태 (08).
  - `read_line(f = nil)`·`read_lines(f = nil)`: `f`가 `nil`이면 표준 입력(`fung -e`에서도 같다), 파일 핸들이면 그 파일.
    `read_line`은 `Ok(줄)`, 끝이면 `Ok(nil)`, 닫힌 핸들이면 `Err({kind: 'closed})`. `read_lines`는 남은 줄 전부를 `Ok(리스트)`로.
    표준 입력은 필요할 때만, 한 번에 최대 64 KiB(Linux 파이프 버퍼 크기)를 읽는다. 그래서 같은 입력을 이어 읽는 다른 프로세스
    (`{ fung a.fung; cat; } < file`의 `cat`)는 fung이 읽어 간 버퍼 뒤부터 받는다.
- 타입·변환: `type_of`, `int`, `float`, `str`, `sym`, `parse_int`, `parse_float`, `parse_dur`
  - `parse_dur(s)`: 기간 리터럴 문법(`1h30m`, `500ms`, `-5s`, 앞뒤 공백 허용)을 `Ok(dur)`/`Err 'parse`로 (2026-10-14). `--timeout 30s` 같은 옵션용.
- 컬렉션: `len`, `is_empty`, `map`, `filter`, `fold`, `each`, `sum`, `min`, `max`, `sort`, `sort_by`, `reverse`, `range`, `zip`, `first`, `last`, `take`, `drop`, `find`, `any`, `all`, `contains`, `unique`, `flatten`, `group_by`, `keys`, `values`, `pairs`, `get`, `put`, `merge`, `remove`, `chars`
  - `each(xs, f)`: 부수효과용 순회, `nil` 반환 (`for`는 리스트를 만든다).
  - `chars(s)`, `pairs(m)`, `keys(m)`, `values(m)`: 문자열과 맵을 순회 가능한 리스트로 (02).
- 문자열: `split`, `join`, `trim`, `upper`, `lower`, `replace`, `starts_with`, `ends_with`, `pad_left`, `pad_right`, `repeat`, `lines`
  - `upper`·`lower`: 유니코드 기본 대소문자 변환(전체 매핑). 길이가 바뀔 수 있고(`ß` → `SS`) 단어 끝 시그마는 `ς`.
  - `lines(s)`: 텍스트를 줄 리스트로 — `read_lines`와 같은 규칙(`\n` 구분, 줄 끝 `\r` 제거, 끝의 줄바꿈은 빈 줄을 만들지 않음).
    `split(s, "\n")`은 끝에 빈 조각을 남기므로 줄 나누기에는 `lines`를 쓴다.
- 결과값: `is_ok`, `is_err`
- 반복 패턴 (fung 코드, 00의 "상태가 있는 반복은 재귀" 보완): `until(x, f, pred)`, `iterate(x, f, n)` (`retry`는 `conc` 모듈)
- 에러·정리: `fail`, `with`(05·03)
- 동시성: `scope`, `supervise`, `spawn`, `await`, `cancel`, `channel`, `send`, `recv`, `close`, `atom`, `deref`, `swap`, `reset`, `sleep` (05). `fan_in`·`pmap`·`retry`는 `conc` 모듈
- 프로세스: `exec`, `sh`, `env`, `args`, `exit`
  - `args`: 진입 스크립트 뒤의 인자만 담은 **문자열 리스트** (prelude 값).
  - `env(name)`: 문자열 또는 `nil`. `env()`: 전체 맵.
  - `exit(code = 0)`.

`contains`는 리스트·문자열·맵 모두에 동작한다.

## 모듈 (1차안, 알파 버전에서 써보며 조정)

| 모듈 | 내용 |
|------|------|
| `fs` | read, read_lines, write, append, list(`dir, full = false` — `full`이면 `dir`을 붙인 경로), glob, walk, exists, stat, mkdir, remove, remove_all, move, copy, open, close, write_line, with_open, with_temp_dir |
| `path` | join(이어 붙이고 정리, 뒤의 절대 경로가 앞을 지우지 않음), base, dir, ext(점으로 시작하는 이름은 확장자 없음), stem, abs |
| `cli` | `parse(argv, bools = [])` → `{flags, rest}`. `--name value`·`--name=value`·`--name`(true, `bools`에 있으면 값을 먹지 않음), `--` 뒤는 전부 rest, 이름의 `-`는 `_`로. 검사·변환은 보통 코드로 (fung으로 작성) |
| `json` | `parse(s)` → `Ok(값)` / `Err({kind: 'parse, line, col, message})`, `stringify(v, indent = nil)`. 아래 "JSON" 참고 (`_native.json`, M8) |
| `http` | `get(url, headers = {}, timeout = 30s)`, `post(url, body, headers = {}, timeout = 30s)`, `request(method, url, body = nil, headers = {}, timeout = 30s)`, `serve(addr, handler, ready = nil)`. 아래 "HTTP" 참고 (`_native.net` 위의 fung, `extras/http.fung`) |
| `conc` | `fan_in(s, rxs)`, `pmap(xs, f, limit = 0)`, `retry(f, times = 3, delay = 0ms)` (05, fung으로 작성) |
| `time` | `now()`, `parse(s, format = nil)` → `Ok`/`Err 'parse`, `format(t, format = nil)`, `utc(t)`, `local(t)`, `unix(t)`, `from_unix(n)`, `parts(t)`. `now()`는 `time` 타입 값 (02·07): `==`, `<`, `+ dur`, `time - time → dur`. 아래 "시간 형식" 참고 (`_native.time`, M8) |
| `regex` | `is_match(s, pat)` → bool, `find(s, pat)` → 매치 맵 또는 `nil`, `find_all(s, pat)`, `replace(s, pat, repl)`, `split(s, pat)`. 아래 "정규식" 참고 (`_native.regex`, M8) |
| `math`, `random` | (M8 이후) |

- 파일·명령·네트워크 함수의 `Err({kind, message})`에서 운영체제가 말한 이유는 소문자 짧은 문장이다(6c-2). 흔한 것(`no such file or directory`,
  `permission denied`, `operation not permitted`, `file exists`, `not a directory`, `is a directory`, `directory not empty`, `invalid argument`,
  `no space left on device`, `broken pipe`, `connection refused` 등)은 fung이 정해서 어느 플랫폼에서나 같고, 드문 것은 운영체제의 문구를 소문자로 시작해 쓴다.
- `fs`의 규칙(6c-2): `mkdir`는 위의 없는 디렉터리까지 만들고, 이미 디렉터리가 있으면 성공, 다른 것이 있으면 `'exists`.
  `mkdir("")`은 `'not_found`. `remove`는 파일·링크·빈 디렉터리 하나. `remove_all`은 없는 경로와 `""`에 성공하고, 루트(`/`)와 마지막 성분이
  `.`·`..`인 경로는 거절한다(쓴 것보다 많이 지우게 되므로). 링크는 `link/`처럼 써도 링크만 지운다.
  `move`는 이미 있는 디렉터리를 덮지 않는다(같은 디렉터리의 대소문자만 바꾸는 이름 바꾸기는 된다). `list`·`walk`는 이름의 바이트 순서이고,
  `walk`가 읽지 못한 디렉터리는 그 경로로 에러가 난다.

추후 후보: `csv`, `hash`, `base64`, `gui`

## JSON

- `parse`: object → 맵(**키 순서 유지**), array → 리스트, 소수점·지수 없는 숫자 → int(int64를 넘으면 `Err 'parse`), 그 외 숫자 → float,
  `null` → `nil`. 입력 뒤에 쓰레기가 남아도 `Err`. `line`·`col`은 1부터(열은 글자 단위, BOM도 한 글자).
  RFC 8259 그대로 읽고(6c-3), 엄격하다: 같은 키가 두 번 나오면(``duplicate key "a"``), 짝 없는 서로게이트(`\ud800`)도 `Err`. 공백은 스페이스·탭·LF·CR뿐.
  깊이는 10,000까지. 에러 위치는 문제가 있는 그 글자이고, 문구는 `unexpected character ']'`, `unexpected end of input`,
  `unexpected text after the value`, ``expected `,` or `]` ``, `invalid escape`, `control character in a string`, `invalid number`,
  `number out of range`, `nesting too deep (limit 10000)` 등.
- `stringify`: 맵·리스트·문자열·int·float·bool·nil은 그대로, 튜플 → array, 심볼 → string, time → ISO 8601 string. 기간·함수·태그·범위
  등은 **런타임 에러** E0513(`cannot convert dur to JSON`, 코드 오류). `indent`는 `nil`(한 줄) 또는 들여쓰기 칸 수. 문자열은 RFC 8259에 필요한 것만 이스케이프한다(`"`, `\`, U+0020 미만; 6c-3).
  결과가 1억 글자를 넘으면 E0806.

## 정규식

- 문법은 Rust `regex` 크레이트의 것(RE2 계열, 6c-4): 역참조·전후방 탐색 없음, 대신 항상 선형 시간. `\d`·`\w`·`\s`·`\b`는 유니코드를 알아본다
  (Python 3와 같음, ASCII만이면 `(?-u:\d)`). 그룹 이름은 ASCII 글자·숫자·`_`이고 겹치면 안 된다.
- 매치 맵 `{text, start, end, groups, named}`. `start`·`end`는 **글자(룬) 인덱스**(02의 문자열 규칙; `s[start..end]`가 맞는다).
  `groups`는 캡처 그룹 리스트(전체 매치 제외, 안 잡힌 그룹은 `nil`), `named`는 이름 있는 그룹 맵.
- `find_all`·`replace`는 앞 매치 바로 뒤의 빈 매치를 세지 않는다. `replace`의 `repl`은 `$1`·`${name}`·`$$`(`$1a`는 이름 `1a`, 그래서 `${1}a`).
  `split`은 `split(s, sep)`처럼 빈 조각을 남기고, 문자열 맨 앞·맨 끝의 빈 매치로는 자르지 않는다(`split("abc", "")` → `["a","b","c"]`).
- 잘못된 패턴은 런타임 에러 E0515(코드 오류): ``invalid pattern `a(`: unclosed group``. 중첩은 250단계까지, 컴파일한 패턴은 128 MiB까지
  (넘으면 `pattern too large`; `\w{400}`은 들어가고 `\pL{1000}{1000}`은 넘는다). 빈 패턴으로 빈 문자열을 나누면 `[]`(`split("", "")`과 같음). 컴파일한 패턴은 Rust 쪽에서 캐시한다. 별도 "컴파일된 regex 값" 타입은 없다.

## HTTP

**fung HTTP 약속** (2026-10-16). `_native.net`(소켓·TLS 원시 기능) 위의 fung 코드(`extras/http.fung`)가 지키는 동작이다.
약속 밖의 세부는 open-questions "Rust 구현: 정한 것과 남은 일"의 알려진 한계(HTTP, 소켓·TLS)에 있다. 학습 노트 `docs/notes/R4d-net-http.md`.

**클라이언트** (`get` `post` `request`)
- 응답을 받으면 상태 코드와 무관하게 `Ok({status, headers, body})` (404도 `Ok` — "서버가 답했다"와 "요청이 실패했다"는 다른 일).
  전송 실패 `Err({kind: 'network, message})`, 시간 초과 `Err({kind: 'timeout, message: "request to `<url>` timed out"})`.
  `body`는 문자열. 응답 `headers`의 키는 **소문자**, 같은 이름이 여럿이면 `", "`로 잇는다.
- HTTP/1.1(RFC 9110·9112). 요청에 `Host`, `User-Agent: fung/<버전>`(사용자 값이 덮고, 빈 값이면 보내지 않음), 본문이 있으면 `Content-Length`.
  사용자 헤더는 이름을 표준 대소문자로 바꾼 뒤 **맵 순서 그대로** 보낸다. 같은 이름이면 사용자 것이고, 비어 있지 않은 사용자 `Host`도 자동 값을 대신한다(다른 백엔드·가상 호스트 고르기; 리다이렉트가 다른 `host:port`로 가면 그 요청부터 버림).
  `Content-Length`·`Transfer-Encoding`·`Trailer`는 본문의 끝을 정하므로 언제나 클라이언트가 쓴다. URL에 사용자 정보(`user:pass@`)가 있고 `Authorization`이 없으면 `Authorization: Basic …`.
- 응답 본문은 `Content-Length`, `Transfer-Encoding: chunked`, 연결 닫힘까지 중 하나로 읽는다. `HEAD` 응답과 1xx·204·304는 본문 없음.
- **리다이렉트**(RFC 9110 15.4): 301·302·303 → `GET`(원래 `HEAD`면 `HEAD`), 본문 버림. 307·308 → 메서드·본문 유지.
  리다이렉트는 10번까지 따라가고 11번째는 `Err 'network`(`too many redirects (limit 10)`). 리다이렉트 응답의 본문은 읽지 않고 연결을 닫는다.
  다른 호스트(첫 요청의 호스트나 그 하위 도메인이 아닌 곳)로 넘어가면 `Authorization`·`Cookie` 같은 민감한 헤더를 버린다.
  `Location`은 현재 URL 기준으로 푼다(RFC 3986 5.2). 다음 요청에 `Referer`(사용자 정보 없이, https → http면 없음).
- **프록시**(대부분의 HTTP 클라이언트가 읽는 환경 변수): `HTTP_PROXY`/`http_proxy`(http용), `HTTPS_PROXY`/`https_proxy`(https용), `NO_PROXY`/`no_proxy`(이름·하위 도메인·IP·CIDR, 포트 선택).
  `localhost`·루프백 주소는 거치지 않는다. CGI 프로그램(`REQUEST_METHOD`가 있음)에서는 `HTTP_PROXY`를 쓰지 않는다(요청의 `Proxy` 헤더에서 올 수 있다, "httpoxy").
  http는 절대 URL 요청, https는 `CONNECT` 터널(같은 `User-Agent`) 뒤 TLS.
- **https**: 운영체제 루트 인증서(`SSL_CERT_FILE`·`SSL_CERT_DIR`이 있으면 그것), SNI, rustls의 검증(사슬 다음 이름). 인증서 오류는 `Err 'network`.
- **`timeout`**: 기간, 연결·TLS 핸드셰이크·`CONNECT`·리다이렉트를 포함한 요청 전체 하나(따로 짧은 한도가 없다). `0s`면 제한 없음.
  안에서 `timeout` 범위(05)로 구현하므로 실제 기한은 바깥 기한과의 이른 쪽이다: `timeout(1s, () => http.get(url))`은 1초에 바깥의 `Err({kind: 'timeout})`.
  `_native.net`의 `connect`·`start_tls`에는 기한 인자가 없다(범위의 기한이 이름 조회·연결·핸드셰이크를 끊는다).
- **에러 문구**: `cannot request `<url>`: <이유>`. 이유는 짧은 문구다: `connection refused`, `no such host`, `invalid URL: …`, ``unsupported scheme `ftp` ``,
  ``certificate is not valid for `host` ``, `unknown certificate authority`, `certificate has expired (valid until <시각>)`, `too many redirects (limit 10)`,
  ``invalid Location header `…`: …``, ``proxy `host:port`: …``(프록시에서 난 것), ``redirected to `url`: …``(리다이렉트 뒤의 요청에서 난 것).
- 압축을 요청하지 않는다: 본문은 언제나 서버가 보낸 그대로다.
- 취소되면 E0701.
- 응답 본문 상한 **64 MiB**(넘으면 `Err 'network`; 2026-10-16 사용자 결정, 더 큰 응답은 나중에 요청별 옵션으로).
  SOCKS 프록시와 https 프록시 너머의 https는 지원하지 않는다(`Err 'network`).

**서버** `serve(addr, handler, ready = nil, drain = 10s, reuse_port = false)`
- 바인드 실패 `Err 'network`, 되면 **막힌 채로** 요청을 받는다. 끝나는 길은 둘이다 (L6).
  - **정지 요청**(05 "정지 요청과 신호": 첫 SIGINT·SIGTERM, `request_stop()`): 리스너를 닫아 새 연결을 그만 받고, 처리 중인 요청을 `drain`까지 마친 뒤
    남은 것은 취소하고(503으로 답함) `nil`을 돌려준다. 정상 종료다.
  - **취소**(바깥 범위, `timeout`, `exit`): 기다리지 않고 곧바로 멈춘다.
- 안에서 범위를 연다. 연결마다 `supervise`의 태스크이고, 핸들러는 연결 안에서 다시 자기 태스크로 돈다. 그래서 `serve`가 돌아오면 남는 태스크가 없다.
- `reuse_port`는 바인드 전에 `SO_REUSEPORT`를 켠다. 같은 호스트에서 프로세스 여럿이 한 포트를 받을 때 쓴다(여러 코어를 쓰는 방법, 05). Linux는 연결을 나눠 주고, macOS는 받게만 한다.
  `ready`가 함수면 리스닝 직후 실제 주소(`"127.0.0.1:53121"`)로 한 번 부른다(`":0"`의 포트를 아는 길).
- 핸들러 `req => resp`. `req = {method, path, query, headers, body}`(`query`는 이름마다 첫 값, `+`는 공백, 잘못된 `%` 이스케이프가 든 쌍은 빠짐, `headers`는 소문자 키).
  `resp`는 맵 `{status = 200, headers = {}, body = ""}` 또는 문자열(200, `text/plain; charset=utf-8`). 응답 헤더에 `Date`(IMF-fixdate, RFC 9110 5.6.7)와
  `Content-Length`가 붙고, 본문이 있는데 `content-type`이 없으면 `text/plain; charset=utf-8`.
- 핸들러가 런타임 에러를 내면 500을 답하고 stderr에 한 줄(`http: handler failed: …`), 다른 타입을 돌려주면 500과 `http: bad handler response: …`를 찍고 서버는 계속 돈다(종료 코드 무관).
  핸들러 안의 `exit`, 취소, `drain`을 넘긴 정지는 503이며 보고하지 않는다.
- 서버가 스스로 내는 답(형식 오류 400, 한도 413·431, 501, 505, 핸들러 실패 500, 503)은 표준 상태 줄과 본문 `<코드> <이유 문구>\n`이다.
  이유 문구는 IANA HTTP 상태 코드 레지스트리의 것이고(418은 "(Unused)"라 빈 문구), 표에 없는 코드(`599`)는 빈 이유 문구(`HTTP/1.1 599 `)로 보낸다.
- 요청마다 **새 fung 태스크**에서 핸들러가 돈다(동시에 돌 수 있다; 공유 상태는 `atom`). 라우팅은 없고 `match req.path:`로 쓴다.
  예: `examples/api.fung`·`serve.fung`.
- 한도 (2026-10-14, 고정값; 2026-10-18 머리 한도 정리): 헤더를 10초 안에 보내지 않는 연결은 끊고, 요청 머리는 1 MiB(줄과 CRLF, 빈 줄 포함)·헤더 줄 500개까지(넘으면 431),
  요청 본문은 16 MiB까지(넘으면 413, 핸들러는 불리지 않음), 형식 오류는 400.
  조각 전송 요청 본문도 받는다.
- 런타임 연동: 서버 루프는 fung 코드다(범위, `stop_signal`, `with`). 내부 함수는 연결 태스크를 띄우지 못했을 때(스레드가 바닥남) 서버가 죽지 않게 하는
  `_native.conc.try_spawn` 하나다. R7에서 `spawn`이 실패하지 않게 되면 지운다.
- 연결마다 요청 하나라서 HTTP/1.1 응답에 늘 `Connection: close`가 붙는다. 응답 본문은 언제나 `Content-Length`로 보낸다.

**약속 밖**(지원하지 않는 것): 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축, 바이너리 본문(UTF-8이 아니면 U+FFFD).

## 파일 핸들

- `fs.open(path, mode = 'read)` → `Ok(file)` / `Err`. `mode`는 `'read` `'write` `'append`. `fs.close(f)` → `Ok(nil)`.
- 새로 만드는 파일(`fs.write`·`fs.append`·`fs.open`의 `'write`·`'append`)의 권한은 0666에서 umask를 뺀 것이다(셸의 `>`·`>>`와 같음; 2026-10-18).
- `fs.with_open(path, f, mode = 'read)`: 열고 `f(핸들)`를 부르고 **결과가 무엇이든** 닫은 뒤 그 결과를 돌려준다 (fung 코드, 03).
  람다 안의 `?`는 람다에서 `Err`를 돌려주는 것이므로 그대로 닫히고 전달된다.
- `read_line(f)` `read_lines(f)`(prelude), `fs.write_line(f, s)` → `Ok(nil)` / `Err`.
- 핸들의 읽기·쓰기 버퍼는 64 KiB다. 쓴 줄은 버퍼가 차거나 `close`할 때 파일에 닿고, `close` 없이 마지막 사본이 사라져도 비운다(그때의 쓰기 에러는 보고되지 않는다).
- 닫힌 핸들에 읽고 쓰면 `Err({kind: 'closed})`. 핸들을 `with_open` 밖으로 돌려줘도 막지 않는다 — 쓰는 순간 `'closed`다.
- 핸들은 태스크 사이에 공유할 수 있다 (2026-10-14): 연산(`read_line`·`read_lines`·`write_line`·`close`) 하나하나가 핸들의 뮤텍스로
  직렬화되어 줄이 찢어지거나 섞이지 않는다. 어느 태스크가 어느 줄을 받는지는 정해지지 않는다("값은 공유, 연산은 원자적").

## 시간 형식

- 토큰 방식: `"YYYY-MM-DD HH:mm:ss"`
- 형식을 지정하지 않으면 ISO 8601 (`2026-10-12T22:45:00+09:00`). `print`·`str`도 같은 형식(소수 초는 0이 아닐 때만).
  `parse(s)`(형식 없음)는 RFC 3339를 엄격하게 읽는다(6c-3): `YYYY-MM-DDThh:mm:ss`, 소수는 `.`와 1~9자리, 끝은 `Z`나 `±hh:mm`(초가 있으면 `±hh:mm:ss`),
  `T`·`Z`는 대소문자 무시, 공백 구분과 한 자리 필드는 받지 않는다. 에러는 ``cannot parse `x` as a time: <이유>``(`month out of range`,
  `expected 2 digits for the hour`, `expected a month name like Oct` 등).
- 기본 시간대는 컴퓨터의 로컬 시간대. 값은 시간대를 보존한다(`utc(t)`로 바꾼 값은 UTC로 찍힌다).
- 로컬 시간대는 C 라이브러리처럼 찾는다(6c-3): `TZ`가 없으면 `/etc/localtime`, `""`·`UTC`면 UTC, 앞의 `:`는 뗀다. 절대 경로는 그 파일, 상대 이름은
  `$TZDIR`(없으면 `/usr/share/zoneinfo`) 아래에서 찾고, 파일이 없으면 POSIX 시간대 문자열(`KST-9`, `EST5EDT,M3.2.0,M11.1.0`)로 읽는다. 다 실패하면 UTC.
- 토큰 (2026-10-14 확정): `YYYY` `YY`(파싱은 2000~2099) `MM` `MMM`(영문 월 약자 `Oct`) `DD` `ddd`(영문 요일 약자 `Sun`) `HH`(24시간) `mm` `ss`
  `SSS`(밀리초) `Z`(`+09:00`) `ZZ`(`+0900`). 파싱에서 영문 이름은 대소문자를 가리지 않고, `ddd`는 이름만 검사하며 날짜와 대조하지 않는다.
  목록에 없는 글자는 그대로 출력·그대로 기대한다. 긴 토큰부터 맞춘다. `parse`는 같은 토큰으로 자리수를 정확히 맞추고, 시간대 토큰이
  없으면 로컬로 해석한다. 12시간제·이스케이프는 보류.
- `parts(t)` → `{year, month, day, hour, minute, second, weekday}`, `weekday`는 `'mon`…`'sun`.
- 범위와 오프셋(6c-3): 시각은 UTC로 0000~9999년. `from_unix`가 범위 밖이면 E0403, 산술이 넘으면 E0503. 오프셋에 초가 있으면(1883년 전 뉴욕의 `-04:56:02` 같은
  지방 평균시) ISO 출력과 `Z`·`ZZ`가 초까지 쓴다. 숫자 오프셋으로 읽은 값은 쓴 그대로의 고정 오프셋이다.
- 지역 시간으로 읽은 벽시계 시각이 서머타임 경계에 걸리면(6c-3): 없는 시각(시계를 앞으로 돌린 빈틈)은 빈틈만큼 뒤로 밀리고(`02:30` → `03:30`),
  두 번 있는 시각(되돌린 구간)은 이른 순간이다.

## 명령 실행

```
exec(["mv", old, new])          # 인자 리스트. 쉘을 거치지 않음. 공백·특수문자 안전
exec(cmd, cwd: dir, env: {...}, stdin: s)   # 옵션은 이름 붙인 인자
sh("ls -la | grep .log")        # 쉘 기능이 필요할 때만 명시적으로
```

- 종료 코드 0: `Ok({out, err, code})`
- 그 외: `Err({kind: 'exit, code, out, err, message})`
- `exec([...])!` 는 쉘의 `set -e`와 같은 효과.
- 시작하지 못하면 `Err({kind: 'not_found, message: "cannot run `<이름>`: <이유>"})`(6c-2). 인자 순서대로 검사한다: 빈 이름(`empty command name`),
  NUL이 든 인자(`argument contains a NUL byte`), `$PATH`에 없음(`command not found in PATH`; 상대 경로 항목에서만 찾으면 거절,
  `command found only in a relative PATH entry`), 작업 디렉터리(NUL, 없음), 환경 변수의 NUL, 커널이 실행할 수 없는 파일
  (`not an executable file (run scripts with sh)`: shebang 없는 스크립트를 `/bin/sh`로 몰래 돌리지 않는다).
