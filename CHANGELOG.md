# Changelog

fung의 사용자에게 보이는 변화를 버전별로 적는다. 형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
버전은 [유의적 버전](https://semver.org/lang/ko/)을 따른다. `0.x` 동안은 마이너 버전에서 호환이 깨질 수 있다.

새 변화는 `[Unreleased]`에 쌓고, 릴리스할 때 그 절을 새 버전 제목으로 바꾼다. 릴리스 워크플로는 해당 버전 절을
GitHub 릴리스 노트로 쓰며, 절이 없으면 릴리스가 실패한다.

## [Unreleased]

fung 실행 파일이 Go 구현에서 Rust 구현으로 바뀌었다. 언어와 표준 모듈은 아래에 적은 것 말고는 그대로다.
`atom` 제거 때문에 호환이 깨지는 릴리스다.

### 바뀜(호환 깨짐)

- `atom`이 없어지고 `agent`가 대신한다. 옮겨 쓰기는 `atom(` → `agent(`이고 `swap`·`deref`는 이름 그대로다.
  타입 표기도 `fn f(a: agent)`, `type_of`는 `'agent`.
- `swap(a, f)`의 `f`는 부른 태스크가 아니라 **에이전트의 태스크**에서 돈다. 요청은 받은 순서대로 하나씩 처리한다.
  - `f`가 실패하면 상태는 그대로이고, 같은 에러가 부른 쪽 `swap(` 자리에서 난다.
  - `f` 안의 `exit(n)`은 부른 쪽의 `exit(n)`이 되고, 에이전트는 계속 일한다.
  - `f` 안(과 `f`가 띄운 태스크)의 `deref`는 바꾸기 전 값이다.
  - 부른 태스크가 취소되면(`exit`, 교착 보고, `cancel`) 그 `f`도 취소되고 상태는 바뀌지 않는다(atom과 같은 결과).
- 에이전트마다 운영체제 스레드가 하나라서 **값마다 칸을 두던 프로그램은 `atom(` → `agent(`으로 그대로 옮겨지지 않는다.**
  살아 있는 에이전트가 수천 개면 스레드 한도에 걸려 `E0801 cannot start a task`로 끝나고, 만들기와 `swap`도 atom보다 수십 배 느리다.
  에이전트 하나에 맵으로 모은다.
- 다른 에이전트를 겹쳐 쓰는 `f`는 교착이 될 수 있다: `swap(a, x => swap(b, y => deref(a) + 5))`는 atom에서 6이었지만 이제 E0702다.
  `f` 안의 `deref`가 기다리지 않는 것은 같은 에이전트일 때뿐이다.
- 서로의 핸들을 품은 두 에이전트(`b = agent(a)` 뒤 `swap(a, _ => b)`)는 핸들이 모두 사라져도 끝나지 않고 프로그램 끝까지 남는다.
  프로그램이 끝나는 것을 막지는 않는다.
- 에이전트는 `<agent>`로 출력된다. atom은 `<atom 1>`처럼 값을 보였다.
- 새 에러 E0704 `agent called from inside its own swap`: `f` 안에서 같은 에이전트에 `swap`했다. atom의 E0703을 잇는다.
- 새 에러 E0705 `channel sent into itself`: `send(ch, v)`의 `v` 안에 `ch`가 들어 있다.
- `fan_in`·`pmap`·`retry`가 prelude에서 `conc` 모듈로 옮겨졌다. `import conc` 뒤 `conc.pmap(…)`처럼 부른다.
  prelude 이름 그대로 부르면 정의되지 않은 이름 에러다.
- `http` 클라이언트의 응답 본문은 **64 MiB까지**다. 넘으면 `Err 'network`. 0.1에는 상한이 없었다.
- `http`에 연결 재사용(keep-alive)·파이프라이닝·HTTP/2·압축이 없다. `serve`는 연결마다 요청 하나만 받아 HTTP/1.1 응답에 늘
  `Connection: close`를 붙이고, HTTP/1.0 keep-alive 요청에 `Connection: keep-alive`로 답하지 않는다. 2 KB 넘는 응답 본문도
  조각 전송이 아니라 `Content-Length`로 보낸다(클라이언트가 읽는 결과는 같다).
- `http`는 SOCKS 프록시(`socks5://`·`socks5h://`)와 `https://` 프록시 너머의 https를 지원하지 않는다(`Err 'network`).
- `serve`가 끝나는 중에 읽은 요청에는 답 없이 닫지 않고 503으로 답한다.
- `fung test`: 테스트 파일 안에서 교착이 보고되거나 띄운 태스크가 `exit(n)`을 부르면, 직접 부른 `exit(n)`처럼 그 파일의 나머지 블록을
  건너뛰고 메모 한 줄을 남긴다. 건너뛰기는 실패 하나로 센다(앞 블록이 통과했으면 `FAIL <파일>: <rest of file>`,
  최상위에서 걸렸으면 `FAIL <파일>: <top level>`). 0.1은 남은 블록을 돌렸다.
- REPL:
  - 한 입력 안의 교착은 그 입력만 실패시키고 세션은 계속된다. 정의와 에이전트는 남고, 그 뒤 정상으로 끝나면 종료 코드는 0이다.
  - 띄운 태스크의 `exit(n)`은 프롬프트에서 친 `exit(n)`처럼 세션을 코드 n으로 끝낸다. 터미널에서는 다음 줄을 친 뒤에 끝나고, 그 줄은 실행하지 않는다.
  - 다음 입력을 기다리는 태스크는 사람이 천천히 쳐도 교착이 아니다.
  - 에이전트는 입력마다가 아니라 세션 끝(`:quit`, 입력 끝, `exit(n)`)에 끝난다.
- 표준 입력을 기다리던 태스크도 취소된다. 그래서 다른 태스크가 `exit(n)`을 부르면 아무도 치지 않는 줄을 기다리지 않고 곧 끝난다.
  0.1은 줄이나 입력 끝이 올 때까지 끝나지 않았다.
- `upper`·`lower`가 유니코드 전체 매핑을 쓴다: `upper("straße")`는 `"STRASSE"`, `lower("ΟΔΟΣ")`는 `"οδος"`. 0.1은 글자 하나를 글자 하나로만 바꿨다.
- 아주 큰 범위의 에러가 내부 에러(E0801)에서 한도 에러로 바뀌었다: 결과가 모든 원소를 담는 함수(`map`·`sort`·`flatten`·`group_by` 등)는 원소 1억 개,
  문자열(`repeat`·`join`)은 1억 글자를 넘으면 `E0806`. 인자의 타입 에러가 먼저 나고, `find`·`any`·`all`은 큰 범위에서도 바로 답한다.
  `channel(n)`의 버퍼 크기 상한은 없어졌다.
- `float`·`parse_float`는 실수 리터럴 문법만 받는다: `".5"`·`"5."`·16진 실수(`"0x1p4"`)가 이제 에러다.
- 채널을 순회하는 함수(`for`·`each`·`map`·`filter`·`fold`·`find` 등)가 값이 올 때마다 함수를 부른다. 0.1은 채널이 닫힐 때까지 모두 받은 뒤에
  함수를 불러서, `for x in ch:`가 채널이 닫히기 전에는 아무것도 하지 않았다. `find`·`any`·`all`·`take`·`first`는 멈춘 뒤의 값을 채널에 남기고,
  함수 인자가 틀리면 아무것도 받기 전에 에러가 난다.
- int 폭보다 넓은 범위(`-9000000000000000000..9000000000000000000` 등)가 빈 범위로 취급되던 문제. 이제 원소를 정확히 세고, 그 `len()`은 `E0503 integer overflow`다.
- `fs.glob`의 잘못된 패턴(`a*[`)이 디렉터리 내용과 상관없이 항상 `Err('invalid)`다. 0.1은 그 조각에 맞는 이름이 있을 때만 에러였다.
- `fs.remove_all`이 루트(`/`)와 마지막 성분이 `..`인 경로(`x/..`)를 거절하고, `link/`처럼 쓴 링크는 링크만 지운다. 0.1은 `x/..`에 부모 디렉터리를, `link/`에 링크가 가리키는 디렉터리를 지웠다. `fs.mkdir`는 같은 이름의 파일이 있으면 `'exists`, `fs.walk`의 에러는 읽지 못한 디렉터리를 가리킨다.
- `path.ext`·`path.stem`: 점으로 시작하는 이름(`.hidden`, `..`)은 확장자가 없다. 0.1은 `path.ext(".hidden")`이 `".hidden"`이었다.
- `close` 없이 버린 쓰기 핸들도 쓴 내용을 파일에 남긴다(마지막 사본이 사라질 때 비움). 0.1은 마지막 4 KiB 미만을 잃었다. 핸들의 버퍼는 64 KiB다.
- 에러 메시지와 파일 핸들 표시의 따옴표 문자열이 fung 문자열 리터럴 형식이다: `"\u{FEFF}"`, `<file "a\u{1}">`. 0.1은 `\ufeff`·`\x01` 꼴이었다.
- `exec`·`sh`가 시작하지 못할 때의 이유 문구가 바뀌었다: `command not found in PATH`, `empty command name`, `argument contains a NUL byte`, `not an executable file (run scripts with sh)` 등. 운영체제 에러 문구는 흔한 것이 플랫폼마다 같다.
- 시각의 범위가 0000~9999년이다(`time.from_unix` 밖이면 E0403, 산술은 E0503). 0.1은 수천억 년까지 만들었다.
- 오프셋에 초가 있으면 출력에 초까지 쓴다(`-04:56:02`). 숫자 오프셋으로 읽은 시각은 지역 시간대와 오프셋이 같아도 고정 오프셋으로 남는다.
- 서머타임 경계의 지역 시각: 없는 시각은 빈틈만큼 뒤로(`02:30` → `03:30`), 두 번 있는 시각은 이른 순간이다. 0.1은 시간대에 따라 달랐다.
- `TZ`에 POSIX 시간대 문자열(`KST-9`, `EST5EDT,M3.2.0,M11.1.0`)을 쓸 수 있고, 상대 시간대 이름은 `TZDIR`(없으면 `/usr/share/zoneinfo`)에서 찾는다.
- `time.parse`(형식 없음)가 RFC 3339를 엄격하게 읽는다: 한 자리 시, 쉼표 소수, `+24:00`, 10자리 넘는 소수는 이제 에러다. 시간 파싱 에러 문구가 짧아졌다(`month out of range`, `expected a month name like Oct`).
- `json.parse`가 같은 키가 두 번 나오는 객체와 짝 없는 서로게이트(`"\ud800"`)를 거절한다(0.1은 뒤의 키가 이기고 U+FFFD로 바꿨다). 에러는 문제가 있는 글자를 가리키고 문구가 짧아졌다(`unexpected character ']'`, `number out of range`).
- `json.stringify`가 U+2028·U+2029를 이스케이프하지 않는다(필요한 것만: `"`, `\`, U+0020 미만). 아주 큰 `indent`는 E0806(글자 한도)이다.
- UTF-8이 아닌 바이트를 읽을 때 잘못된 부분 수열마다 U+FFFD 하나로 바꾼다(유니코드 권장 방식). 잘린 다중 바이트 문자 `\xE2\x82`는 이제 U+FFFD 하나다.
- 정규식 문법이 Rust `regex` 크레이트의 것이다(RE2 계열). `\d`·`\w`·`\s`·`\b`가 유니코드를 알아보고(`\d`가 `٣`에, `\w`가 `가`에 맞음),
  `(?x)`·`[a&&b]`·`\<`를 받으며, 같은 이름의 그룹과 숫자로 시작하는 그룹 이름은 에러다. 잘못된 패턴의 문구는 ``invalid pattern `a(`: unclosed group``.
  `a**`·`a{1001}`처럼 0.1이 거절하던 반복을 받고, 중첩은 250단계까지다(0.1은 1,000).
  `regex.split`은 문자열 끝의 빈 매치로 자르지 않는다(`split("abc", "")` → `["a","b","c"]`). 0.1의 Go 정규식 문법에서 바뀐 점이다.
- `http` 클라이언트:
  - 에러 문구가 이유만 남았다: ``cannot request `url`: connection refused``(0.1은 `Get "url": dial tcp …: connect: connection refused`).
    잘못된 URL은 `invalid URL: …`, 스킴은 ``unsupported scheme `ftp` ``, 리다이렉트 초과는 `too many redirects (limit 10)`, 프록시에서 난 것은
    ``proxy `host:port`: …``, 리다이렉트 뒤의 요청에서 난 것은 ``redirected to `url`: …``.
  - 인증서 오류 문구: `unknown certificate authority`, ``certificate is not valid for `host` ``, `certificate has expired (valid until <시각>)`.
    검사 순서가 사슬 다음 이름이라, 믿지 않는 발급자가 다른 이름에 낸 인증서는 이제 `unknown certificate authority`다.
  - `User-Agent`가 `fung/<버전>`이다(0.1은 `Go-http-client/1.1`). `CONNECT`에도 붙는다.
  - 사용자 헤더가 이름순이 아니라 맵 순서대로 나간다. 사용자 `Host`가 자동 값을 대신한다(0.1은 버렸다). 리다이렉트가 다른 호스트로 가면 버린다.
  - `timeout` 하나가 연결·TLS 핸드셰이크·`CONNECT`·응답을 모두 덮는다. 0.1의 연결 30초·핸드셰이크 10초·`CONNECT` 60초 한도가 없어졌다.
  - 리다이렉트 응답의 본문을 읽지 않고 바로 다음 요청으로 간다(0.1은 2 KB까지 읽느라 멈춘 본문에서 시간 초과가 났다).
    리다이렉트는 10번까지 따른다(0.1은 9번, 10번째 요청의 리다이렉트에서 멈췄다).
- `http.serve`:
  - 서버가 스스로 내는 답의 본문이 `<코드> <이유 문구>`다(`500 Internal Server Error` 등; 0.1은 `handler failed`·`400 Bad Request: missing required Host header` 꼴).
    상태 줄에는 표준 이유 문구만 쓰고, 표에 없는 코드(`599`)는 빈 이유 문구다(0.1은 `status code 599`).
  - 이유 문구가 IANA 레지스트리의 것이다: 413 `Content Too Large`, 414 `URI Too Long`, 416 `Range Not Satisfiable`, 422 `Unprocessable Content`, 418은 빈 문구.
  - 요청 머리 한도가 1 MiB다(0.1은 1 MiB + 4 KiB). 핸들러가 다른 타입을 돌려줄 때의 표준 에러 줄은 `http: bad handler response: …`.
- 실수의 지수를 채우지 않는다: `1e-7`, `1e-5`(0.1은 `1e-07`, `1e-05`).
- 새로 만드는 파일의 권한이 모두 0666에서 umask를 뺀 것이다(`fs.write`·`fs.append`·`fs.open`의 `'write`·`'append`; 0.1은 `fs.open`의 `'write`만 0666이고 나머지는 0644).
- 범위로 자를 때의 인덱스 에러(E0507)가 쓴 인덱스를 보인다: `[1,2,3][-5..]`은 `index -5`(0.1은 `-2`), `[1,2,3][0..=3]`은 `index 3`(0.1은 `4`),
  아주 큰 끝도 감기지 않는다. 기간 뺄셈 `a - b`는 결과가 범위 안이면 `b`가 최소 기간이어도 값이다.
- 도구:
  - `fung test`가 읽지 못한 파일(`FAIL path: <load>`, 종료 코드 2)과 종료 핸들러의 실패(`FAIL path: <exit handler>`)를 실패로 센다. 0.1은 찍기만 했다.
  - `fung test`와 `fung fmt`가 같은 파일을 고른다: `.`으로 시작하는 디렉터리는 둘 다 건너뛰고(0.1의 `test`는 들어갔다), 인자로 준 링크는 둘 다 따라간다.
  - 파일 에러 문구가 `fs` 모듈과 같다: ``error: cannot read `p`: no such file or directory``(0.1은 `stat p: …`, `cannot read p: open p: …`). 표준 입력은 `<stdin>`.
  - `fung check -e CODE` 뒤의 인자는 사용 오류다(0.1은 무시했다).
  - `fung fmt`의 정렬이 화면 칸 수를 센다: 한글·한자 같은 넓은 글자는 2칸, 결합 문자는 0칸. 0.1은 글자 수를 셌다.
  - REPL: 에러가 그 코드를 정의한 입력의 줄을 보이고(0.1은 지금 입력의 같은 줄), 태스크 실패 보고에도 발췌가 있다. 세션의 전역 개수에
    한도가 없다(0.1은 65536개를 넘기면 그 뒤 모든 입력이 실패했다). `/dev/null` 같은 터미널이 아닌 장치는 줄 편집 없이 읽고 기록 파일을 만들지 않는다.
  - REPL 줄 편집기: 커서가 화면 칸 수로 움직인다(한글은 2칸; 0.1은 글자마다 1칸이라 한글이 든 줄에서 어긋났다). 잘못된 UTF-8 입력은 부분마다 U+FFFD 하나(0.1은 바이트마다).
- `go install github.com/tunarider/fung/cmd/fung@latest`로 설치하는 길이 없어졌다. 설치 스크립트를 쓰거나,
  Rust 1.94 이상으로 저장소에서 `cargo build --release --locked`한다.

### 더함

- `agent` 라이브러리: `agent(v)`·`swap(a, f)`·`deref(a)`. 상태를 쥔 배경 태스크에 채널로 요청을 보내는 패턴(Elixir의 `Agent`)을 감쌌다.
  요청을 하나씩 처리하므로 자물쇠가 필요 없다. 핸들의 마지막 사본이 사라지면 에이전트도 끝나고, 프로그램이 끝나는 것을 막지 않으며,
  `at_exit` 핸들러에서도 쓸 수 있다.
- `conc` 모듈: `conc.pmap(xs, f, limit = 0)`·`conc.fan_in(chs)`·`conc.retry(f, times = 3, delay = 0ms)`. 동작은 0.1의 prelude 함수와 같다.

### 배포

- Linux 바이너리는 0.1처럼 정적 링크라 배포판·glibc 버전과 상관없이 돈다(이제 musl로 빌드한다). 대상 플랫폼, 묶음 이름, `SHA256SUMS`,
  빌드 증명, `install.sh`는 그대로다.

### 고침

- `fung fmt`가 `fn` 타입 표기가 있는 매개변수(`fn f(h: fn, x = 1):`) 뒤의 블록 몸통을 내어쓰던 문제. 타입 이름 `fn`을 함수 머리로 읽었다.
- `fung fmt`가 BOM(U+FEFF)으로 시작하는 파일을 "formatter changed the code"로 거절하던 문제. 이제 BOM을 떼고 포맷한다.
- `fung fmt --check -`가 `--check`를 무시하고 포맷 결과를 찍던 문제. 이제 바뀔 게 있으면 `<stdin>`을 찍고 종료 코드 1이다.
- 이름에 결합 문자(힌디어 모음 기호 등)가 이어지지 못하던 문제. 이름은 이제 UAX #31을 따른다(`XID_Start` 또는 `_`로 시작해 `XID_Continue`가 이어짐).
  `हिंदी = 1`이 0.1에서는 문법 에러였다. 로마 숫자 같은 글자 숫자(`ⅻ`)도 이름에 쓸 수 있다.
- 이름은 NFC로 정규화한다: 조합형 `é`와 분해형 `é`(`e` + U+0301)가 같은 이름이다. `sym(s)`도 같다. 문자열 값은 그대로다.

### 성능

- 계산 위주의 작업은 0.1.0보다 빠르다. `bench/`의 스크립트를 한 기계(Apple M2 Max, macOS)에서 잰 중앙값으로, 0.1.0에 견준 걸린 시간은
  `fib(30)` 재귀 0.39배, 300만 번 산술 반복 0.38배, 키 20만 개 맵 만들고 조회 0.73배다. 숫자 50만 개를 문자열로 잇기는 1.14배로 느려졌다.
  표(0.1.0과 0.2.0)와 재는 방법은 `bench/README.md`.
- 태스크와 에이전트는 0.1보다 무겁다. 태스크마다 운영체제 스레드를 하나 쓰기 때문이다: 태스크 하나 만들기 약 55 µs(0.1의 약 25배),
  채널 주고받기 약 2.5 µs(약 2~3배), 에이전트 `swap` 한 번 약 7 µs(0.1 atom은 약 0.4 µs). 같은 `bench/` 측정에서 태스크 2,000개 띄우고 기다리기는
  0.1.0의 12.6배, 버퍼 없는 채널로 값 10만 개 주고받기는 2.2배 시간이 걸린다. 살아 있는 태스크·에이전트 수는 운영체제 스레드 한도가 천장이다.
- `send`는 E0705 검사를 위해 보내는 값을 훑으므로 비용이 값의 크기에 비례한다(정수 100만 개 리스트 한 번에 약 0.75 ms).

### 알려진 차이

0.1(Go)과 다르게 동작하지만 고치지 않고 남긴 것 중, 스크립트가 걸려 넘어질 만한 것만 적는다. HTTP의 세부는 매뉴얼 `builtins.html`의 `http` 절에 있다.

- UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD 하나로 바뀐다: 파일 내용(`fs.read`·`read_lines`·`read_line`), 프로세스 출력, 환경 변수 값, HTTP 본문,
  경로 이름. 그래서 바이너리 데이터는 그대로 받을 수 없다. 0.1은 원래 바이트를 지켰다.
- 띄운 형제 태스크가 시작하는 순서와, 아무도 꺼내지 않은 태스크 실패가 보고되는 시점·순서가 0.1과 다를 수 있다.
  순서에 기대는 프로그램과 `test` 블록은 두 구현에서 결과가 다를 수 있다.
- `time.now()`에 단조 시계가 없다. 두 `now()` 사이에 시스템 시각이 바뀌면(NTP 보정 등) 그 차이로 잰 경과 시간이 틀릴 수 있다.
- Linux에서 `SSL_CERT_FILE`·`SSL_CERT_DIR` 중 하나라도 있으면 그 위치의 인증서만 읽는다. 0.1은 기본 디렉터리(`/etc/ssl/certs` 등)도 함께 읽었다.
- 아주 큰 정규식(유니코드 클래스를 수천 번 반복하는 등)은 0.1이 받던 것도 `pattern too large`(E0515)로 실패할 수 있다.
- fung은 프로그램을 4 GiB, 태스크를 1 GiB 스택을 예약한 스레드에서 돌린다(예약만 하고 메모리는 쓰는 만큼만 차지한다). 메모리와 스왑이 작거나
  가상 메모리를 엄격하게 제한한 환경(`ulimit -v`, 엄격한 overcommit)에서 이 예약이 거부되면 더 작은 스택(최소 64 MiB)으로 돈다. 그런 환경에서는
  아주 깊은 재귀(fung 함수를 부르는 `map`·`fold`를 수만 단계 겹치기, 깊이 중첩된 값 출력)가 더 일찍 실패한다. 0.1(Go)은 스택이 필요한 만큼 자랐다.

## [0.1.0] - 2026-10-14

첫 공개 릴리스. 언어의 기본 기능, 표준 모듈, 개발 도구를 갖춘 알파다.

### 언어

- 값: 정수·실수·문자열(보간 `${}`)·불리언·`nil`·심볼·기간(`5s`, `2h`)·시간, 리스트·맵·튜플·범위, 태그(`Ok(x)`, `Err(e)`).
- 블록은 `:`로 열고 `;;`로 닫는다. 들여쓰기는 의미가 없다.
- 함수: `fn` 짧은 형·블록 형, 람다 `x => …`, 위치·이름 인자, 기본값, 타입 표기 검사, 꼬리 호출 최적화.
  클로저는 만들어질 때 보이는 변수를 복사한다.
- 패턴: `match`, 구조 분해 대입, guard, `...rest`, 매개변수 패턴.
- 에러: `fail`, 결과값 연산자 `!`·`?`·`??`, 소스 발췌와 호출 경로가 붙은 에러 출력, 에러 번호 카탈로그.
- 파이프 `|>`, 자리표시 `_`, `for`(→ `map`), `in`, `type` 별칭.
- 모듈: `import`, 모듈별 전역.
- 동시성: `spawn`·`await`·`cancel`, 채널, `atom`, `sleep`. 교착 감지.
- 끝내 아무도 `await`하지 않은 태스크의 실패는 보고되고 종료 코드가 1이 된다. 보고 여부는 실행 순서와 무관하다.
- 리스트와 맵은 영속 자료구조다. 누적과 꼬리 재귀가 선형이다.

### 표준 모듈

- prelude: 컬렉션·문자열·변환·결과값·입출력·프로세스 함수.
- `fs`, `path`, `cli`, `time`, `json`, `regex`, `http`(클라이언트와 `serve`).
- prelude와 표준 모듈을 합쳐 내장 함수 128개.

### 도구

- `fung <script>`, `fung -e`, REPL(줄 편집, 기록 `~/.fung_history`).
- `fung test`(같은 파일의 `test` 블록과 `assert`), `fung check`(경고 W0101~W0104), `fung fmt [--check]`, `fung dump`.
- 셰뱅 `#!/usr/bin/env fung`.

### 배포

- Linux·macOS(amd64, arm64) 정적 바이너리, `SHA256SUMS`, 빌드 증명.
- 설치 스크립트 `install.sh`, `go install github.com/tunarider/fung/cmd/fung@latest`.
- HTML 매뉴얼(`docs/`, GitHub Pages).

[Unreleased]: https://github.com/tunarider/fung/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tunarider/fung/releases/tag/v0.1.0
