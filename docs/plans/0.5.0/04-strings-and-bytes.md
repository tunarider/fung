# L4. 문자열이 바이트를 품지 못해 비UTF-8 입력이 U+FFFD로 바뀐다

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

fung 문자열은 UTF-8이다(`Value::Str(Rc<String>)`, `crates/fung-vm/src/value.rs`). 바깥에서 들어오는 바이트는 모두 문자열로 풀리고, 잘못된 부분마다 U+FFFD 하나가 된다.
원래 바이트는 남지 않는다. 그래서 fung으로 읽고 그대로 쓰기만 해도 파일이 바뀐다. 릴리스 바이너리(2026-10-26, macOS):

```
$ printf 'a\xffb\xe2\x82c\n' > bad.txt                       # 7바이트
$ fung -e 'import fs
s = fs.read("bad.txt")!
print(len(s))
fs.write("out.txt", s)!'
6
$ od -c out.txt
0000000    a 357 277 275   b 357 277 275   c  \n                # 12바이트
$ fung -e 'print(exec(["printf", "a\\377b"])!)'
{out: "a�b", err: "", code: 0}
$ X=$(printf 'v\xff') fung -e 'print(env("X"))'
v�
$ fung -e 'print(args)' "$(printf 'q\xff')"
["q�"]
$ printf 'x\xffy\n' | fung -e 'print(read_line()!)'
x�y
```

같은 일이 HTTP 바이너리 본문(06 "HTTP"가 "약속 밖"으로 적음), `fs.list`·`walk`·`glob`의 파일 이름, 시간대 약어, 비UTF-8 디렉터리 아래 모듈 경로에도 일어난다.
fung은 "쉘 스크립트로 짜던 소소한 작업"을 하는 언어다(background "출발점"). 명령 출력·파일·파일 이름을 그대로 넘기는 일이 핵심인데 그것이 말없이 망가진다.
이것은 원칙 1(조용히 넘어가는 동작은 에러로)에 어긋난다. 잘못된 바이트를 바꾸고도 에러도 경고도 없다.
리터럴로 바이트를 쓸 방법도 없다(`"\xff"` → `E0103 unknown escape \x in string`, 01 "문자열"의 이스케이프 목록).

## 지금 동작과 원인

- 풀기 함수 하나: `fung_front::source::decode_lossy`(`crates/fung-front/src/source.rs`, `String::from_utf8_lossy`). "가장 긴 잘못된 부분마다 U+FFFD 하나"(유니코드 권고)다.
- 부르는 곳(2026-10-26 `grep`):
  - 파일: `native/fs.rs`의 `read`(207행), `str_list`(247행, `list`·`walk`·`glob`의 이름), 임시 디렉터리(750행), 에러 메시지의 경로(`from_utf8_lossy`, 335·766행).
  - 줄 읽기: `prelude/io.rs`(99·142행, `read_line`·`read_lines`의 파일 쪽), `vm.rs`의 `stdin_line`(183행, 표준 입력).
  - 프로세스: `native/process.rs`의 `env_fn`(88·95·96행), `exec`의 `out`·`err`(603·604행), `cwd` 인자(769행).
  - 소켓·HTTP: `native/net.rs`의 `text`(1130행)와 `take_body`(1296행 근처). 바이트로 쌓았다가 한 번에 푼다(R4d 노트 2절).
  - 시간대: `native/tz.rs`(61·232·267·541·700·744·751행), 앞단 로더: `fung-front/src/loader.rs`(453·468·595행, `to_string_lossy`).
  - `args`: `crates/fung-cli/src/main.rs` 68행이 `args_os`로 바이트를 받지만 결과는 같은 풀기를 거친다(위 재현).
- 경로는 안에서는 이미 바이트(`&[u8]`, `os_path`)로 다룬다(R4a 노트: Go `filepath`를 바이트 단위로 옮김). 바뀌는 곳은 fung 값으로 내놓는 순간뿐이다.
  `native/glob.rs`의 `decode_rune`은 Go 규칙("잘못된 바이트 하나 = 길이 1의 U+FFFD 한 글자")으로 바이트 위에서 맞춘다.
- 바이트를 다루는 네이티브는 `_native.net`의 `byte_len`·`utf8_bytes`·`utf8_string`(정수 리스트 ↔ 문자열)뿐이다. R4d 노트 6절이 "모자란 것은 바이트"라고 적었다.
- 글자 연산: `len`은 코드 포인트 수(`value/index.rs` 41행, ASCII 빠른 길), `s[i]`·`chars`도 코드 포인트다. 앞단은 소스도 같은 풀기로 읽는다(`source.rs` 59행).

## 고려할 점

- **값이 둘인가 하나인가.** 글(text)과 바이트를 타입으로 나누면 경계가 분명하지만, API가 두 벌이 되고(원칙 6) 바깥 값을 받을 때마다 "어느 쪽인가"를 골라야 한다.
- **OS 문자열.** 파일 이름·환경 변수·인자는 Unix에서 그냥 바이트다. 바이트 타입을 따로 둬도 이것들을 무엇으로 줄지는 따로 정해야 한다(Rust는 `OsString`이라는 세 번째 타입).
- **글자 연산의 뜻.** 잘못된 바이트가 든 문자열의 `len`·`s[i]`·`upper`·정규식·`json.stringify`가 무엇을 해야 하나. 조용히 U+FFFD로 보면 원칙 1에 다시 걸린다.
- **보이는 모양.** `print`는 바이트를 그대로 써야 `cat`처럼 동작한다. `str`·REPL·에러 메시지의 값 표시는 터미널을 망치지 않게 `\xff`처럼 보여야 하고, 그 모양은 다시 리터럴로 읽혀야 한다.
- **비교와 정렬.** UTF-8은 바이트 순서가 코드 포인트 순서와 같다. 바이트로 비교해도 유효한 글에서는 결과가 지금과 같다.
- **비용.** `Rc<String>`을 `&str`로 받는 네이티브가 많다(`nat::need_str`). 표현을 바꾸면 모든 문자열 네이티브와 `regex`(바이트용 `regex::bytes`가 같은 크레이트에 있음)를 손본다.
- 다른 언어: Go·Lua·Perl은 문자열이 바이트이고 글자 연산이 UTF-8로 해석한다(Go는 잘못된 바이트를 U+FFFD 한 글자로 센다). Python 3은 `str`·`bytes`를 나누고 OS 문자열은 surrogateescape(PEP 383)로 왕복시킨다.
  Rust는 `String`·`Vec<u8>`·`OsString` 셋. Elixir는 문자열이 UTF-8 바이너리이고 유효성을 따로 검사한다.

## 선택지

(가) **바이트 타입을 따로 둔다.** `bytes` 값, `fs.read_bytes`·`exec(…, bytes: true)`·`http`의 바이트 본문, `decode(b)`(엄격, `Err 'invalid`)·`encode(s)`.
  글 쪽 함수는 잘못된 UTF-8에 `Err`를 낸다. 장점: 글은 늘 유효한 UTF-8이라 글자 연산이 단순하다. 단점: 바깥을 읽는 함수가 거의 다 두 벌, 파일 이름·`env`·`args`는 여전히 답이 없다(세 번째 타입이나 (다)가 필요).
(나) **문자열을 바이트 열로 바꾼다(Go 방식).** 문자열은 바이트이고 "보통은 UTF-8"이다. 바깥 값은 바이트 그대로 들어오고 그대로 나간다(손실 없음, API 그대로).
  글자 연산은 UTF-8로 해석하고 잘못된 바이트 하나를 한 글자로 센다(지금 `glob`의 규칙을 언어 전체로). 표시는 `\xHH`, 리터럴에 `\xHH` 이스케이프를 더한다.
  유니코드 뜻이 꼭 필요한 곳(`json.stringify`, 대소문자, NFC 같은 것)은 잘못된 바이트에 에러를 낸다. 장점: 타입 하나, 규칙 하나(원칙 6·7), 쉘 대용에 맞음. 단점: 표현과 거의 모든 문자열 네이티브를 바꾸는 큰 일.
(다) **UTF-8을 지키고 잘못된 바이트를 손실 없이 이스케이프한다(surrogateescape).** 잘못된 바이트를 U+DC80~U+DCFF로 바꾸고 내보낼 때 되돌린다.
  Rust `String`은 홀로 선 서로게이트를 품지 못해 결국 자체 표현(WTF-8 비슷한 것)이 필요하고, 사용 영역(U+E000~)에 넣으면 진짜 그 글자와 섞인다. Python에서도 내보낼 때 `surrogates not allowed`로 늦게 터진다.

추천: **(나)**. 손실이 없고, 값 종류와 API를 늘리지 않으며, 규칙이 "문자열은 바이트, 글자 연산은 UTF-8로 읽는다" 하나다.
(가)는 OS 문자열 문제를 풀지 못해 결국 (나)나 (다)를 함께 들여야 하고, (다)는 Rust에서 (나)보다 복잡한 내부 표현을 요구하면서 실패를 뒤로 미룬다.
엄격함은 "유니코드 뜻이 필요한 연산이 잘못된 바이트를 만나면 에러"로 지킨다.

## 정할 것

1. 잘못된 바이트가 든 문자열의 `len`·`s[i]`·`chars`: 잘못된 바이트 하나를 한 글자로(그 글자는 그 바이트 하나짜리 문자열) 세는가, 에러인가. 추천: 한 글자로 센다(Go·지금 `glob`과 같음, 자르고 붙여도 바이트가 보존됨).
2. 리터럴 `\xHH`: 더하는가, 범위는. 추천: `"…"`에만 `\x00`~`\xff`를 더하고, `\xHH`로 만든 잘못된 UTF-8도 허용한다. `str`·REPL 표시는 같은 모양으로 낸다.
3. 유니코드 뜻이 필요한 연산의 목록과 실패 모양. 추천: `json.stringify`·`upper`·`lower`·NFC를 쓰는 곳은 새 에러 번호(카탈로그에서 할당)로 실패, 비교·해시·`+`·`split`·`replace`·`contains`·`regex`는 바이트로 동작.
4. 정규식: `regex::bytes::Regex`로 바꾸는가. 추천: 바꾼다(유니코드 모드 유지, `.`는 잘못된 바이트에 맞지 않는 크레이트 동작을 그대로 02에 적음; 구현 때 확인).
5. 바이트를 다루는 내장 함수: `_native.net`의 `byte_len`·`utf8_bytes`·`utf8_string`을 prelude로 올리고 이름을 정하는가(예: `byte_len`·`bytes`·`from_bytes`). 추천: 올린다. 그리고 `valid_utf8(s)`를 더한다.
6. 소스 파일이 UTF-8이 아니면. 추천: 소스는 지금처럼 텍스트로 읽되 잘못된 바이트를 컴파일 에러로 한다(소스에 U+FFFD를 말없이 넣지 않음, 원칙 1).
7. 표현: `Rc<String>`을 `Rc<[u8]>`·`Rc<Vec<u8>>` 중 무엇으로, "유효한 UTF-8인가"·"ASCII인가"를 기억하는 칸을 두는가. 추천: 만들 때 한 번 재서 기억한다(`len`·`s[i]`의 빠른 길과 3번의 검사가 O(1)).

## 할 일

1. 시험 먼저: 위 재현을 골든 테스트(`tests/lang/`)와 `crates/fung-cli/tests/expected/`의 기대 출력으로 만든다. 읽고 쓰기 왕복(`fs.read`→`fs.write`, `exec`의 `out`, `read_line`, `env`, `args`)이 바이트를 보존하는지,
   표시가 `\xHH`인지, 3번의 연산이 에러인지. 비UTF-8 파일 이름은 macOS(APFS)가 만들지 못하므로(`mkdir: Illegal byte sequence`, 2026-10-26 확인) Linux CI에서만 도는 시험과 단위 테스트로 둔다.
2. `Value::Str`의 표현을 바꾸고 `nat::need_str` 계열을 바이트로 받게 한다. `decode_lossy`를 부르는 곳(위 목록)을 모두 바이트 그대로 넘기게 바꾸고, 남는 `decode_lossy`는 에러 메시지용으로만 남기거나 지운다.
3. 글자 연산(`value/index.rs`, `value/arith.rs`의 `check_chars`, 문자열 prelude), 표시(`value/format.rs`), 렉서의 `\xHH`, `regex`, `json`, `http`의 본문(`extras/http.fung`의 `utf8_*` 사용처)을 바꾼다.
4. 설계 문서: 01("문자열" 이스케이프, "글자 수준 규칙"), 02("문자열"), 06(`fs`·프로세스·HTTP의 "약속 밖"에서 바이너리 본문 삭제), 09(값 표현).
   open-questions "알려진 한계"의 "문자열과 바이트" 절을 지우고 닫은 기록을 `docs/notes/open-questions-closed.md`로.
5. 매뉴얼: 배우기의 문자열 장, 레퍼런스(문법·내장 함수 표), VM 부록의 값 표현을 고치고 `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`로 다시 만들어 diff를 본다.
6. `spec/builtins.txt`·`spec/errors.txt`를 `FUNG_UPDATE_SPEC=1 cargo test spec_file`로 갱신. CHANGELOG `[Unreleased]`의 "바뀜(호환 깨짐)"에 적는다.
7. 성능: 문자열이 많은 작업(`bench/compare/`의 텍스트·파일 작업)을 `scripts/bench.sh`로 전후 비교해 `docs/notes/bench-languages.md`에 남긴다. 학습용 설명을 `docs/notes/`에 쓴다.

## 끝의 기준

- 위 재현의 `out.txt`가 `bad.txt`와 바이트 단위로 같고, `exec`·`env`·`args`·`read_line`의 값을 그대로 내보내면 원래 바이트가 나온다.
- `str`·REPL이 잘못된 바이트를 `\xff`로 보이고 그 글이 리터럴로 다시 읽힌다. 3번의 연산은 에러 번호로 실패한다.
- 코드에 `from_utf8_lossy`·`to_string_lossy`·`decode_lossy`가 값을 만드는 데 쓰이지 않는다(에러 메시지·시험 도우미만).
- open-questions에 "문자열과 바이트" 한계가 없고, 커밋 전 검사(CLAUDE.md)가 Linux·macOS에서 통과한다.
