# Rust 이식 6c단계: 이식 잔가지 걷어내기 — 설계

2026-10-17. 앞 단계는 `2026-10-17-rust-go-removal-6b-design.md`(6b, main `202ba47`). 할 일 목록의 원본은
`docs/design/open-questions.md`의 "Rust 이전" 절: "6c 할 일: 이식 잔가지 걷어내기", 그 아래 "6c: 이름과 주석", "5단계(도구)에서 알고 남긴 차이와 잔가지"의 "Go 흉내".

## 목표

저장소가 **처음부터 Rust로 짠 것처럼** 보이게 한다. Go 구현은 이제 제약일 뿐이고 사용자가 없으므로 호환성은 고려하지 않는다.
"Go가 그래서"는 근거가 될 수 없고, 각 동작은 그 자체로 옳은지 판단한다.

- Go 구현의 우연을 바이트 단위로 따라 하던 동작을 fung의 규칙으로 바꾸거나 단순하게 한다.
- Go를 가리키는 코드 이름·주석, extras·스크립트·설정의 주석을 걷어낸다.
- 현재 문서에서 Go 흉내 서술과 이름의 Go 유래를 걷어내고, 언어 비교는 영향받은 언어 전반의 설명으로 다시 쓴다.

## 결정 사항 (2026-10-17 사용자)

| 항목 | 결정 |
|---|---|
| 나누기 | **영역별 하위 단계**(아래 1절). 각 단계가 그 영역의 동작·이름·주석·문서를 함께 정리하고, 마지막 단계가 남은 것을 훑는다. "동작 / 이름·주석" 두 단계로 나누면 동작을 바꾼 순간 그 동작을 설명하던 주석이 틀려지고 같은 파일을 두 번 열게 되어 택하지 않았다 |
| 머지 | 이 브랜치(`claude/rust-port-stage-6c-4ec418`)에 쭉 쌓고 **6c 전체가 끝난 뒤 한 번** 사용자 검토 → main 스쿼시 머지. 중간 push·CI 기다림 없음 |
| 검사 | **전체 커밋 전 검사는 맨 끝에 한 번**(아래 2절). 그 뒤 실패를 한꺼번에 고친다. 6c 동안 `CLAUDE.md`의 "커밋 전 검사"를 이렇게 완화한다 |
| 이름의 유래 | "fun + **g**o"를 지우고 한국어 "펑"만 남긴다(README, `background.md`) |
| 언어 비교 | "Go와 닮은 점"을 말하는 것 자체는 괜찮다. 다만 fung에는 여러 언어의 특징이 들어갔으므로(Go·Python의 표면 문법, Lisp의 균일 트리, Erlang/Elixir의 태스크·실패 모델 등) "겉은 Go, 속은 Lisp"와 `background.md`의 언어 표를 **영향받은 언어 전반**의 설명으로 다시 쓴다. 다른 언어와 나란히 놓는 비교(05의 `select`·`context`, 01의 리터럴)는 남긴다 |
| 대체 동작을 정하는 때 | 각 하위 단계를 **시작할 때** 그 영역의 흉내 동작만 하나씩 묻는다(선택지 + 추천 + 이유). 정한 것은 이 스펙의 해당 절에 결정표로 덧붙이고 커밋한다 |

## 원칙

1. **흉내였더라도 그 자체로 옳은 동작은 남기고 이름과 주석만 고친다.** 예: 경로의 `..` 정리, 닫힌 표준 출력에서 신호로 죽기, 필요할 때만 표준 입력 읽기, 포매터의 원래 바이트 보존.
   바꾸는 일 자체가 목표가 아니다.
2. **대체 동작이 Rust 표준 동작의 노출이어서도 안 된다.** `00-concept.md`의 "구현 언어의 동작 특성이 새어 나오지 않게"는 Rust에도 적용된다.
   예: Go 에러 문구 대신 Rust `io::Error`의 `No such file or directory (os error 2)`를 그대로 내지 않는다. 대체 동작은 **fung의 규칙**으로 정하고 설계 문서에 적는다.
3. **바른 동작을 먼저 정한다.** `docs/design`과 `tests/lang`(필요하면 단위 테스트)에 정한 뒤 코드를 바꾸고, 그 영역의 기대 출력을 bless하고 diff를 검토한다.
4. 사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`에 적는다. 문법·내장 함수·내부 구조를 바꾸면 매뉴얼도 같이 고친다.
5. 해결한 open-questions 항목은 6b처럼 "→ 해결 (6c, 하위 단계)"으로 표시한다.

## 1. 하위 단계

| 단계 | 영역 | 대상 (open-questions 항목, 주요 파일) |
|---|---|---|
| 6c-1 | 기반 | 유니코드 판정 표(`spec/unicode.txt`, `fung-front/src/unicode.rs`)와 식별자 규칙, 대소문자 단순 매핑(`prelude/strings.rs`), 런타임 한도 문구(`makeslice`·`makechan`·`strings: Repeat`; `nat.rs`·`prelude.rs`·`conc.rs`), 표준 입력 4 KiB 단위(`vm/stdin.rs`), 16진 실수 파싱(`prelude/convert.rs`), `exec format error` 사전 검사(`native/process.rs`), 채널 순회(`native/nat.rs`의 `check_items`·`each_until`·`items`) |
| 6c-2 | 파일 시스템·OS | errno 문구 표(`native/oserr.rs`)와 `fs`의 단계 옮김(`mkdir_all`·`remove`·`remove_all`·`rename`·`make_temp_dir`, NUL 경로), 경로 규칙(`native/path.rs`), `list`·`walk` 순서와 `walk` 에러, `glob`(`native/glob.rs`), 쓰기 버퍼 `GoWriter`(`native/file.rs`), 핸들 표시의 따옴표 형식(`value/format.rs`) |
| 6c-3 | 시간·JSON | 시간대 탐색(`native/tz.rs`), 시각 값 표현과 넘침 규칙(`value/time.rs`), 서식·파싱·에러 문구(`native/timefmt.rs`), JSON 읽기(토큰 상태 기계·읽기 버퍼 흉내·에러 문구·깊이 한도; `native/json.rs`), JSON 이스케이프와 큰 들여쓰기 문구 |
| 6c-4 | 정규식 | 유니코드 표(`spec/regex_unicode.txt`), `native/regex_go.rs`(이름, 에러 문구, 에러 선택), 치환·`extract`·빈 매치·`Split`·캐시 규칙(`native/regex.rs`), 고정 회귀 데이터 `regex_go_fixtures.rs`·`regex_go_tests.rs`의 이름과 정리. 정규식 **문법** 자체의 재판단은 범위 밖 |
| 6c-5 | 네트워크·HTTP | `net` 문구와 주소 규칙(`native/net.rs`·`sock.rs`), TLS 문구와 검사 순서(`native/tls.rs`의 `GoOrder`), SIGPIPE 세부(`sigpipe.rs`), HTTP 클라이언트·리다이렉트·프록시·서버(`extras/http.fung`; `User-Agent`, 리다이렉트 본문 버리기, `Date` 형식, 상태 문구 등), `tls_test_server.rs`를 테스트 지원 크레이트로 옮길지 |
| 6c-6 | 도구 | `fung test`의 중복 제거·세기·종료 코드, 경로 순회 규칙(`fung-cli/src/paths.rs`), `check -e` 나머지 인자, `/dev/stdin` 이름, 포매터 정렬 너비, REPL 잔가지(발췌, 전역 한도 E0806, `/dev/null`, 줄 끝 처리), W0102 순서 |
| 6c-7 | 마무리 | 앞 단계에서 남은 이름·주석(`crates` 전체, `extras` 주석, `deny.toml`, `scripts/build-release.sh`, `scripts/logo/render.html`), 현재 문서(설계 00~09·`background.md`·README·매뉴얼·`CLAUDE.md`·open-questions), 이름의 유래와 언어 비교 다시 쓰기, `06-native-and-stdlib.md` 파일 이름 바꾸기와 링크, `bench/README.md`의 측정 열, 지킴이 테스트(아래 3절), 학습 노트 `docs/notes/R6c-cleanup.md` |

순서는 위와 같다. 6c-1의 유니코드 표 결정이 6c-4(정규식 표)·6c-6(포매터 너비)에 닿고, 6c-2의 에러 문구 규칙이 6c-5·6c-6(`os_error_text`)에서 다시 쓰이기 때문이다.

각 하위 단계의 흐름:
1. 그 영역의 코드와 기대 출력을 읽고, 흉내 동작마다 선택지 + 추천 + 이유로 사용자에게 묻는다.
2. 결정표를 이 스펙의 해당 절(아래 4절)에 덧붙여 커밋한다.
3. 짧은 계획을 세운다(`docs/plans/2026-10-17-rust-port-cleanup-6c-<n>.md`).
4. 설계 문서와 `tests/lang`을 먼저 고치고, TDD로 구현한다.
5. 그 영역의 기대 출력을 bless하고 diff를 검토한 뒤 커밋을 나눠 둔다(마지막 검토 때 커밋 단위로 볼 수 있게).
6. 손댄 파일의 이름·주석·문서에서 Go를 걷어낸다.

## 2. 검증

- **단계 중**: TDD의 실패 확인과 통과 확인은 고친 테스트만 돌린다. bless는 그 영역의 테스트만 돌린다
  (`FUNG_EXPECTED=bless cargo test -p fung-cli --test <이름>`, fung-vm은 `runs_match_the_expected_output`). 기대 출력 3,570개를 맨 끝에 한꺼번에 bless하면
  의도한 변경과 사고를 가려낼 수 없으므로, bless는 영역마다 한다.
- **맨 끝에 한 번**: 커밋 전 검사 전체(`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`와 `--release`,
  `cargo test --locked --no-fail-fast`와 `--release`, `cargo deny check`, `cargo audit`). 실패는 한꺼번에 고치고, 고친 뒤 전체를 다시 돌린다.
- 위험: 중간 커밋은 전체 검사를 거치지 않아 다른 영역을 깨뜨린 것이 맨 끝에 드러날 수 있다. 스쿼시 머지이므로 main에는 남지 않는다.
  Linux에서만 드러나는 차이(errno 문구, `accept4`, 시간대 경로)는 main 머지 뒤 CI가 처음 본다. 머지 뒤 CI가 실패하면 main에서 고친다.
- 사례 수 상수(`DUMP_CASES` 등)는 사례를 더하거나 뺄 때 함께 바꾼다.

## 3. 완료 기준과 지킴이

**완료 기준**: 코드에 "Go"가 없다. 현재 문서에는 언어 비교 문장만 남는다.

- 범위: `crates`(기대 출력 `tests/expected` 포함), `extras`, `spec`, `scripts`, `tests/lang`, `examples`, `bench`, `deny.toml`, `.cargo`, `.github`, `install.sh`, `README.md`, `CLAUDE.md`, `docs`(아래 역사 문서 제외).
- 빼는 것(역사 기록): `docs/notes`, `docs/plans`, `docs/superpowers`, `CHANGELOG.md`의 `[0.1.0]` 절. `[Unreleased]`와 이후 절은 검사한다.
- 찾는 방법: 대소문자를 구분해 낱말 단위로 `Go`(앞뒤가 낱말 글자가 아님)를 찾는다. 바늘은 6b처럼 조각으로 만들어 테스트 파일 자신에 걸리지 않게 한다.
- 허용 목록: 언어 비교 문장만 (파일, 문장 조각) 쌍으로 둔다. 코드에는 허용 목록이 없다.
- `crates/fung-cli/tests/repo_rust_only.rs`를 넓혀 지킨다(6c-7에서 먼저 실패하는 것을 보고 통과시킨다).
- open-questions의 해결된 항목에 남는 Go 서술은 역사 기록 성격이다. 표현을 다듬어 남길지, 학습 노트로 옮기고 지울지는 6c-7에서 정한다.

## 4. 하위 단계별 결정

각 하위 단계를 시작할 때 채운다.

### 6c-1 기반

2026-10-17 사용자 결정(아홉 항목 모두 추천안). 계획 `docs/plans/2026-10-17-rust-port-cleanup-6c-1.md`.

| 항목 | 결정 | 이유 |
|---|---|---|
| 1. 유니코드 표·식별자 | 이름은 UAX #31: (`XID_Start` 또는 `_`)로 시작해 `XID_Continue`가 이어진다. 태그는 첫 글자가 Lu(지금과 같음). "보이는 글자"(`is_print`)는 일반 범주 L·M·N·P·S와 U+0020. 표는 **Rust 생성기**가 유니코드 데이터 파일(`UnicodeData.txt`, `DerivedCoreProperties.txt`, Unicode 17.0.0)에서 만든다(`FUNG_UPDATE_SPEC=1 FUNG_UCD=<dir>`), 평소 테스트는 고정 파일을 읽고 생성기와 같은지는 `FUNG_UCD`가 있을 때만 본다 | 표준이고(Rust·Python·JS), 결합 문자(Mn·Mc)를 받지 못하던 결함(`नमस्ते`)을 고친다. "보이는 글자" 표는 어차피 필요하다. 6c-4 정규식 표도 같은 생성기로 |
| 2. `upper`·`lower` | 유니코드 기본 대소문자 변환(전체 매핑, 끝 시그마 포함; `str::to_uppercase`·`to_lowercase`) | 표준의 기본 변환. 단순 매핑은 글자 단위 API의 근사 |
| 3. 큰 범위 펼치기 | 범위를 리스트로 **펼쳐야 하는** 함수만 원소 1억 개(100,000,000) 한도, 넘으면 `E0806 too many elements (limit 100000000)`. 인자 검사를 먼저. 결과가 모든 원소를 담는 함수(`map`·`sort`·`flatten`·`group_by`)만 검사하고, 일찍 멈추거나 결과가 작을 수 있는 함수(`find`·`any`·`all`·`filter`·`fold`·`each`)는 검사하지 않는다. `first`·`last`·`take`·`drop`은 범위를 펼치지 않는다 | 문자열 한도와 같은 숫자, 1.6 GB. 내부 에러가 아니라 한도 에러 |
| 4. `channel(n)` 한도 | 없앤다(음수는 지금처럼 E0403) | 버퍼를 미리 잡지 않으므로 지킬 것이 없다 |
| 5. `repeat` 넘침 | 넘치지 않는 곱셈으로 `E0806 too many characters (limit 100000000)` 하나 | 지금은 `wrapping_mul`이 감겨 Go 문구로 새는 결함 |
| 6. 표준 입력 단위 | 64 KiB(65,536바이트) | Linux 파이프 버퍼 크기, 로그 처리의 처리량. "필요할 때만, 한 번에 최대 64 KiB를 읽어 갈 수 있다"를 설계 문서에 적는다 |
| 7. `float(s)`·`parse_float(s)` | fung 실수 리터럴 문법 + 앞의 부호 하나 + 정수 꼴(`"42"`): 소수점 양쪽 숫자, 지수, `_`는 숫자 사이에만. 16진 실수·`.5`·`1.`·`inf`·`nan` 거부. `int`는 지금처럼 10진 | "문자열 변환은 리터럴 문법" 한 규칙, 엄격함 우선 |
| 8. 실행 형식 사전 검사 | 거부를 유지(셸 스크립트는 `sh()`). 문구는 6c-2에서 OS 에러 문구와 함께 | 조용한 `/bin/sh` 대체 실행보다 명시적 |
| 9. 채널 순회 | 값이 올 때마다 함수를 부른다(스트리밍). 인자 검사 먼저. `find`·`any`·`all`·`take` 등 일찍 멈추면 나머지는 채널에 남는다. 전체가 필요한 함수(`sort`·`reverse`·`join` 등)만 닫힐 때까지 받는다 | 05 "채널은 순회 대상"의 자연스러운 의미. 지금은 `for x in ch:`가 닫히기 전에 아무것도 하지 않는다 |
| 10. 정렬 (계획 때 찾음, 사용자 승인) | 안정 병합 정렬(인덱스, 위에서 아래로), 첫 비교 에러에서 멈춤. 표준 `sort_by`는 쓰지 않음 | 블록 삽입 정렬 + symMerge를 옮긴 것은 에러에 나오는 쌍을 맞추려는 것뿐이었다 |
| 11. 식별자 정규화 (리뷰에서 나옴, 2026-10-17 사용자 결정) | 렉서가 이름을 **NFC**로 정규화한다(조합형·분해형 `é`가 같은 이름). 정규화 표도 유니코드 생성기가 만든다 | UAX #31의 권고. macOS에서 온 분해형 글자가 "정의되지 않은 이름"이 되지 않게 |
| 12. 넓은 범위의 길이 (구현 중 찾음, 사용자 결정) | 길이를 정확히 센다(int64로 감지 않음): i64 폭보다 넓은 범위도 비지 않는다. `len()`이 int를 넘으면 `E0503 integer overflow`, 펼치는 함수는 E0806 | 범위는 원소를 펼치지 않는 값이라 넓다는 이유로 막을 까닭이 없다 |

### 6c-2 파일 시스템·OS

2026-10-17 사용자 결정(여덟 항목 모두 추천안). 계획 `docs/plans/2026-10-17-rust-port-cleanup-6c-2.md`.

| 항목 | 결정 | 이유 |
|---|---|---|
| 1. OS 에러 문구 | 흔한 errno(약 20개)는 fung이 정한 문구(지금 문구와 같은 소문자 문장), 나머지는 OS `strerror`를 소문자로 시작하고 `(os error N)`을 뗀 것. errno가 없는 에러도 fung 문구로 | 플랫폼(glibc·musl·macOS)마다 같은 문구, 표 300줄 → 20줄 |
| 2. `fs`의 여러 단계 함수 | `mkdir`(하위까지)은 `DirBuilder::recursive`(같은 이름 파일이 있으면 `'exists`), `remove`는 lstat 뒤 rmdir/unlink 하나, `remove_all`은 없는 경로 성공·`.`·`..`로 끝나는 경로 거부, `move`는 있는 디렉터리를 덮지 않음(fung 규칙으로 근거), 임시 디렉터리 규칙은 그대로 | 단순하고, 남긴 것은 그 자체로 안전 |
| 3. `list`·`walk` | 이름 바이트 순서는 유지. `walk`의 읽기 에러는 실패한 디렉터리 경로를 적는다 | 결정적 순서. 어디서 실패했는지가 보여야 한다 |
| 4. `glob` | 패턴 전체를 먼저 검사해 잘못된 패턴은 항상 에러. `\` 이스케이프·`!`는 글자·읽지 못하는 디렉터리 건너뜀·결과 정렬은 유지. 깊이 한도 10,000은 없앤다(패턴 길이가 한계) | 에러가 파일 시스템 내용에 따라 달라지면 안 된다 |
| 5. `path` | 점으로 시작하는 이름은 확장자가 없다(`ext(".hidden") == ""`, `stem(".hidden") == ".hidden"`). `join`은 이어 붙여 정리(뒤의 절대 경로가 앞을 지우지 않음), `..` 정리와 `base("") == "."`는 유지 | Python·Rust의 확장자 규칙. `join`의 절대 경로 재시작은 사고를 부른다 |
| 6. 파일 핸들 버퍼 | 표준 `BufWriter`·`BufReader` 64 KiB. 마지막 사본이 사라질 때 쓰기 버퍼를 비운다 | 버린 핸들의 데이터를 잃지 않는다 |
| 7. 따옴표 형식 | `diag::quote`(Go `strconv.Quote`)를 fung 문자열 리터럴 형식으로: 핸들 표시 `<file "...">`, 에러 문구의 `%q`, `import` 표시, 시간·JSON 문구 | 사용자가 보는 따옴표 문자열은 fung 문법이어야 한다 |
| 8. 명령 실행 문구 | `command not found in PATH`, `command found only in a relative PATH entry`(거부는 유지), `empty command name`, `argument contains a NUL byte`·`environment variable contains a NUL byte`, `not an executable file (run scripts with sh)`. 검사는 인자 순서로 | fung의 문구 |

### 6c-3 시간·JSON

2026-10-17 사용자 결정(열네 항목 모두 추천안, 그리고 6c-2 끝에서 정한 UTF-8 해독). 계획 `docs/plans/2026-10-17-rust-port-cleanup-6c-3.md`.

| 항목 | 결정 | 이유 |
|---|---|---|
| T1. 시간대 찾기 | C 라이브러리의 규칙: `TZ` 없음 → `/etc/localtime`, `""`·`UTC` → UTC, 앞의 `:` 뗌, 절대 경로는 그 파일, 상대 이름은 `$TZDIR` 또는 `/usr/share/zoneinfo`, **POSIX 시간대 문자열**(`KST-9`, `EST5EDT,M3.2.0,M11.1.0`)도 받음, 실패하면 UTC | 사용자가 아는 유닉스 규칙 |
| T2. 시각 표현과 범위 | 유닉스 기준 i64 초 + 나노초, **0000~9999년**. 넘으면 `from_unix`는 E0403, 산술은 E0503 | 모든 서식이 4자리 연도를 전제한다 |
| T3. 서식 없는 `parse` | RFC 3339 엄격: 두 자리 필드, `.` 소수 1~9자리, `Z`나 `±hh:mm`(hh ≤ 23, mm ≤ 59; T6의 `:ss` 허용), `T`·`Z` 대소문자 무시, 공백 구분 없음 | 느슨한 형태를 받지 않는다 |
| T4. 숫자 오프셋으로 읽은 시간대 | 항상 쓴 대로의 고정 오프셋 | 결과가 실행 환경에 따라 달라지지 않게 |
| T5. 없는/두 번 있는 벽시계 시각 | 없는 시각은 빈틈만큼 앞으로, 두 번 있는 시각은 이른 쪽 | java.time과 같은 명시적 규칙 |
| T6. 오프셋의 초 | 초가 있으면 `+08:27:52`로 쓰고 `parse`도 받는다 | 보이는 시각이 틀리지 않게 |
| T7. POSIX 규칙의 해 계산 | 정확히(윤년 366일) | 결함 |
| T8. 시간 에러 문구 | ``cannot parse `x` as a time: <이유>`` (이유: `month out of range`, `expected 2 digits for the hour`, `expected a month name like Oct` 등) | fung 문구 |
| J1. JSON 파서 | 입력 전체를 보는 반복 파서 하나. 에러 위치는 문제의 그 글자(글자 단위 줄·열), 깊이 한도 10,000 | 읽기 버퍼 흉내를 없앤다 |
| J2. JSON 에러 문구 | `unexpected character ']'`, `unexpected end of input`, `unexpected text after the value`, `invalid escape`, `control character in a string`, `number out of range`, `nesting too deep (limit 10000)` 등 | fung 문구 |
| J3. 짝 없는 서로게이트 | `'parse` 에러 | 엄격함 우선 |
| J4. 같은 키 | `'parse` 에러 ``duplicate key `a` `` | 엄격함 우선(남의 JSON을 거절할 수 있음을 알고 정함) |
| J5. `stringify` 이스케이프 | RFC 8259에 필요한 것만: `"` `\` 와 U+0020 미만(`\b \f \n \r \t`, 그 밖은 `\u00XX`) | 표준의 최소 |
| J6. 큰 `indent` | 결과에 1억 글자 한도(E0806), 음수 `indent`는 E0403 | 6c-1 한도와 같은 규칙 |
| U. UTF-8이 아닌 바이트 | 유니코드 권장 방식(잘못된 부분 수열마다 U+FFFD 하나, 표준 `from_utf8_lossy`), `decode_go` 이름도 바꾼다 | 표준 |

### 6c-4 정규식

2026-10-18 사용자 결정(여섯 항목 모두 추천안). 계획 `docs/plans/2026-10-18-rust-port-cleanup-6c-4.md`.

| 항목 | 결정 | 이유 |
|---|---|---|
| R1. 문법을 누가 정하나 | **`regex` 크레이트의 문법**이 fung 정규식 문법이다(RE2 계열). 옮겨 온 Go 파서(`regex_go.rs`), Go 고정 회귀 데이터, `spec/regex_unicode.txt`를 지운다 | 다른 런타임의 파서와 그 우연을 지키지 않는다. 정규식 문법은 문서화된 방언이다. 크레이트 업그레이드는 `Cargo.lock`과 기대 출력 테스트가 지킨다 |
| R2. `\d \w \s \b` | 유니코드(크레이트 기본, Python 3와 같음). ASCII만 원하면 `(?-u:\d)` | 식별자의 UAX #31과 같은 방향 |
| R3. 에러 문구 | ``invalid pattern `a(`: unclosed group``(크레이트의 에러 종류 한 줄), 코드 E0515 그대로 | fung 문구 |
| R4. `find_all`·`replace` | 크레이트의 규칙(앞 매치에 붙은 빈 매치는 세지 않음, `$1`·`${name}`·`$$`, `$1a`는 이름 `1a`) | 지금과 결과가 같다 |
| R5. `split`과 빈 매치 | 주어의 맨 앞과 맨 끝에서 생긴 **빈 매치로는 자르지 않는다**: `split("abc", "")` → `["a","b","c"]`, `split("", "x*")` → `[""]`. 가운데 빈 조각은 남긴다 | 문자열 `split`과 같게(fung 안의 일관성) |
| R6. 패턴 캐시 | 256개, 넘치면 통째로 비움(보이지 않는 성능 세부). 이름만 정리 | — |

### 6c-5 네트워크·HTTP

2026-10-18 사용자 결정(열두 항목 모두 추천안). 계획 `docs/plans/2026-10-18-rust-port-cleanup-6c-5.md`.
기본 원칙: HTTP/1.1 표준(RFC 9110·9112)의 규칙(IMF-fixdate `Date`, 이유 문구 표, chunked, 프록시 보안 규칙 등)은 남기고 문구와 이름만 바꾼다.

| 항목 | 결정 |
|---|---|
| N1. 네트워크 에러 문구 | fung의 짧은 문구: ``cannot connect to `host:port`: connection refused``, ``cannot resolve `host`: no such host``, `connection is closed`, `timed out`. HTTP는 ``cannot request `url`: <이유>``(안쪽 `Get "url":`·`dial tcp` 겹침 없음). 이유는 6c-2의 OS 문구, `accept4`/`accept` 같은 플랫폼 차이 없음 |
| N2. 주소 규칙 | 빈 listen 호스트 `[::]` 이중 스택, 빈 dial 호스트 로컬, 두 번째 `close`는 에러: 그대로. `EOF`/`unexpected EOF`는 `connection closed after N of M bytes`, 포트·주소 문구는 fung 문구 |
| N3. TLS | rustls가 판단한 에러 종류를 fung 문구로(`certificate has expired`, ``certificate is not valid for `host` ``, `unknown certificate authority`, `TLS alert: <이름>`, `TLS handshake timed out`), 검사 순서는 rustls의 것, 시각은 fung ISO |
| N4. SIGPIPE | 그대로(닫힌 표준 출력·에러에서 신호 13, 부모의 무시 설정을 기본으로 되돌림, `exit(141)` 대비책). 근거만 fung 규칙으로 |
| H1. `User-Agent` | `fung/<버전>`(사용자 값이 덮고 빈 값이면 보내지 않음), `CONNECT`에도 |
| H2. 요청·응답 규칙 | RFC 9112 규칙은 그대로. 사용자 헤더는 정규화한 뒤 **맵 순서 그대로**(정렬하지 않음). 문구는 fung 문구 |
| H3. 연결 시간 한도 | 내부 상한(TCP 30초, TLS 10초, `CONNECT` 60초)을 없애고 요청의 `timeout` 하나가 연결·핸드셰이크·응답을 덮는다. 넘으면 `Err('timeout)` |
| H4. 리다이렉트 | 10번 한도·`Referer`·민감 헤더 버리기·홉마다 Basic 인증은 그대로. 리다이렉트 본문을 읽고 버리지 않고 바로 닫는다. 문구는 fung 문구(`too many redirects (limit 10)`, ``invalid Location header `…` ``) |
| H5. 프록시 | 환경 변수 우선순위, CGI 안전장치, 스킴 없는 값, `NO_PROXY` CIDR, `CONNECT`는 그대로. 문구·`User-Agent`·기한(H3)만 |
| H6. 서버 | 표준 이유 문구·`Date`·204/304/1xx·`nosniff`·413/431 뒤 읽고 버리기·`accept` 재시도는 그대로. 모르는 코드는 빈 이유 문구, 서버가 스스로 내는 본문은 `<코드> <이유>\n`, 요청 머리 한도 1 MiB, 로그 줄 문구 다듬기 |
| H7. `tls_test_server.rs` | `crates/fung-cli/tests/common/`으로 옮기고 rustls는 fung-cli의 dev-dependency(`deps.rs` 맞춤) |
| H8. 흔들리는 `bad5` | "닫힌 포트"를 고르지 않고, 테스트가 쥔 리스너로 결과가 정해지게 바꾼다 |
| H9. 사용자 `Host` (2026-10-18, 최종 리뷰 뒤) | 비어 있지 않은 사용자 `Host`가 자동 값을 대신한다(`User-Agent`와 같음). 리다이렉트가 다른 `host:port`로 가면 그 요청부터 버린다. `Content-Length`·`Transfer-Encoding`·`Trailer`는 계속 클라이언트가 쓴다 |
| H10. 이유 문구 표의 출처 (같은 날) | IANA HTTP 상태 코드 레지스트리. 418은 레지스트리가 "(Unused)"로 두므로 빈 이유 문구 |

### 6c-6 도구

2026-10-18 사용자 결정(모두 추천안). 계획 `docs/plans/2026-10-18-rust-port-cleanup-6c-6.md`.

| 항목 | 결정 |
|---|---|
| T1. `test`의 에러 중복 제거 | `(소스 이름, 줄, 열, 코드)` 키 그대로. 이름·주석만 |
| T2. 읽지 못한 테스트 파일 | 로드 실패와 같게: `FAIL path: <load>`, 실패 하나, 종료 코드 2 |
| T3. 종료 핸들러 에러 | `FAIL path: <exit handler>`, 실패 하나 |
| T4. 테스트 안의 `exit(n)` | 그대로(08 규칙: 실패 하나, 명령 종료 코드는 n이 아님) |
| P1. `.` 디렉터리 | `test`·`fmt` 모두 건너뜀(`_`도) |
| P2. 심볼릭 링크 | 인자로 준 링크는 둘 다 따라감, 순회 중의 링크는 둘 다 따라가지 않음 |
| P3. 파일 에러 문구 | `fs`와 같은 꼴 ``cannot read `p`: <이유>``(`run`·`check`·`test`·`fmt`) |
| C1. `check -e CODE 나머지` | 사용 에러(종료 코드 2) |
| C2. `check` 디렉터리 | 경로 하나 그대로, ``cannot read `dir`: is a directory`` |
| S1. 표준 입력 이름 | `<stdin>` |
| F1. 포매터 정렬 너비 | 표시 너비(UAX #11: 넓은 글자 2칸, 결합 문자·서식 문자 0칸), 잘못된 UTF-8 부분은 U+FFFD 하나로 1칸. 표는 유니코드 데이터에서 생성 |
| R1. REPL 발췌 | 입력마다 글을 남겨, 정의한 입력의 줄을 보인다(런타임 보고 포함) |
| R2. REPL 전역 한도 | 넘긴 입력의 선언을 되돌려 그 입력만 실패 |
| R3. `/dev/null` 등 | `isatty`일 때만 터미널 모드 |
| R4. 줄 끝 | LF 하나와 그 앞의 CR 하나만 뗀다(두 읽기 길 모두) |
| R5. 기록 파일 | 원래 바이트로 쓴다 |
| R6. REPL 종료 코드 | 그대로(세션이 끝난 방식만 따름) |
| W1. W0102 순서 | 이름 순서 그대로, 08에 규칙으로 |

### 6c-7 마무리

2026-10-18 사용자 결정(모두 추천안). 계획 `docs/plans/2026-10-18-rust-port-cleanup-6c-7.md`.

| 항목 | 결정 |
|---|---|
| 1. 코드 주석·테스트 이름 | 주석은 규칙과 이유로, `*_like_go` 등 테스트 이름은 확인하는 내용으로 |
| 2a. 기간 뺄셈 | `a - b`는 결과가 범위 안이면 돌려준다(`b`가 최솟값이어도) |
| 2b. 범위 자르기의 넘침 | 에러는 사용자가 쓴 인덱스를 보인다(감긴 숫자가 아니라) |
| 3. open-questions의 이식 기록 | 해결된 항목과 이식 기록은 학습 노트로 옮기고 open-questions에는 열린 항목만 fung 말로 |
| 4. 언어 비교·이름 유래 | 00에 "영향을 준 언어" 절(Go·Lisp·Erlang/Elixir·Python 등), README·매뉴얼은 요약, 다른 문서의 비교 문장은 그대로(지킴이 허용 목록) |
| 5. 06 파일 이름 | `06-native-and-stdlib.md`, 역사 문서의 링크도 고침(내용은 그대로) |
| 6. `bench/README.md` | 0.1.0 열은 "0.1.0"으로 남기고 "Go HEAD" 열은 지운다 |
| 7. CHANGELOG | 전체를 역사 문서로 보고 지킴이에서 뺀다 |
| 8. E0703 | 번호와 항목을 남기고 "쓰지 않음(0.1의 atom, E0704가 이음)" 표시 |
| 9a. 줄 편집기 키 디코딩 | 잘못된 UTF-8은 부분마다 U+FFFD 하나 |
| 9b. 줄 편집기 커서 | 표시 너비(`unicode::width`)로 센다 |
| 10. 지킴이 | 3절대로: 낱말 단위 `Go`, 빼는 것은 `docs/notes`·`docs/plans`·`docs/superpowers`·`CHANGELOG.md`, 허용 목록은 언어 비교 문장의 (파일, 조각) |
| 11. 실수의 지수 (구현 중 찾음, 같은 날) | 지수를 채우지 않는다: `1e-7`, `1e-5`(`1e21`과 같은 규칙) |
| 12. `fs.open`의 새 파일 권한 (같은 날) | `'write`·`'append` 모두 0666에서 umask를 뺀 권한(셸의 `>`·`>>`와 같음) |
| 13. REPL 전역 한도 (같은 날) | 없앤다(고정 크기 표의 흔적). 표는 늘어나고 메모리는 다른 한도가 막는다 |

## 5. 범위 밖

0.2.0 릴리스(6c 뒤, 태그·시험 실행은 사용자 확인), rustls 0.23.46 업그레이드(2026-10-28 이후), R7 스케줄러, 정규식 문법 자체의 재판단(open-questions "정규식 문법"),
"알고 남긴 차이" 중 Rust가 Go를 따르지 않은 방향의 항목(U+FFFD, 환경 순서, `close(2)` 에러 등; 해당 영역 단계에서 언어 규칙으로 정할 일이 생기면 그때 묻는다),
학습 노트·계획·스펙·CHANGELOG 0.1.0의 과거 기록.
