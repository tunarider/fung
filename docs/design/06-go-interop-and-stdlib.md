# 06. Go 연동 · 표준 라이브러리

## 내장 함수 연결 방식

- 내장 함수는 **손으로 감싼다.** fung 값 검사 → Go 호출 → fung 값으로 변환.
- 인자 검사 등 반복 작업은 도우미 함수로 줄인다.
- 리플렉션 자동 연결은 쓰지 않는다 (Go의 이름·타입·에러 방식이 새어 나오므로).
- **내장 함수도 fung 함수와 같은 호출 규약을 따른다.** 매개변수 목록(이름, 기본값, `...rest`)을 선언하고,
  위치·이름 인자·기본값·개수 검사는 VM이 한 곳에서 한다. `exec(cmd, cwd: d)`가 되는 이유이며,
  "내장 함수는 이름 인자가 안 된다" 같은 예외 조항을 두지 않는다 (배경 원칙 7).
  값에 따라 달라지는 기본값(예: 현재 디렉터리)은 `nil`로 받아 Go 쪽에서 채운다.
- `exit(n)`은 Go의 `os.Exit`를 직접 부르지 않는다. VM 안에서 **종료 신호**로 올라가고(에러와 같은 경로, 호출 경로·메시지 없음),
  `fung` 명령이 받아 `at_exit`를 돌린 뒤 그 코드로 끝낸다. 같은 프로세스 안의 테스트·REPL이 깨지지 않기 위해서다.

## 네 층: core · native · extras

| 층 | 뜻 | 언어 | 예 |
|------|------|------|------|
| **core** | 언어의 의미와 런타임. 없으면 다른 언어가 된다. "언어가 보장하는 값 연산"을 포함한다 | Go | 렉서~VM, 값·에러, 태스크·채널·atom, `exit`·`at_exit`, `print`, `map`·`filter`·`fold`·`sort`, `split`·`join`, `int`·`str` |
| **native** (`_native.*`) | 바깥세상과의 다리. fung으로 **쓸 수 없는** 것, 또는 쓸 수는 있지만 **안쪽 루프라 성능이 나오지 않는** 것. 숨김, 최소 | Go | `_native.fs`, `_native.path`, `_native.process`, `_native.conc`, `_native.json`, `_native.regex`, `_native.time`, `_native.http` |
| **extras (내장)** | core·native 위에 **fung으로** 쓴 것. 없어도 언어는 같지만 불편하다 | fung | `fs`·`path`·`cli` 모듈, `with_open`, `take`·`drop`·`lines`, `until`·`retry`·`pmap` |
| **extras (프로젝트)** | 사용자 패키지. 같은 메커니즘(맨 경로 import), 의존성 관리는 추후 | fung | — |

- 기준: core는 "최소한 언어가 이 정도는 보장한다"; extras는 "core만으로 만들 수 있고(성능도 충분) 없으면 너무 불편해서 넣어 둔
  것"이며 **가급적 Go를 쓰지 않는다**. native는 그 둘 사이에서 Go가 꼭 필요한 좁은 다리다.
- **Go native를 쓰는 때** (2026-10-14 확정): 가급적 쓰지 않되, **성능이 이유면 extras에서도** `_native.*`에 원시를 두고 fung은 얇게
  감쌀 수 있다. 글자 단위로 도는 파서(`json`)는 `split`·`join`을 core에 둔 것과 같은 이유로 Go에 둔다. 경계는 측정으로 옮긴다.
- 우선순위(이름 해석): core > 내장 extras > 프로젝트 extras.
- 의존 방향: **core는 extras를 import하지 않는다.** fung으로 쓴 extras는 core의 prelude와 `_native.*` 다리만 쓴다. Go 쪽 `extras`
  패키지는 "라이브러리의 모양"인 `frontend.Registry`(모듈 이름 → fung 소스, `_native.*` 모듈 표)에 내장 파일과 다리를 담아 돌려줄
  뿐이다. Go 패키지 경계로 강제한다(`internal/`은 `extras/`를 모른다; 조립은 `cmd/fung`만 한다; `tests/layers_test.go`가 `go list -deps`로
  확인한다).
- 원소를 도는 함수(`map` `filter` `fold` `each` `sort` `find`…)와 문자열·변환 원시는 안쪽 루프라 core(Go)에 둔다. 그 위의 조합
  (`take` `drop` `first` `sum` `min` `lines` `pad_*` `is_ok`…)은 fung extras다. 경계는 측정해서 옮길 수 있고 의미는 바뀌지 않는다(골든).
  측정으로 돌아온 예: `flatten`은 fung에서 리스트를 누적하면(`acc + x`) 제곱 시간이라 core에 남았다(M7.5 노트).
- fung extras의 인자 검사는 `fail`(E0803)로 한다. E0403 같은 호출 규약 에러는 Go 원시만 낸다.
- 내장 extras 안에서 난 런타임 에러의 위치는 **사용자 코드의 가장 가까운 프레임**이다(사용자는 내장 파일을 열 수 없다). 호출 경로에는
  내장 프레임이 그대로 보인다. 로더가 내장 파일의 `Proto`에 `Builtin`을 표시하고 VM이 그것을 본다.
- fung으로 쓴 extras는 실행 파일에 내장한다 (Go `embed`, `extras/*.fung`). 배포는 여전히 `fung` 하나.
- Go 함수가 사용자 람다를 다시 호출할 수 있어야 한다 (태스크별 VM 상태 분리로 가능).
- fung으로 쓴 모듈은 **import될 때** 컴파일한다 (prelude만 시작 시). 스크립트 시작 시간을 지키기 위해서다.

## 사용자 Go 확장

- 지금은 열지 않는다. 필요한 Go 기능은 fung 본체에 추가한다.
- `_native.*`도 사용자 코드(프로젝트 extras)에 열지 않는다 (2026-10-14 확정). 공개 모듈이 원시를 **같은 Go 함수 값으로** 재내보내므로
  (`read = _n.read`) 사용자 라이브러리가 `import fs`로 쓰는 것과 비용이 같다. 숨겨진 것은 정리를 강제하려고 감싼 `temp_dir`과 내부용
  `spawn_awaited`뿐이다. 열면 원시의 이름·인자가 공개 API로 굳는다.
- 외부 기능은 `exec` / `sh`로 다른 프로그램을 호출한다.
- Go 플러그인(.so)은 쓰지 않는다 (Go 버전 일치 필요, Windows 미지원 등 불안정).

## 에러 규칙

- **외부 요인으로 실패할 수 있는 것** (파일, 네트워크, 파싱) → `Ok` / `Err` 반환.
- **코드가 틀린 것** (타입 오류, 범위 밖 인덱스, 0으로 나누기) → 런타임 에러로 중단.
- 변환 함수 `int`·`float`·`str`·`sym`은 잘못된 입력에 **런타임 에러** (코드 오류 취급). 그래서 `int("5") + 3`이 그대로 된다.
  사용자 입력처럼 믿을 수 없는 문자열은 `parse_int(s)`·`parse_float(s)`가 `Ok`/`Err`로 돌려준다.
  "이 문자열은 숫자여야 한다"(단언)와 "숫자인지 확인한다"(검사)를 이름으로 구분한다.
- 사용자 코드가 "여기 오면 버그다"를 말할 때는 `fail(msg)`: 메시지와 함께 런타임 에러를 일으킨다.
  `x ?? fail("config.port is required")`처럼 `??`와 짝이 맞는다. (Go 용어 `panic`은 쓰지 않는다.)
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
- 타입·변환: `type_of`, `int`, `float`, `str`, `sym`, `parse_int`, `parse_float`, `parse_dur`
  - `parse_dur(s)`: 기간 리터럴 문법(`1h30m`, `500ms`, `-5s`, 앞뒤 공백 허용)을 `Ok(dur)`/`Err 'parse`로 (2026-10-14). `--timeout 30s` 같은 옵션용.
- 컬렉션: `len`, `is_empty`, `map`, `filter`, `fold`, `each`, `sum`, `min`, `max`, `sort`, `sort_by`, `reverse`, `range`, `zip`, `first`, `last`, `take`, `drop`, `find`, `any`, `all`, `contains`, `unique`, `flatten`, `group_by`, `keys`, `values`, `pairs`, `get`, `put`, `merge`, `remove`, `chars`
  - `each(xs, f)`: 부수효과용 순회, `nil` 반환 (`for`는 리스트를 만든다).
  - `chars(s)`, `pairs(m)`, `keys(m)`, `values(m)`: 문자열과 맵을 순회 가능한 리스트로 (02).
- 문자열: `split`, `join`, `trim`, `upper`, `lower`, `replace`, `starts_with`, `ends_with`, `pad_left`, `pad_right`, `repeat`, `lines`
  - `lines(s)`: 텍스트를 줄 리스트로 — `read_lines`와 같은 규칙(`\n` 구분, 줄 끝 `\r` 제거, 끝의 줄바꿈은 빈 줄을 만들지 않음).
    `split(s, "\n")`은 끝에 빈 조각을 남기므로 줄 나누기에는 `lines`를 쓴다.
- 결과값: `is_ok`, `is_err`
- 반복 패턴 (fung 코드, 00의 "상태가 있는 반복은 재귀" 보완): `retry(f, times = 3, delay = 0ms)`, `until(x, f, pred)`, `iterate(x, f, n)`
- 에러·정리: `fail`, `at_exit`
- 동시성: `spawn`, `await`, `cancel`, `channel`, `send`, `recv`, `close`, `atom`, `swap`, `deref`, `sleep`, `fan_in`, `pmap` (05)
- 프로세스: `exec`, `sh`, `env`, `args`, `exit`
  - `args`: 진입 스크립트 뒤의 인자만 담은 **문자열 리스트** (prelude 값).
  - `env(name)`: 문자열 또는 `nil`. `env()`: 전체 맵.
  - `exit(code = 0)`.

`contains`는 리스트·문자열·맵 모두에 동작한다.

## 모듈 (1차안, 알파 버전에서 써보며 조정)

| 모듈 | 내용 |
|------|------|
| `fs` | read, read_lines, write, append, list(`dir, full = false` — `full`이면 `dir`을 붙인 경로), glob, walk, exists, stat, mkdir, remove, remove_all, move, copy, open, close, write_line, with_open, with_temp_dir |
| `path` | join, base, dir, ext, stem, abs |
| `cli` | `parse(argv, bools = [])` → `{flags, rest}`. `--name value`·`--name=value`·`--name`(true, `bools`에 있으면 값을 먹지 않음), `--` 뒤는 전부 rest, 이름의 `-`는 `_`로. 검사·변환은 보통 코드로 (fung으로 작성) |
| `json` | `parse(s)` → `Ok(값)` / `Err({kind: 'parse, line, col, message})`, `stringify(v, indent = nil)`. 아래 "JSON" 참고 (`_native.json`, M8) |
| `http` | `get(url, headers = {}, timeout = 30s)`, `post(url, body, headers = {}, timeout = 30s)`, `request(method, url, body = nil, headers = {}, timeout = 30s)`, `serve(addr, handler, ready = nil)`. 아래 "HTTP" 참고 (Go는 `_native.http`, M8; Rust는 `_native.net` 위의 fung, 4d) |
| `time` | `now()`, `parse(s, format = nil)` → `Ok`/`Err 'parse`, `format(t, format = nil)`, `utc(t)`, `local(t)`, `unix(t)`, `from_unix(n)`, `parts(t)`. `now()`는 `time` 타입 값 (02·07): `==`, `<`, `+ dur`, `time - time → dur`. 아래 "시간 형식" 참고 (`_native.time`, M8) |
| `regex` | `is_match(s, pat)` → bool, `find(s, pat)` → 매치 맵 또는 `nil`, `find_all(s, pat)`, `replace(s, pat, repl)`, `split(s, pat)`. 아래 "정규식" 참고 (`_native.regex`, M8) |
| `math`, `random` | (M8 이후) |

추후 후보: `csv`, `hash`, `base64`, `gui`

## JSON

- `parse`: object → 맵(**키 순서 유지**), array → 리스트, 소수점·지수 없는 숫자 → int(int64를 넘으면 `Err 'parse`), 그 외 숫자 → float,
  `null` → `nil`. 입력 뒤에 쓰레기가 남아도 `Err`. `line`·`col`은 1부터.
- `stringify`: 맵·리스트·문자열·int·float·bool·nil은 그대로, 튜플 → array, 심볼 → string, time → ISO 8601 string. 기간·함수·태그·범위
  등은 **런타임 에러** E0513(`cannot convert dur to JSON`, 코드 오류). `indent`는 `nil`(한 줄) 또는 들여쓰기 칸 수. HTML 이스케이프는 하지 않는다.

## 정규식

- Go RE2: 역참조·전후방 탐색 없음, 대신 항상 선형 시간.
- 매치 맵 `{text, start, end, groups, named}`. `start`·`end`는 **글자(룬) 인덱스**(02의 문자열 규칙; `s[start..end]`가 맞는다).
  `groups`는 캡처 그룹 리스트(전체 매치 제외, 안 잡힌 그룹은 `nil`), `named`는 이름 있는 그룹 맵.
- `replace`의 `repl`은 `$1`·`${name}` 치환(Go 규칙). `split`은 `split(s, sep)`처럼 빈 조각을 남긴다.
- 잘못된 패턴은 런타임 에러 E0515(코드 오류). 컴파일한 패턴은 Go 쪽에서 캐시한다. 별도 "컴파일된 regex 값" 타입은 없다.

## HTTP

**fung HTTP 약속** (2026-10-16, Rust 이식 4d). 두 구현이 지키는 동작이다. Go 구현(0.1)은 이것을 Go `net/http`로, Rust 구현은
`_native.net`(소켓·TLS 원시 기능) 위의 fung 코드(`extras/rust/http.fung`)로 지킨다. 약속 안에서는 두 구현이 같고, 약속 밖의 차이는
open-questions "Rust 이전"의 "0.2.0에서 바뀌는 것"과 "4d단계(http)에서 알고 남긴 차이"에 있다. 학습 노트 `docs/notes/R4d-net-http.md`.

**클라이언트** (`get` `post` `request`)
- 응답을 받으면 상태 코드와 무관하게 `Ok({status, headers, body})` (404도 `Ok` — "서버가 답했다"와 "요청이 실패했다"는 다른 일).
  전송 실패 `Err({kind: 'network, message})`, 시간 초과 `Err({kind: 'timeout, message: "request to `<url>` timed out"})`.
  `body`는 문자열. 응답 `headers`의 키는 **소문자**, 같은 이름이 여럿이면 `", "`로 잇는다.
- HTTP/1.1. 요청에 `Host`, `User-Agent: Go-http-client/1.1`(Go 기본값, 과도기 잔가지), 본문이 있으면 `Content-Length`.
  사용자 헤더가 같은 이름이면 사용자 것. URL에 사용자 정보(`user:pass@`)가 있고 `Authorization`이 없으면 `Authorization: Basic …`.
- 응답 본문은 `Content-Length`, `Transfer-Encoding: chunked`, 연결 닫힘까지 중 하나로 읽는다. `HEAD` 응답과 1xx·204·304는 본문 없음.
- **리다이렉트**(Go `Client` 규칙): 301·302·303 → `GET`(원래 `HEAD`면 `HEAD`), 본문 버림. 307·308 → 메서드·본문 유지.
  요청 10번째의 답이 또 리다이렉트면 `Err 'network`(`stopped after 10 redirects`). 다른 호스트(첫 요청의 호스트나 그 하위 도메인이 아닌 곳)로
  넘어가면 `Authorization`·`Cookie` 같은 민감한 헤더를 버린다. `Location`이 상대 경로면 현재 URL 기준으로 푼다.
- **프록시**(Go `ProxyFromEnvironment`): `HTTP_PROXY`/`http_proxy`(http용), `HTTPS_PROXY`/`https_proxy`(https용), `NO_PROXY`/`no_proxy`.
  `localhost`·루프백 주소는 거치지 않는다. http는 절대 URL 요청, https는 `CONNECT` 터널 뒤 TLS.
- **https**: 운영체제 루트 인증서(`SSL_CERT_FILE`·`SSL_CERT_DIR`이 있으면 그것), SNI. 인증서 오류는 `Err 'network`.
- **`timeout`**: 기간, 리다이렉트를 포함한 요청 전체(Go `Client.Timeout`). `0s`면 제한 없음.
- **에러 문구**: `cannot request `<url>`: <이유>`. 흔한 이유(연결 거부, 호스트 없음, 인증서 오류, 리다이렉트 초과, 잘못된 URL)는 두 구현이 같다.
- 압축 요청을 드러내지 않는다: 본문은 언제나 풀린 것이다(Go는 `gzip`을 요청하고 스스로 풀며, Rust는 요청하지 않는다).
- 취소되면 E0701.
- **Rust만**: 응답 본문 상한 **64 MiB**(넘으면 `Err 'network`; 2026-10-16 사용자 결정, 더 큰 응답은 나중에 요청별 옵션으로). Go 구현에는 상한이 없다.
  SOCKS 프록시와 https 프록시 너머의 https는 Rust에서 지원하지 않는다(`Err 'network`, Go는 지원).

**서버** `serve(addr, handler, ready = nil)`
- 바인드 실패 `Err 'network`, 되면 **막힌 채로** 요청을 받는다. 끝나는 길은 그 태스크의 취소(`cancel(t)`) 또는 `exit`.
  `ready`가 함수면 리스닝 직후 실제 주소(`"127.0.0.1:53121"`)로 한 번 부른다(`":0"`의 포트를 아는 길).
- 핸들러 `req => resp`. `req = {method, path, query, headers, body}`(`query`는 첫 값만, 퍼센트 디코딩은 Go `url.ParseQuery`와 같음, `headers`는 소문자 키).
  `resp`는 맵 `{status = 200, headers = {}, body = ""}` 또는 문자열(200, `text/plain; charset=utf-8`). 응답 헤더에 `Date`(Go 형식)와
  `Content-Length`가 붙고, 본문이 있는데 `content-type`이 없으면 `text/plain; charset=utf-8`.
- 핸들러가 런타임 에러를 내거나 다른 타입을 돌려주면 500을 답하고 stderr에 한 줄(`http: handler failed: …`)을 찍고 서버는 계속 돈다(종료 코드 무관).
  핸들러 안의 `exit`·취소는 503이며 보고하지 않는다. 응답 본문 문구는 Go `net/http`와 같다.
- 요청마다 **새 fung 태스크**에서 핸들러가 돈다(동시에 돌 수 있다; 공유 상태는 `atom`). 라우팅은 없고 `match req.path:`로 쓴다.
  Rust 구현에는 `atom`이 없고 `agent`를 쓴다(open-questions "Rust 이전"의 `atom` 항목; 예제는 6단계에서 고친다).
- 한도 (2026-10-14, 고정값): 헤더를 10초 안에 보내지 않는 연결은 끊고, 요청 본문은 16MB까지(넘으면 413, 핸들러는 불리지 않음), 형식 오류는 400.
  조각 전송 요청 본문도 받는다.
- 취소되면 리스너를 닫고 처리 중인 요청을 최대 2초 기다린 뒤 E0701(Go `Shutdown`).
- 런타임 연동: Go는 `value.Context`의 `Done()`(취소 신호)과 `RunTask(fn, args)`(새 태스크에서 실행)로 vm을 모르는 `internal/native`에 둔다.
  Rust는 `_native.net`의 `hold`·`close_on_cancel`과 `_native.conc`의 `set_asker`·`describe`로 fung 코드가 서버 루프를 쓴다.
- **Rust만**: 연결마다 요청 하나라서 HTTP/1.1 응답에 늘 `Connection: close`가 붙는다(Go는 닫을 때만). 2 KB 넘는 본문도 `Content-Length`로 보낸다(Go는 조각 전송).

**약속 밖**(알려진 차이, 0.2.0에서 바뀌는 것): 연결 재사용(keep-alive)·파이프라이닝, HTTP/2, 압축, 바이너리 본문(UTF-8이 아니면 U+FFFD),
Go가 덧붙이는 그 밖의 헤더, 흔하지 않은 에러 문구.

## 파일 핸들

- `fs.open(path, mode = 'read)` → `Ok(file)` / `Err`. `mode`는 `'read` `'write` `'append`. `fs.close(f)` → `Ok(nil)`.
- `fs.with_open(path, f, mode = 'read)`: 열고 `f(핸들)`를 부르고 **결과가 무엇이든** 닫은 뒤 그 결과를 돌려준다 (fung 코드, 03).
  람다 안의 `?`는 람다에서 `Err`를 돌려주는 것이므로 그대로 닫히고 전달된다.
- `read_line(f)` `read_lines(f)`(prelude), `fs.write_line(f, s)` → `Ok(nil)` / `Err`.
- 닫힌 핸들에 읽고 쓰면 `Err({kind: 'closed})`. 핸들을 `with_open` 밖으로 돌려줘도 막지 않는다 — 쓰는 순간 `'closed`다.
- 핸들은 태스크 사이에 공유할 수 있다 (2026-10-14): 연산(`read_line`·`read_lines`·`write_line`·`close`) 하나하나가 핸들의 뮤텍스로
  직렬화되어 줄이 찢어지거나 섞이지 않는다. 어느 태스크가 어느 줄을 받는지는 정해지지 않는다(`atom`처럼 "값은 공유, 연산은 원자적").

## 시간 형식

- 토큰 방식: `"YYYY-MM-DD HH:mm:ss"`
- 형식을 지정하지 않으면 ISO 8601 (`2026-10-12T22:45:00+09:00`). `print`·`str`도 같은 형식(소수 초는 0이 아닐 때만).
- 기본 시간대는 컴퓨터의 로컬 시간대. 값은 시간대를 보존한다(`utc(t)`로 바꾼 값은 UTC로 찍힌다).
- 토큰 (2026-10-14 확정): `YYYY` `YY`(파싱은 2000~2099) `MM` `MMM`(영문 월 약자 `Oct`) `DD` `ddd`(영문 요일 약자 `Sun`) `HH`(24시간) `mm` `ss`
  `SSS`(밀리초) `Z`(`+09:00`) `ZZ`(`+0900`). 파싱에서 영문 이름은 대소문자를 가리지 않고, `ddd`는 이름만 검사하며 날짜와 대조하지 않는다.
  목록에 없는 글자는 그대로 출력·그대로 기대한다. 긴 토큰부터 맞춘다. `parse`는 같은 토큰으로 자리수를 정확히 맞추고, 시간대 토큰이
  없으면 로컬로 해석한다. 12시간제·이스케이프는 보류.
- `parts(t)` → `{year, month, day, hour, minute, second, weekday}`, `weekday`는 `'mon`…`'sun`.

## 명령 실행

```
exec(["mv", old, new])          # 인자 리스트. 쉘을 거치지 않음. 공백·특수문자 안전
exec(cmd, cwd: dir, env: {...}, stdin: s)   # 옵션은 이름 붙인 인자
sh("ls -la | grep .log")        # 쉘 기능이 필요할 때만 명시적으로
```

- 종료 코드 0: `Ok({out, err, code})`
- 그 외: `Err({kind: 'exit, code, out, err, message})`
- `exec([...])!` 는 쉘의 `set -e`와 같은 효과.
