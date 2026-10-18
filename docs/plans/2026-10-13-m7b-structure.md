# M7.5 구조 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 06의 네 층(core · native · extras · 프로젝트 extras)을 코드 구조로 만든다: `internal/prelude`(core의 Go 값 연산), `internal/native`(`_native.*` 다리만), `extras/`(fung 내장, embed), 조립은 `cmd/fung`만. 조합 함수 20여 개를 fung으로 옮기고, "`internal/`은 `extras/`를 import하지 않는다"를 테스트로 고정한다. 언어 의미는 바뀌지 않는다(골든 128개 그대로 통과).

**Architecture:** `internal/nat`(선언 도우미: `Def` `Param` `Opt` `Rest` `Ok` `ErrValue` `NeedStr` …)를 prelude와 native가 공유한다. `internal/prelude.Core(args)`가 core 값 연산 표를, `internal/native.Bridges()`가 `_native.fs/path/process`를 준다. `extras.Source(name)`이 embed된 fung 파일을, `extras.Default()`가 `{Stdlib, Natives(Go 다리)}` 레지스트리를 준다(vm의 `_native.conc`는 cmd가 합친다 — extras는 vm을 모른다). `frontend.Compile`은 기본 stdlib 없이 동작한다(핵심 패키지는 extras를 모른다); stdlib이 필요한 호출자(cmd, vm 테스트)가 `Loader`에 레지스트리를 넣는다. `exec`·`sh`·`env`는 `_native.process`로 내려가고 `prelude.fung`이 다시 내보낸다.

**Spec:** `06-native-and-stdlib.md` (네 층, 경계), `09-implementation-plan.md` (디렉터리), `04-modules.md` (우선순위).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**. Conventional Commits, `gofmt`·`go vet`·`go test ./...`(+`-race` vm/cmd) 통과, `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- **동작 불변:** 골든·단위 테스트의 기대값을 바꾸지 않는다(이동한 함수의 테스트는 위치만 옮긴다). 에러 번호 불변.
- **의존 방향:** `internal/*`은 `extras`를 import하지 않는다. `extras`는 `internal/vm`을 import하지 않는다.

## 계획 수준의 결정

1. core에 남는 Go 값 연산: `print` `eprint` `fail` `at_exit` `exit` `len` `map` `type_of` `dbg` `read_line` `read_lines` `args`, `filter` `fold` `each` `sort` `sort_by` `find` `any` `all` `group_by` `zip`?(→ fung) `unique`?(→ fung), `pairs` `put` `merge` `remove` `chars` `repeat` `split` `join` `trim` `upper` `lower` `replace` `starts_with` `ends_with` `int` `float` `str` `sym` `parse_int` `parse_float`.
2. fung extras(`extras/prelude.fung`)로 옮기는 것: `is_empty` `sum` `min` `max` `reverse` `range` `zip` `first` `last` `take` `drop` `contains` `flatten` `unique` `keys` `values` `get` `pad_left` `pad_right` `lines` `is_ok` `is_err`, 그리고 `exec` `sh` `env`의 재내보내기. `take`·`drop`은 슬라이스로(재귀 깊이 없이), `reverse`·`zip`은 인덱스 `for`로, 나머지는 `fold`·`match`로.
3. `fs`·`path`·`cli`·`prelude`의 fung 파일은 `extras/`로 옮기고 `internal/stdlib`은 지운다. 표시 경로는 `extras/fs.fung`(에러 메시지의 파일명).
4. `_native.process`: `exec` `sh` `env`. `args`는 값이라 core prelude에 남는다.
5. `frontend.Compile`의 기본 stdlib 제거. `dump`·REPL·`check`·`test`는 이미 `Loader`를 직접 쓰거나 cmd의 `allNatives`를 쓴다 → cmd에 `registry()` 하나로 모은다. vm 테스트는 `extras.Default()` + `vm.NativeModules()`로 `Loader`를 만들고 `prelude.fung`도 부트하는 테스트 도우미를 둔다(cmd의 `bootPrelude`와 같은 일; vm 테스트가 cmd를 import할 수 없어 복제).
6. 의존 방향 테스트: `tests/layers_test.go`가 `go list -deps`로 `internal/...`의 의존에 `extras`가 없고 `extras`의 의존에 `internal/vm`이 없음을 확인한다.

## Review Focus

1. 옮긴 함수의 에러 번호·메시지가 같다 (`take(1, 2)` E0506, `get({}, 1)` E0501, `pad_left("a", 3, "")` E0403 — fung에서는 `fail`로 못 내므로 **`fail` 대신 같은 상황을 Go 원시가 내게** 설계: `pad_*`는 `repeat`가, `get`은 인덱스가 낸다). 골든 `collections.fung`·`strings.fung`·`string_fns.fung`이 그대로 통과.
2. `prelude.fung`이 커져도 시작 시간이 눈에 띄게 늘지 않는다 (`fung -e 'print(1)'`가 50ms 안).
3. W0103이 `extras/prelude.fung` 자신에게 나지 않는다(내장 파일 경고 숨김)와 사용자 코드에서는 여전히 난다.
4. `go list -deps ./internal/...`에 `extras`가 없다.

---

### Task 1: `internal/nat` 도우미와 `internal/prelude` / `internal/native` 분리

- `internal/nat/nat.go`: `Def(name, fn, params...)`, `Param`, `Opt`, `Rest`, `Ok(v)`, `ErrValue(kind, msg, extra...)`, `NeedStr`, `NeedInt`, `NeedFn`, `NeedMap`, `NeedKey`, `Items(ctx, v, verb)`, `CallBool`, `NewList`, `JoinForPrint`.
- `internal/prelude`: `Core(args []string) map[string]value.Value` — 결정 1의 함수들(파일: `io.go` `collections.go` `strings.go` `convert.go`). 기존 테스트(`prelude_test.go` 등)를 옮긴다.
- `internal/native`: `Bridges() map[string]*value.Module` — `fs.go` `path.go` `process.go`(`_native.process`: exec, sh, env). 기존 `fs_test.go` 유지, process 테스트는 모듈을 통해.
- `cmd`·`frontend`·`repl`·vm 테스트의 `native.Prelude()`/`native.Modules()` 호출을 `prelude.Core`/`native.Bridges`로.
- 커밋 `refactor: split core prelude from native bridges`.

### Task 2: `extras/` 패키지와 조립

- `extras/extras.go`: embed, `Source(name)`, `type Registry struct{ Stdlib func(string) (string, bool); Natives map[string]*value.Module }`, `Default() Registry`(Source + `native.Bridges()`).
- fung 파일 이동(`git mv internal/stdlib/*.fung extras/`), `internal/stdlib` 삭제. 로더의 표시 경로 `extras/<name>.fung`.
- `frontend.Compile`에서 stdlib 기본값 제거(핵심은 extras를 모름). `cmd/fung`: `registry()` = `extras.Default()` + `vm.NativeModules()`; `bootPrelude`·`execute`·`check`·`test`·`repl`이 그것을 쓴다. vm 테스트: `testRegistry()`와 `bootTestPrelude(rt)` 도우미.
- `tests/layers_test.go` 추가(결정 6). 커밋 `refactor: move the embedded fung extras to extras/ and assemble them in cmd only`.

### Task 3: 조합 함수를 fung으로

- `extras/prelude.fung`에 결정 2의 함수들을 쓰고 Go 구현을 지운다. 테스트: 기존 vm `TestCollectionFunctions`·native 테스트 중 옮긴 함수의 케이스는 vm 테스트(prelude.fung 부트)로 옮긴다. 골든 불변.
- `fung -e 'print(1)'` 시작 시간 측정(10회 평균)을 노트에 적는다.
- 커밋 `refactor: write the prelude combinators in fung`.

### Task 4: 문서·노트

- `docs/notes/M7b-structure.md`: (1) 네 층과 기준(사용자의 기준 1·2를 그대로), (2) 왜 원소를 도는 함수는 Go에 남겼나(측정값), (3) Go 패키지 경계로 의존 방향을 강제하는 방법, (4) `extras/`가 프로젝트 extras로 이어지는 그림, (5) 옮기면서 발견한 것.
- 09 행 ` — 완료`, CLAUDE.md 상태. 커밋 `docs: add M7.5 structure notes and update status`.
