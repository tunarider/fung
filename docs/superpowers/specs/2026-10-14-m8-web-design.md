# M8 웹 설계 (json · time · regex · http)

2026-10-14 확정. 06·09의 M8 항목을 구체화한 것이며, 결정은 해당 설계 문서에도 반영했다.

## 목표

- 네 모듈을 **내장 extras**로 추가한다: `json`, `time`(core 타입 `time` 포함), `regex`, `http`.
- 마일스톤 기준(09): "간단한 웹서버"가 fung으로 돌아간다. 네 모듈을 함께 쓰는 예제 두 개를 `examples/`에 둔다.
- 언어 문법은 바뀌지 않는다. 렉서·파서는 열지 않는다. 바뀌는 것은 값 층(`time` 타입)과 라이브러리뿐이다.

## 범위 밖

- `math`·`random` 모듈(06 표에 있으나 M8 아님), 라우터·미들웨어·TLS 서버·WebSocket, 스트리밍 응답, 쿠키·세션, `time`의 달력 산술(`add_months` 등), 영문 월·요일 이름 토큰.

## 결정 0. Go native를 언제 쓰는가 (06 수정)

지금까지 native 층의 기준은 "fung으로 **쓸 수 없는** 것만"이었다. 이를 다음으로 바꾼다.

> native(`_native.*`)는 fung으로 쓸 수 없는 것, **또는 쓸 수는 있지만 안쪽 루프라 해석기 속도로는 성능이 나오지 않는 것**을 Go로 쓴 다리다. extras는 가급적 fung으로 쓰되, 성능이 이유면 `_native.*`에 원시를 두고 fung은 얇게 감싼다.

이유: 06의 extras 기준은 원래 "core만으로 만들 수 있고 **성능도 충분**"이었다. 글자 단위로 도는 파서(JSON)는 `split`·`join`을 core에 둔 것과 같은 이유로 Go 쪽이 맞다. 경계를 측정으로 옮기는 원칙(`flatten`의 전례)은 그대로다.

## 결정 1. `json`은 `_native.json` 다리 + 얇은 `json.fung`

09의 "json(fung)"을 고친다. SRE 스크립트가 다루는 JSON(`kubectl -o json` 등)은 수 MB가 흔하고, fung 해석기로 글자를 돌면 1MB에 초 단위가 걸린다.

### API

- `json.parse(s)` → `Ok(값)` / `Err({kind: 'parse, line, col, message})`.
- `json.stringify(v, indent = nil)` → 문자열. `indent`는 `nil`(한 줄) 또는 들여쓰기 칸 수(int).

### 값 대응

| JSON | fung | 비고 |
|------|------|------|
| object | 맵 | **키 순서 유지.** Go 표준 디코더(`map[string]any`)는 순서를 버리므로 `json.Decoder.Token()`으로 토큰을 읽어 직접 조립한다 |
| array | 리스트 | |
| string | 문자열 | |
| number (소수점·지수 없음) | int | int64를 넘으면 `Err 'parse` ("조용히 float로 바꾸지 않는다") |
| number (그 외) | float | `Inf`·`NaN`은 JSON에 없다 |
| true/false | bool | |
| null | nil | |

stringify 방향:

| fung | JSON |
|------|------|
| 맵·리스트·문자열·int·float·bool·nil | 그대로 |
| 튜플 | array |
| 심볼 | string (`'ok` → `"ok"`) |
| time | string (ISO 8601) |
| 기간·함수·태그·범위·모듈·태스크·채널·atom·파일 | **런타임 에러** E0513 (`cannot convert dur to JSON`) — 코드 오류이므로 `Err`가 아님 |

- 같은 맵이 중첩 깊이를 넘는 일은 없다(값은 불변이라 순환 참조가 생기지 않는다).
- 문자열은 Go `encoding/json`과 같은 이스케이프. HTML 이스케이프(`<` → `<`)는 **끄고** 쓴다(`SetEscapeHTML(false)`).
- `parse`의 `line`·`col`은 1부터 센다. 입력 뒤에 쓰레기가 남아도 `Err`.

## 결정 2. `time` 타입 (core) + `time` 모듈 (extras)

### 값 (02·07 수정)

- `value.Time`: Go `time.Time`을 감싼다. 시간대를 보존한다(로컬에서 만든 값은 로컬, `utc`로 바꾼 값은 UTC).
- 리터럴은 없다. `time.now()`·`time.parse`·`time.from_unix`로만 만든다. 패턴 상수로 쓸 수 없다.
- `type_of(t)` → `'time`. 타입 표기 `time`은 이미 07에 있다.
- 출력(`print`·REPL·`str`): ISO 8601 `2026-10-14T09:00:00+09:00`. 소수 초는 0이 아닐 때만 붙인다(`.123`).
- 연산 (07):
  - `time == time`: 같은 순간이면 참(시간대 무관, Go `Equal`). 다른 타입과는 `false`.
  - `time < time` 등 순서 비교: 순간 기준.
  - `time + dur`, `dur + time` → time. `time - dur` → time. `time - time` → dur. 결과가 범위를 넘으면 E0503.
  - 그 외 산술(`time * 2`, `time + time`)은 E0501.
- `sort`·`unique`·`contains`·맵 키 규칙은 02의 비교 표를 따른다(맵 키는 여전히 문자열만).

### 모듈 (06 수정)

`extras/time.fung`이 `_native.time`을 재내보낸다.

| 함수 | 뜻 |
|------|------|
| `now()` | 현재 시각, 로컬 시간대 |
| `parse(s, format = nil)` | `Ok(time)` / `Err({kind: 'parse, message})`. `format`이 `nil`이면 RFC 3339(ISO 8601) |
| `format(t, format = nil)` | 문자열. `format`이 `nil`이면 ISO 8601 |
| `utc(t)` / `local(t)` | 같은 순간을 UTC / 로컬 시간대로 |
| `unix(t)` / `from_unix(n)` | 유닉스 초(int) ↔ time. `from_unix`는 로컬 시간대 |
| `parts(t)` | `{year, month, day, hour, minute, second, weekday}` 맵. `weekday`는 심볼 `'mon`…`'sun`. 시간대는 `t`의 것 |

### 형식 토큰 (open-questions 해결)

| 토큰 | 뜻 | 예 |
|------|------|------|
| `YYYY` | 연 4자리 | 2026 |
| `YY` | 연 2자리 (파싱은 2000~2099) | 26 |
| `MM` | 월 2자리 | 10 |
| `DD` | 일 2자리 | 14 |
| `HH` | 시 2자리(24시간) | 09 |
| `mm` | 분 2자리 | 05 |
| `ss` | 초 2자리 | 07 |
| `SSS` | 밀리초 3자리 | 123 |
| `Z` | 시간대 `+09:00` | |
| `ZZ` | 시간대 `+0900` | |

- 목록에 없는 글자는 그대로 출력·그대로 기대한다(`"YYYY-MM-DDTHH:mm:ss"`의 `T`, 공백, `:`).
- 토큰은 **긴 것부터** 맞춘다(`YYYY` 다음 `YY`, `SSS`, `ZZ` 다음 `Z`).
- `parse`는 같은 토큰으로 자리수를 정확히 맞춘다. 시간대 토큰이 없으면 로컬로 해석한다.
- 영문 이름(`MMM`·`ddd`)·12시간제·이스케이프(`[T]`)는 보류. 필요해지면 open-questions.
- 구현은 토큰 → Go 레이아웃 변환이 아니라 **직접 포맷·파싱**한다(Go 레이아웃은 `MM`과 `mm` 구분이 없어 변환이 애매하다).

## 결정 3. `regex` 모듈 (Go RE2)

`extras/regex.fung`이 `_native.regex`를 재내보낸다.

| 함수 | 반환 |
|------|------|
| `is_match(s, pat)` | bool (`match`는 키워드라 필드 이름으로 쓸 수 없다) |
| `find(s, pat)` | 매치 맵 또는 `nil` |
| `find_all(s, pat)` | 매치 맵 리스트 (겹치지 않는 전부) |
| `replace(s, pat, repl)` | 문자열. `repl` 안의 `$1`·`${name}`은 Go `Regexp.ReplaceAllString` 규칙 |
| `split(s, pat)` | 문자열 리스트 (`split`과 같이 빈 조각을 남긴다) |

- 매치 맵: `{text, start, end, groups, named}`. `start`·`end`는 **글자(룬) 인덱스**(fung 문자열 규칙, 02). Go의 바이트 오프셋을 룬 인덱스로 바꿔 준다(한글에서 `s[start..end]`가 맞아야 한다). `groups`는 캡처 그룹 리스트(전체 매치 제외, 안 잡힌 그룹은 `nil`), `named`는 이름 있는 그룹 맵.
- 잘못된 패턴은 **런타임 에러** E0515 `invalid pattern: <Go 메시지>` (패턴은 보통 코드에 적힌 리터럴 → 코드 오류).
- 컴파일된 패턴은 Go 쪽 캐시(뮤텍스 + 맵, 상한 256개 넘으면 비움)에 둔다. "컴파일된 regex 값" 타입은 두지 않는다.
- 패턴 문법은 RE2(Go `regexp`): 역참조·전후방 탐색 없음, 항상 선형 시간(06).

## 결정 4. `http` 모듈

`extras/http.fung`이 `_native.http`를 재내보낸다. 클라이언트와 서버 둘 다 Go `net/http`.

### 클라이언트

| 함수 | |
|------|------|
| `get(url, headers = {}, timeout = 30s)` | |
| `post(url, body, headers = {}, timeout = 30s)` | |
| `request(method, url, body = nil, headers = {}, timeout = 30s)` | `method`는 문자열(`"PUT"`) |

- 응답을 받으면 상태 코드와 무관하게 `Ok({status, headers, body})`. 404·500도 `Ok`다 ("서버가 답했다"와 "요청이 실패했다"는 다른 일).
- 전송 실패 `Err({kind: 'network, message})`, 시간 초과 `Err({kind: 'timeout, message})`. `timeout`은 기간이며 `0s`면 제한 없음.
- `body`는 문자열(JSON은 `json.stringify`로 사용자가 만든다). `headers`는 문자열 → 문자열 맵.
- 응답 `headers`: 키는 **소문자**(`content-type`), 같은 이름이 여러 개면 `", "`로 잇는다. `body`는 문자열(UTF-8이 아니어도 그대로 담는다).
- 리다이렉트는 Go 기본(최대 10번 따라감). https는 Go 기본 설정.

### 서버 (open-questions 해결)

`serve(addr, handler, ready = nil)`

- `addr`는 `"127.0.0.1:8080"`·`":8080"`. 바인드 실패는 `Err({kind: 'network, message})`.
- 바인드되면 **막힌 채로** 요청을 받는다. 끝나는 길은 둘: 그 태스크가 취소되거나(`t = spawn(() => http.serve(...))` 뒤 `cancel(t)`), `exit`. 취소되면 `sleep`·`recv`처럼 그 태스크가 취소로 끝난다(`await(t)` → `Err({kind: 'cancelled})`, 05). 돌아오는 값은 없다 — `serve`가 값을 돌려주는 경우는 바인드 실패 `Err`뿐이다. 서버 소켓은 취소 시 `Shutdown`으로 닫는다.
- `ready`가 함수면 리스닝 직후 실제 주소 문자열(`"127.0.0.1:53121"`)로 한 번 부른다. `":0"`으로 띄운 서버의 포트를 아는 유일한 길이고, 테스트가 이것으로 "준비됨"을 안다.
- 핸들러는 `req => resp`:
  - `req = {method, path, query, headers, body}`. `query`는 문자열 → 문자열 맵(같은 키가 여럿이면 첫 값). `headers`는 소문자 키. `body`는 문자열.
  - `resp`는 맵 `{status = 200, headers = {}, body = ""}`(없는 키는 기본값) 또는 **문자열**(200, `text/plain; charset=utf-8`). 맵에 `content-type`이 없고 `body`가 비어 있지 않으면 같은 기본값을 붙인다.
  - 그 외 값을 돌려주거나 런타임 에러가 나면 **500**을 답하고, stderr에 한 줄(`http: handler failed: error[E0803]: …`, 소스 발췌·호출 경로 없음)을 찍고, 서버는 계속 돈다. 종료 코드에는 영향이 없다. 핸들러 안의 `exit`·취소는 핸들러 버그가 아니므로 503으로 답하고 보고하지 않는다.
- 요청마다 Go 고루틴이 뜨므로 핸들러는 **요청마다 새 fung 태스크**에서 돈다(VM 상태는 태스크별, 05). 핸들러들은 동시에 돌 수 있으므로 공유 상태는 `atom`으로 쓴다.
- 라우팅은 없다. `match req.path:`로 쓴다.

### 런타임 연동 (`value.Context` 확장)

동시성 내장 함수는 vm 안에서 `ctx.(*Task)`로 태스크를 직접 쓰지만, `http`는 다른 다리처럼 `internal/native`에 둔다. 그래서 `value.Context`에 두 메서드를 더한다:

- `Done() <-chan struct{}`: 이 태스크의 취소 신호. `serve`가 이것으로 `Shutdown`한다. `get` 등 클라이언트도 요청 컨텍스트로 넘겨 취소 시 중단한다.
- `RunTask(fn Value, args []Value) (Value, error)`: 호출한 고루틴 위에 새 태스크를 만들어 `fn`을 끝까지 돌리고 결과를 돌려준다. 교착 감시자에 살아 있는 태스크로 센다(`Runtime.Wait`는 따로 기다리지 않는다 — `serve` 태스크가 `Shutdown`으로 핸들러를 기다린다). 핸들러 실행에 쓴다.

교착 감시자(05)는 `block()` 안의 대기만 세므로 `serve`가 Go에서 막혀 있어도 오탐은 없다(`fs.read`·`exec`과 같은 취급).

## 에러 번호 (신규)

| 번호 | 메시지 | 어디서 |
|------|------|------|
| E0515 | `invalid pattern: %s` | `regex.*`의 잘못된 패턴 |

기존 번호 재사용: E0513 `cannot convert %s to %s`(stringify 불가 타입 → `cannot convert fn to JSON`), E0501 `cannot %s %s and %s`(time 산술 오류), E0503 오버플로, E0403 인자 타입.

`Err`의 `kind`는 기존 어휘로 충분하다: `'parse`(json·time), `'network`·`'timeout`(http).

## 구조

```
internal/value/time.go        Time 값, 산술·비교·출력 (arith.go·compare.go·format.go에 분기 추가)
internal/native/json.go       _native.json: parse, stringify
internal/native/time.go       _native.time: now parse format utc local unix from_unix parts
internal/native/regex.go      _native.regex: match find find_all replace split (+ 캐시)
internal/native/http.go       _native.http: get post request serve
extras/json.fung, time.fung, regex.fung, http.fung   재내보내기
examples/serve.fung           /health(json)·/echo(POST) 서버 + test 블록(spawn·get·cancel)
examples/checkurls.fung       URL 목록을 pmap으로 get, 상태·소요 시간(time) 요약, JSON 본문이면 필드 추출
```

- `native.Bridges()`에 네 모듈을 더한다. `extras.Default()`는 그대로(embed가 새 파일을 집는다).
- `internal/`은 `extras/`를 import하지 않는다는 테스트(`tests/layers_test.go`)는 그대로 지킨다.

## 테스트

- 골든: `json`(parse 왕복·순서·에러 위치), `time`(고정 문자열을 `parse`해 `format`·연산·비교 — `now()`는 골든에 넣지 않는다), `regex`(룬 인덱스·그룹·치환·split), `http`(`":0"` + `ready`로 서버를 띄워 같은 스크립트에서 `get`·`post`하고 `cancel`).
- Go 단위: 각 다리의 인자 검사와 에러 분기, `time` 값 연산(오버플로 경계), 토큰 포맷·파싱 표, regex 캐시, `httptest`로 타임아웃·리다이렉트·헤더 합치기, 핸들러 에러 → 500.
- `-race`: `http` 골든과 `RunTask`는 vm·cmd 패키지 `-race`로도 돈다.
- 성능 확인(테스트 아님): 1MB JSON `parse`가 수십 ms 안.

## 문서

- 설계 문서 갱신(이 스펙과 같은 커밋): 02(`time` 값·비교 표), 06(native 기준, 네 모듈 API, 시간 형식 토큰, 에러 kind), 07(time 연산), 09(M8 행), open-questions(해결 2건, 신규 1건: 사용자 코드의 `_native.*` 접근).
- 완료 후 `docs/notes/M8-web.md`에 무엇을 만들었고 왜 그렇게 했는지(특히 "native를 언제 쓰는가", 룬 인덱스, 서버와 태스크) 남긴다.
