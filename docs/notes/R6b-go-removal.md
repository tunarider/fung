# R6b Go 삭제: 무엇을 했고 왜 그렇게 했나

2026-10-17. 계획 `docs/plans/2026-10-17-rust-go-removal-6b.md`, 스펙 `docs/superpowers/specs/2026-10-17-rust-go-removal-6b-design.md`,
결정과 남은 일은 `docs/design/open-questions.md`의 "Rust 이전"(6b단계 항목, 6c 할 일). 커밋 `fb3d326..` 이 노트의 커밋까지.

6a까지 Go 구현은 **정답지**였다. Rust가 내는 모든 출력을 같은 입력의 Go 출력과 바이트 단위로 비교했고, 그 비교가 이식의 안전망이었다.
6b는 그 정답지를 치우는 단계다. 어려운 점은 지우는 일 자체가 아니라 **안전망을 잃지 않고** 지우는 일이었다.
- Go를 지우면 "Rust가 맞다"를 무엇으로 확인하는가?
- 명세 파일(`spec/builtins.txt`·`errors.txt`)은 Go가 만들었다. 이제 누가 만드는가?
- Go만 지키던 검사(`install.sh`, CHANGELOG 짝)는 어디로 가는가?

사용자가 정한 최종 목표는 "처음부터 Rust로 짠 것 같은 저장소"다. 6b는 그 첫 단계(지우기와 이름표 떼기)이고, Go를 흉내 내던 동작을 걷어내는
일은 6c로 나눴다. 6b는 **동작을 바꾸지 않는다**: 얼린 기대 출력이 그대로 맞아야 한다.

| 작업 | 커밋 |
|---|---|
| 1. 출력이 정해지는 대조를 기대 출력 파일로 얼리기 | `fb3d326`, `f2163ae` |
| 2. 네트워크·신호 대조를 Rust 단독으로 | `177b6b2` |
| (CI를 브랜치 푸시에서, 자식 프로세스 거두기, `TZ` 고정, 시간 대신 증가율) | `e592aac`, `674ed7b`, `70f07ae`, `c900aa7` |
| 3. 명세 파일을 Rust에서 생성 | `98f4405` |
| 4. Go 테스트가 지키던 것(`install.sh`, CHANGELOG)을 Rust로 | `3319e14` |
| 5. Go 삭제 | `f761d65` |
| 6. `extras/rust/`·`# only:`·Rust 전용 차이 목록을 하나의 명세로 | `e07f181` |
| 7. 문서(이 노트, `CLAUDE.md`, 설계 문서, README, 매뉴얼) | 이 노트의 커밋 |

순서가 중요했다. 1~4는 **Go가 있어야** 할 수 있다(Go 출력을 기록하고, Go 테스트가 무엇을 지켰는지 보고 옮긴다). 5는 그 뒤, 6은 Go가 읽던
파일(`prelude.fung`)을 고치므로 Go가 사라진 뒤에야 할 수 있다.

## 1. 무엇을 지웠나

- Go 구현: `cmd/`, `internal/`, `go.mod`, `tests/*.go`(골든·명세·포맷·성능·층 구조·설치·CHANGELOG 테스트), `extras/extras.go`(`embed`).
- `# only: go` 골든 파일 다섯 개(`atoms`·`conc_atoms`·`err_atom_reentry`·`http_serve`·`conc_helpers`의 Go판). Rust에는 `atom`이 없다.
- Go 전용 `extras/http.fung`(Go `net/http`를 감싼 얇은 래퍼). Rust의 fung HTTP(`extras/rust/http.fung`)가 그 자리로 옮겨 왔다.
- 대조 장치: `common::go_fung*`(Go 빌드), `FUNG_REQUIRE_PARITY`(Go가 없으면 조용히 건너뛰지 말라는 변수), 기록 모드, `go_source`(Go에 넘길 때 `import conc`를 지우던 도우미),
  `fung-vm`의 `testutil::go_fung`·`run_go`, `# only:` 표시와 그것을 읽던 골든 실행기·실행 비교의 분기, `RUST_ONLY_EXAMPLES`.
- Rust 전용 차이 목록: `RUST_PRELUDE_FUNG_MOVED`(Go와 함께 쓰는 `prelude.fung`에서 `fan_in` 등을 숨김), `RUST_TYPE_RENAMES`(`atom` → `agent`) 등. Go와 파일을 나눠
  쓰는 동안 "Rust만 다른 점"을 한 곳에 모아 두던 것이라, 명세가 하나가 되자 필요가 없어졌다.
- CI·릴리스 워크플로의 Go 작업과 `setup-go`.

지운 뒤 Go가 다시 들어오지 않게 `crates/fung-cli/tests/repo_rust_only.rs`를 먼저 썼다(TDD: 지우기 전에 실패하는 것을 봤다). `.go` 파일·`go.mod`가 없는지, 워크플로·스크립트·
크레이트·`CLAUDE.md`·`README.md`·`docs/design`에 Go 도구 호출이 없는지 본다. 두 가지 작은 기술이 있다.
- **낱말 단위로 찾기**: 그냥 부분 문자열로 찾으면 `cargo test`도 `go test`에 걸린다(`…go test`). 앞뒤 글자가 낱말 글자가 아닐 때만 센다.
- **바늘을 쪼개 쓰기**: 이 테스트 파일 자체가 찾는 문자열을 담고 있으면 자기 자신에 걸린다. `format!("{g} test")`처럼 조각으로 만든다.
- 학습 노트·계획·스펙·CHANGELOG는 뺐다. 그것들은 Go 시절을 **기록**하는 문서라서 Go가 나오는 것이 맞다.

## 2. 대조를 기대 출력으로 얼리기

### 무엇이 바뀌었나

전에는 테스트가 Go와 Rust를 둘 다 실행해 출력을 비교했다. 이제는 사례마다 기대 출력을 파일에 적어 두고 Rust만 실행해 그 파일과 비교한다.

```
==== run tests/lang/arith.fung
-- stdout
| 7
| 9 -4
-- stderr
-- exit: 0
```

`crates/fung-cli/tests/expected/<주제>/*.txt`(dump, 실행, `check`·`test`·`fmt`·REPL·경로 순회, `json`·`time`·`regex`, `http`·`tls`)와
`crates/fung-vm/tests/expected/{prelude,fs}/runs.txt`, 모두 3,570개 사례다.

### 어떻게 얼렸나: Go를 기록하고, Rust로 갱신한다

얼리는 방법에는 두 갈래가 있었다.
1. Rust 출력을 기록한다. 쉽지만 **순환**이다. "Rust가 맞다"를 Rust가 증언한다. 기록하던 순간 Rust에 버그가 있었다면 그 버그가 기대값이 된다.
2. Go 출력을 기록한다. 정답지의 판정을 그대로 옮겨 적는다.

6b는 2를 골랐고, 한 단계를 더했다. 기록 모드(`FUNG_EXPECTED=record`)는 사례마다 **Go와 Rust를 둘 다 실행해 같은지 먼저 단언**하고, 같을 때만 Go의 출력을 썼다.
그래서 기록하는 순간에도 대조가 살아 있었고, 기록된 파일은 "Go가 낸 것이자 Rust가 낸 것"이다. 기록을 두 번 돌려 `git diff`가 비는 것(결정적)도 확인했다.
예외가 둘 있다. Go를 지운 커밋에서 옮긴 fung-vm의 기대 출력(`prelude/runs.txt` 357개·`fs/runs.txt` 9개·`repl/tty.txt`)은 bless로 Rust에서 썼다. 바로 앞 커밋까지 같은 사례를 살아 있는 Go와 그대로 대조해 통과했고, 그 커밋은 실행 코드를 바꾸지 않았으므로 내용은 같다. Task 6에서 더한 사례(전에 Rust 전용이던 파일)는 대조할 Go가 없어 처음부터 Rust 출력이다.

Go를 지운 뒤의 갱신 경로는 1이다. `FUNG_EXPECTED=bless cargo test -p <크레이트> --test <이름>`이 Rust 출력으로 파일을 다시 쓴다. 이때 순환을 끊는 것은 **사람의 검토**다.
일부러 동작을 바꿨을 때만 bless하고, 커밋 전에 diff를 읽어 바뀐 줄이 바꾸려던 것뿐인지 본다. 형식이 이 검토를 쉽게 하도록 짜였다.

### 형식을 그렇게 짠 이유

- **한 줄에 한 줄**: 출력 한 줄을 `| ` 한 줄로 적는다. 동작이 바뀌면 diff에 바뀐 줄만 보인다. JSON 같은 구조 하나에 담았다면 diff가 읽히지 않는다.
- **잘못된 바이트는 이스케이프**: UTF-8이 아니거나 제어 문자가 든 값은 `-- stderr (escaped)`로 표시하고 `\xNN`으로 적는다. 파일 자체는 언제나 읽을 수 있는 글이다.
- **큰 값은 해시**: 정해 둔 크기를 넘는 값(아주 깊은 dump, 16 MB 요청)은 길이와 FNV-1a 해시만 남긴다. 저장소가 수십 MB로 불지 않는다. 대가로 그 사례가 틀리면 어디가 틀렸는지는 보이지 않는다.
- **치환**: 실행마다 달라지는 값은 이름으로 바꾼다. 임시 디렉터리 `$TMP`, 저장소 `$REPO`, 포트 `$PORT`, 버전 `$VERSION`. 치환 없이 얼리면 다음 실행에서 바로 틀린다.
- **사례 수 단언**: 테스트마다 사례 수를 상수로 단언한다(`DUMP_CASES` 등). 사례 목록을 만드는 코드가 실수로 빈 목록을 돌려주면 "0개 비교, 모두 통과"가 된다. 이것을 막는다.
- **엄격한 비교**: 값이 다른 것뿐 아니라 빠진 항목, 남는 항목, 순서 바뀜, 같은 이름표 둘도 실패다.

### 이름표 떼기

`parity.rs`는 `dump_expected.rs`로, `run_parity`는 `run_expected`로, `http_parity`는 `http`로 바꿨다. 테스트 이름의 `_matches_go`도 `_matches_the_expected_output`이 됐다.
"처음부터 Rust로 짰을 때의 골든 테스트와 구별되지 않게"가 기준이었다. 기대 출력이 어디서 왔는지는 이 노트와 open-questions가 기록한다.

### 실제 서버와 맞붙이던 것

`http`·`tls`·`sigpipe` 대조는 Go 서버에 Rust 클라이언트를, Rust 서버에 Go 클라이언트를 붙였다. 이것은 출력 하나를 얼리는 것으로 끝나지 않는다.
- 클라이언트: Rust 클라이언트가 가짜 서버에 보낸 **요청 원문**을 바이트 그대로 얼렸다(`request <서버>.<n>` 칸). 응답과 에러 문구도 함께.
- 서버: 원시 요청 88개를 Rust 서버에 보내고 받은 **응답 원문**을 얼렸다(`Date`는 가림).
- TLS: Go TLS 서버 대신 테스트 안의 rustls 서버(`fung_vm::tls_test_server`)를 쓴다. 바꾸기 전에 Go 서버로 기록한 파일이 Rust 서버로도 그대로 통과하는 것을 확인했다.
  `fung-cli`는 rustls에 직접 의존할 수 없어서(`deps.rs`) `fung-vm` 안에 문서에서 숨긴 공개 모듈로 두었다.
- SIGPIPE: 신호 13과 EPIPE 문구는 원래 상수로 고정돼 있어 Go 쪽만 떼면 됐다.

### Go 테스트가 지키던 것

`tests/install_test.go`(가짜 릴리스를 만들어 `install.sh`를 실행)와 `tests/changelog_test.go`(CHANGELOG 절과 매뉴얼 사이드바 버전의 짝)는 Rust 통합 테스트
`install_script.rs`·`changelog.rs`로 옮겼다. 옮긴 뒤 일부러 고장을 내 실패하는지 확인했다(4/4). errno 표를 살아 있는 Go와 대조하던 테스트는 기록으로 바꿀 수 없었다.
Linux 표를 이 기계에서 만들 수 없었기 때문이다. 대신 표(`native/oserr.rs`) 자체를 기록으로 보고, Rust 단독으로 대표 문구와 "모든 칸이 한 줄"을 확인한다.

## 3. 호스트 의존: 기록한 기계의 것이 섞여 들어간다

기대 출력은 **macOS에서만** 기록했다(이 기계). CI는 Ubuntu와 macOS 둘에서 돈다. 그래서 Go를 지우기 전에 관문을 하나 두었다. 기록한 파일을 브랜치에 푸시하고,
Go가 아직 있는 상태에서 Linux CI가 같은 파일로 통과하는지 본다(사용자 결정 2026-10-17). 통과하지 않으면 Go가 있는 동안 Linux에서 다시 기록할 수 있다. 실제로 걸린 것들:

- **시간대**: 첫 Linux 실행에서 `json_expected` 37개 중 하나가 두 OS 모두에서 틀렸다. 그 프로그램이 지역 시각을 찍었고, 기록한 기계의 시간대(Asia/Seoul)가 기대값에 들어가
  있었다. 기록할 때는 Go와 Rust가 같은 기계의 같은 시간대를 읽으니 둘이 같았다. 대조는 이것을 잡지 못한다. **둘 다 같은 환경을 보면 둘 다 같이 틀린다.**
  고친 방법: 모든 기대 출력 실행에 `TZ=UTC`(`common::TZ`; UTC는 zoneinfo 파일도 필요 없다). `time_expected`만 자기 `TZ` 목록을 쓰고, `TZ`가 없을 때의 동작은
  "`TZ=/etc/localtime`과 같다"로 비교한다(값을 얼리지 않음).
- **비UTF-8 파일 이름**: Linux는 `w\xfe.fung` 같은 이름의 파일을 만들 수 있지만 macOS(APFS)는 거부한다. macOS에서 기록하면 그 사례는 아예 기록되지 않는다.
  그래서 얼린 사례에서 빼고, "같은 내용의 UTF-8 이름 파일과 출력이 이름만 다르다"를 비교하는 **유도 테스트**로 바꿨다. 그리고 그 유도 테스트를 UTF-8 대역 이름(`wé.fung`)으로
  모든 OS에서도 돌리게 했다. Linux에서만 도는 코드는 macOS에서 한 번도 실행되지 않는다. 대역으로 돌려 보자 바로 버그 둘이 나왔다(9바이트 창으로 7바이트 바늘을 찾던 것,
  제자리 `fmt`가 아무것도 안 찍는다고 가정한 것). "이 기계에서는 돌지 않는 코드"는 리뷰 말고는 검증되지 않는다는 교훈이다.
- **root 실행**: 권한 000 디렉터리를 읽지 못하는 사례가 있다. root는 그것도 읽는다(컨테이너 CI가 흔히 root). 그 사례는 먼저 탐침 파일로 읽히는지 보고, 읽히면 비교하지 않는다.
- **파일 시스템 값**: 디렉터리 크기(APFS 96, ext4 4096)는 `$SIZE`로, macOS의 `/tmp`가 실제로는 `/private/tmp`인 것은 `/tmp`로 치환했다.
- **부하에 따라 흔들리는 시간 한도**: 헤더 수십만 줄을 읽는 시험이 "4초 안"이었는데, 모든 코어에 부하를 건 반복 실행에서 15번 중 1번 넘었다. 절대 시간 대신 **증가율**
  (입력을 늘렸을 때 시간이 선형으로 느는가)로 바꿨다(`c900aa7`). 기계 속도가 아니라 알고리즘의 성질을 재는 것이다.

관문은 두 번 통과했다. 기록 직후(Go 있음)와 Go 삭제 직후. 6b의 모든 커밋은 브랜치 푸시마다 두 OS에서 돈다.

## 4. 고아 프로세스와 `Reaped`

Task 2 도중 이 기계에 6시간 된 프로세스 둘이 남아 있는 것을 발견했다(`fung-go-http-examples-… run examples/static.fung`, 부모 1). 서버를 띄운 테스트가 단언에서
패닉하면, 서버를 쥔 구조체가 그 서버를 죽이지 않은 채 사라졌던 것이다. Rust의 `std::process::Child`는 `Drop`에서 자식을 죽이지 않는다(기다리지도 않는다).
테스트가 실패할 때마다 서버가 하나씩 남아 포트와 메모리를 쥐고 있게 된다.

사용자 요청으로 규칙을 정했다. **테스트가 띄우는 모든 자식은 패닉해도 죽이고 거둔다.** `common::Reaped`는 `Child`를 감싸 `Drop`에서 `kill` 뒤 `wait`한다
(이미 거둔 자식에 `kill`은 아무 일도 하지 않는다). 출력을 모아야 하는 곳은 `into_inner()`로 꺼내 `wait_with_output`에 넘긴다.
`a_panicking_test_leaves_no_server_behind`가 일부러 패닉한 뒤 그 pid가 사라졌는지(`kill -0`) 확인하고, `Drop` 본문을 비워 이 테스트가 실패하는 것도 봤다.

손자 프로세스는 거두지 않는다. 자식마다 프로세스 그룹을 따로 만들면 그룹째 죽일 수 있지만, 그러면 터미널의 Ctrl-C가 자식에게 가지 않아 중단한 `cargo test`가 자식을 남긴다.
테스트가 띄우는 프로그램은 손자를 남기지 않으므로(`sh`는 `exec`으로 fung이 되고, fung은 자기가 띄운 프로세스를 기다린다) 지금은 이 편이 낫다.

## 5. 명세의 원본을 Rust 코드로

`spec/builtins.txt`(내장 이름과 매개변수)와 `spec/errors.txt`(에러 카탈로그)는 Go 테스트가 Go 코드에서 생성했고, Rust는 그 파일을 읽었다. Go가 사라지면 원본이
없어진다. 원칙은 "코드가 원본, 파일은 코드에서 만든 사본, 테스트가 둘이 같은지 지킨다"이다. 갱신은 `FUNG_UPDATE_SPEC=1 cargo test spec_file` 하나.

- 에러 카탈로그: 파일 내용을 그대로 Rust 표(`fung_front::diag::catalog`)로 옮겼다. 조회는 이제 표를 읽고, 파일은 표에서 찍는다.
- 내장 이름: `fung_vm::boot`가 실제로 부팅하는 표들(`NATIVE_MODULES`, prelude 네이티브, `extras::PRELUDE_FILES`)에서 줄을 만든다.

처음 판(Task 3)에는 **닭과 달걀** 문제가 있었다. 생성기가 런타임을 부팅해서 표를 얻는데, 부팅하는 앞단이 `spec/builtins.txt`를 읽었다. 그래서 새 `_native.*` 모듈을 더하면
파일에 손으로 먼저 적어야 생성기가 돌았다. Task 6에서 생성기가 파일을 읽지 않고 Rust 표에서 시작하게 바꿨다. 네이티브 줄을 먼저 만들고, 그 명세로 prelude 파일들을 컴파일해
fung 이름을 더하고, 마지막으로 `conc`·`http` 모듈을 그때까지 만든 명세로 컴파일한다(`Loader`가 파일 대신 넘겨받은 명세를 쓸 수 있게 했다). 증명: 파일에서 `_native.net`,
`_native.conc`, `prelude fung`, `module http` 줄을 모두 지우고 갱신 명령을 돌리면 파일이 바이트 그대로 돌아온다.

`spec/unicode.txt`·`regex_unicode.txt`(Go의 유니코드 판정 표)와 `regex_go_fixtures.rs`(무작위 정규식 기대값)는 Rust에서 다시 만들 방법이 없다. 지금 내용으로 **고정 데이터**로
두고, 머리 주석이 다시 만드는 법(Go를 지우기 전 커밋의 Go 테스트를 Go 툴체인으로 돌림)을 적은 open-questions를 가리킨다. 표를 Rust에서 얻게 되면(6c) 그 방법은 버린다.

## 6. 브랜치와 스쿼시: 머지 뒤에는 다시 올린다

이식 단계는 작업 브랜치에 쌓고, 사용자가 요청할 때 main에 **스쿼시 머지**한다(main 기록을 단계마다 커밋 하나로). 6a를 스쿼시 머지한 뒤 같은 브랜치에서 6b를 이어 쌓다가
사용자가 짚었다. 스쿼시 머지는 브랜치의 커밋들을 main에 **새 커밋 하나로 다시 만든다**. 브랜치에는 옛 6a 커밋들이 그대로 남아 있으므로, git이 보기에 브랜치와 main은
같은 변경을 서로 다른 커밋으로 가진 두 갈래다. 다음 머지에서 6a 변경이 다시 들어오려 하거나 충돌한다. 스쿼시의 목적(깨끗한 main)이 무너진다.

고친 방법: 스쿼시 직후 브랜치를 main 위로 다시 올린다. `git rebase --onto origin/main <트리가 main과 같은 브랜치 커밋>`은 "그 커밋 뒤의 새 커밋들만" main 위로 옮긴다.
그리고 `--force-with-lease`로 브랜치를 푸시한다. 6b의 앞선 커밋 6개를 이렇게 옮겼다. 같은 자리에서 정한 것: CI를 돌리려고 초안 PR을 열지 않고, `ci.yml`이 `claude/**`
브랜치 푸시에서도 돌게 했다(`e592aac`).

## 7. 일부러 남긴 것과 6c

6b는 동작을 바꾸지 않았으므로 Go를 흉내 내던 것들이 그대로 있다. 그것이 6c의 입력이다(open-questions "6c 할 일").
- **동작**: Go 문구로 만든 운영체제·네트워크·TLS·JSON·시간 에러, Go `filepath`·`Glob` 규칙, `GoWriter`의 4 KiB 버퍼와 비우는 시점, `User-Agent: Go-http-client/1.1`과
  Go HTTP 클라이언트·서버의 세부, 시각 값의 Go 표현, 채널 순회가 닫힐 때까지 다 받는 것 등. 하나씩 바른 동작을 설계 문서에 정하고, 코드를 고치고, 기대 출력을 bless한다.
- **이름과 주석**: `crates/` 아래 약 126개 파일의 "Go …" 주석, `regex_go*`·`GoWriter`·`GoOrder`·`*_like_go` 같은 이름, `deny.toml`·`build-release.sh`·`extras/*.fung`의 주석.
- **문서**: 06의 HTTP·정규식 절에 남은 Go 규칙 서술, 06의 파일 이름, 매뉴얼의 `User-Agent` 줄.
- 6c를 마치면 코드와 현재 문서에서 "Go"가 검색되지 않는다(완료 기준). 언어 설계의 비교("겉은 Go, 속은 Lisp", 이름의 유래)를 그 기준에서 어떻게 다룰지는 6c에서 정한다.
  0.2.0 릴리스는 6c 뒤다.

## 8. 다음

6c(이식 잔가지 정리) → 0.2.0 릴리스(태그 전 릴리스 워크플로 시험 실행, 사용자 확인) → R7 스케줄러(그린 스레드).
