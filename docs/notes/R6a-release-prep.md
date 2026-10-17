# R6a 릴리스 전환 준비: 무엇을 했고 왜 그렇게 했나

2026-10-17. 계획 `docs/plans/2026-10-17-rust-release-prep-6a.md`, 스펙 `docs/superpowers/specs/2026-10-17-rust-release-prep-6a-design.md`,
결정과 남은 일은 `docs/design/open-questions.md`의 "Rust 이전"(6a단계 항목). 커밋 `8088d72..` 이 노트의 커밋까지.

5단계까지 Rust 구현은 Go 구현이 하는 일을 모두 같은 출력으로 하게 됐다. 그래도 아직 "내보낼 수 있는" 상태는 아니었다.
- 예제와 매뉴얼은 여전히 `atom`과 Go 구현을 설명했다.
- 릴리스 스크립트는 Go 바이너리를 만들었다.
- 0.2.0에서 무엇이 바뀌는지 사용자에게 알려 줄 CHANGELOG가 비어 있었다.

6단계는 둘로 나눴다(2026-10-17 결정). **6a**는 전환 준비이고 Go는 끝까지 정답지로 남는다. **6b**는 Go를 지우고, Go와 비교하던 테스트를
얼린 기대값으로 바꾼다. 0.2.0 태그는 6b 뒤에 붙인다. 지우기 전에 내보낼 준비를 먼저 끝내 두면, Go가 있는 동안 모든 변경을 Go와 대조해 볼 수 있다.

| 작업 | 커밋 |
|---|---|
| 1. `fan_in`·`pmap`·`retry`를 `conc` 모듈로 | `e9c39c8` |
| 2. 예제·매뉴얼을 Rust(0.2.0) 기준으로, `manual.rs` 검사 | `a42db83` |
| 3. Rust 릴리스 파이프라인(Linux musl 정적) | `28a9a09`, `e1a99b1` |
| 4. CHANGELOG `[Unreleased]` | `999df33` |
| 5. Go·Rust 벤치마크 기록 | `7cdb041` |
| 6. 문서(이 노트, open-questions, 설계 문서의 남은 `atom` 표현, `release.md`) | 이 노트의 커밋 |

## 1. prelude에 무엇을 둘까: "확실한 것만"

### 원칙

prelude는 `import` 없이 보이는 이름이다. 여기에 이름을 두는 결정에는 비대칭이 있다.
- 나중에 **더하는** 것은 안전하다. 사용자가 같은 이름을 이미 정의했다면 그 정의가 이기고 W0103 경고(prelude 이름을 가림)만 난다.
- 나중에 **빼는** 것은 호환성 깨짐이다. 그 이름을 부르던 프로그램이 E0204(정의되지 않은 이름)로 멈춘다.

그래서 원칙은 "prelude에는 확실한 것만"이다. 빼야 할 일이 생길 만한 이름은 처음부터 모듈에 둔다. 0.2.0은 `atom` 제거로 어차피 호환이 깨지는
릴리스라서, 위치를 고칠 거라면 지금이 가장 싸다(open-questions "Rust 이전"의 6단계 전환 직전 검토 항목).

### 결정: `conc` 모듈

`fan_in`·`pmap`·`retry` 셋을 Rust 구현에서 prelude → `conc` 모듈로 옮겼다(`extras/rust/conc.fung`, `import conc` 뒤 `conc.pmap(…)`).
- 셋 다 기본 연산이 아니라 **조합**이다. `spawn`·채널·`sleep`만으로 fung 코드로 짜여 있고(구현 언어 지침의 "핵심이 아닌 조합 → fung"),
  사용자가 같은 것을 직접 짤 수 있다.
- 셋 다 **정책**을 품는다. `pmap`의 동시 실행 수(`limit`), `retry`의 횟수와 기다리는 시간, `fan_in`이 결과를 모으는 순서. 이런 선택은
  쓰다 보면 바꾸고 싶어지기 쉽다. 모듈 안이면 `conc` 모듈의 판을 바꾸거나 함수를 더하면 되지만, prelude 안이면 그 이름 자체가 묶인다.
- R7 스케줄러(그린 스레드)가 들어오면 동시성 도구의 모양이 다시 바뀔 수 있다. 그때 움직일 여지를 남긴다.

이름 그대로 부르면 E0204이고, 도움말은 Go가 모르는 이름에 내는 문구와 같다(`pmap` → "a similar name exists: `map`").

### 남긴 것과 이유

| 이름 | 남긴 이유 |
|---|---|
| `until`·`iterate` | fung에는 `while`이 없다. 이 둘이 반복문의 대체라서 빠지면 다른 것을 만들기 힘들어진다(핵심의 정의). |
| `exec`·`sh` | fung은 셸 스크립트를 대신하려는 언어다. 명령 실행은 그 용도의 중심이라 매번 `import`하게 할 이유가 없다. |
| `remove`·`close` | `fs.remove`·`fs.close`와 이름이 겹치지만 모듈 이름이 붙어 헷갈릴 일이 적다. 매뉴얼에 구분하는 한 줄을 두었다. |

### Go와 같이 쓰는 파일에서 이름 숨기기

`extras/prelude.fung`는 Go와 Rust가 함께 읽는다. 파일을 나누면 Go를 고쳐야 하므로(이 단계는 Go를 고치지 않는다) Rust 쪽에서 두 겹으로 숨겼다.
1. 앞단: `crates/fung-front/src/builtins.rs`의 `RUST_PRELUDE_FUNG_MOVED`가 세 이름을 prelude 이름 표에서 뺀다. resolve가 이 표를 보므로 E0204가 난다.
2. 부팅: `boot_file(..., hidden)`이 `prelude.fung`의 공개 이름을 prelude 표에 옮길 때 세 이름을 건너뛴다. 테스트가 부팅한 표와 앞단의 표가 같은지 확인한다.

대가로 Rust 바이너리는 `prelude.fung` 안의 Go용 사본 셋을 여전히 컴파일하고 부팅 때 돌린다(함수 몇 개). 6b에서 Go를 지우면 `prelude.fung`에서
지우고 목록도 없앤다(open-questions의 6b 할 일).

## 2. 매뉴얼과 예제를 Rust 기준으로

### 예제

`examples/api.fung`·`serve.fung`은 `atom(0)` → `agent(0)`, `checkurls.fung`는 `import conc` + `conc.pmap`. 셋은 이제 Go가 돌릴 수 없다.
그래서 "Go도 돌릴 수 있는가"를 파일 내용으로 판정하는 `uses_rust_only`(`agent(`나 `import conc` 줄)와 손으로 적은 목록 `RUST_ONLY_EXAMPLES`를 두고,
테스트(`the_rust_only_examples_are_listed`)가 둘이 양쪽으로 같은지 본다. 목록에 든 예제는 Go와 `dump ast`·`fmt`만 비교하고(문법은 같다),
`check`·`test`·`dump core/bytecode` 비교에서는 빠지며, Rust 혼자 `fung test examples`(12개 통과)로 지킨다.

### 매뉴얼

`docs/*.html`을 0.2.0 기준으로 고쳤다. `atom` 절 → `agent`, `conc` 절, Rust로 쓴 HTTP의 약속(64 MiB 상한 등), E0704·E0705, `fung test`·REPL 규칙,
`internals.html`은 Rust 구현(`enum Value`, 실행권, 크레이트 구조)으로. 매뉴얼의 코드 예제 검사는 Go와 비교하던 것을 Rust 혼자의 검사로 바꿨다
(`tools_parity::manual_examples_check_format_and_run`: 예제마다 `check`, `fmt --check`, 실행이 모두 종료 코드 0). 건너뛰는 예제도, 허용 오차도 없다.

### 새 검사 `manual.rs`

함수 참조(`builtins.html`)는 128개 함수의 서명을 손으로 적은 문서다. 그동안 **이 문서를 읽는 테스트가 하나도 없었다.** 서명은 Go의 매개변수 표를 보고
옮겨 적었을 뿐이다. `crates/fung-cli/tests/manual.rs`는 기대하는 표를 **Rust 구현 자신의 표**에서 만든다.
- 네이티브 함수: `fung_vm::prelude::core()`, `fung_vm::conc::natives()`, `fung_vm::boot::native_module(..)`의 매개변수 이름·기본값·나머지 인자.
- fung으로 쓴 함수: Rust 부팅과 로더가 embed한 소스(`fung_front::extras::source`, `extras::AGENT`)의 최상위 `fn` 줄, 부팅이 숨기는 이름(1절)을 뺌.

그리고 문서의 `<div class="fn" id="fn-…">`마다 서명이 정확히 같은지(타입 표기는 빼고 기본값은 쓴 그대로), "fung으로 작성" 표시가 정확히 fung 함수에만
있는지, prelude와 모듈(fs·path·cli·json·time·regex·http·conc)의 공개 이름이 빠짐없이 그리고 그것만 문서에 있는지, 함수 개수 문구(128)가 맞는지 본다.
켜자마자 Rust와 어긋난 곳이 여럿 나왔다(빠진 `agent`·`conc` 항목, 남은 `atom`·`pmap`, 틀린 "fung으로 작성" 표시).

배운 점: **사람이 옮겨 적은 문서는 원본과 반드시 어긋난다.** 원본(구현의 표)에서 기대값을 만들어 문서와 비교하는 테스트가 있어야 문서가 계속 맞는다.
같은 생각이 CHANGELOG와 매뉴얼 사이드바 버전의 짝을 검사하는 `tests/changelog_test.go`에도 있다.

## 3. 정적 링크: 왜 musl인가

### 무엇을 지키려는가

0.1(Go)은 `CGO_ENABLED=0`으로 빌드해 C 라이브러리에 기대지 않는 정적 바이너리였다. 받은 파일 하나가 CentOS 7이든 Alpine이든 최신 Ubuntu든 그대로 돈다.
SRE가 쓰는 스크립트 언어에서 이 성질은 크다. 오래된 서버, 최소 컨테이너, 배포판이 섞인 환경에 "파일 하나 복사"로 끝나야 한다.

Rust는 기본으로 그렇지 않다. Linux의 기본 대상(`x86_64-unknown-linux-gnu`)은 glibc에 **동적으로** 링크한다.
그러면 빌드한 기계의 glibc보다 오래된 glibc가 있는 서버에서는 `version 'GLIBC_2.34' not found` 같은 에러로 아예 뜨지 않는다.

### glibc와 musl

Linux 프로그램이 쓰는 C 라이브러리(libc)는 둘이 흔하다.

| | glibc | musl |
|---|---|---|
| 쓰는 곳 | 거의 모든 일반 배포판(Ubuntu, Debian, RHEL…) | Alpine, 정적 바이너리 |
| 정적 링크 | 공식적으로 권하지 않음. 이름 조회(`getaddrinfo`)·사용자 조회가 NSS(`/etc/nsswitch.conf`)로 공유 라이브러리를 실행 중에 `dlopen`하므로, 정적으로 묶어도 같은 버전의 glibc 공유 라이브러리가 서버에 있어야 제대로 돈다 | 처음부터 정적 링크를 염두에 둔 설계. 이름 조회가 `/etc/hosts`와 `/etc/resolv.conf`를 직접 읽는다 |
| 할당기 | 빠르고 스레드에 강함 | 단순하고 작음. 할당이 많은 다중 스레드 작업에서 느리다는 보고가 많다 |

fung은 `http`·`net` 때문에 `getaddrinfo`를 부른다(`native/sock.rs`). glibc를 정적으로 묶으면 바로 그 함수가 위의 함정에 걸린다.
그래서 Linux 릴리스는 Rust의 musl 대상(`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`)으로 만든다. Rust의 musl 대상은 musl의 `libc.a`와
시작 코드(crt)를 툴체인에 함께 담고 있어("self-contained") 정적 실행 파일이 기본이다. 정확히는 static-pie(위치 독립 정적 실행 파일)로,
동적 링커(`INTERP`)도 필요한 공유 라이브러리(`NEEDED`)도 없다. `check-release.sh`가 `readelf`로 바로 이것을 확인한다.

대가:
- musl의 이름 조회는 nsswitch를 읽지 않는다. `/etc/hosts`와 DNS 밖의 조회 경로(LDAP, mDNS, systemd-resolved의 NSS 모듈)를 쓰는 환경에서는 결과가 다를 수 있다.
- 할당기 속도. fung은 값마다 `Rc`를 만들어 할당이 많다. musl 정적 빌드의 속도는 **아직 재지 못했다**(5절). 느리면 할당기를 바꾸는 선택지가 있지만
  크레이트 허용 목록 결정이 필요하다.
- macOS는 해당 없다. macOS는 시스템 라이브러리(`libSystem`)를 늘 동적으로 링크하게 하고, 그 ABI를 운영체제가 지켜 준다.

### `ring`의 C 코드: 플랫폼별 러너의 이유

TLS(`rustls`)의 암호 공급자 `ring`은 C와 어셈블리 코드를 품고, 빌드 스크립트가 `cc` 크레이트로 그것을 컴파일한다. 그래서 Rust 컴파일러만으로는 안 되고
**대상용 C 컴파일러**가 있어야 한다. Go는 `GOOS`·`GOARCH`만 바꾸면 한 기계에서 네 대상을 만들었지만, Rust 구현은 교차 C 도구를 갖추거나
대상마다 그 CPU의 기계에서 빌드해야 한다. 앞쪽을 고르면 도구(교차 gcc, osxcross 등)라는 짐이 생기므로 GitHub Actions의 플랫폼별 러너를 골랐다
(open-questions 배포 결정).

musl 대상의 C 컴파일러는 Ubuntu `musl-tools`의 `musl-gcc`다. 보통 gcc에 musl 헤더와 라이브러리를 쓰게 하는 감싸개라서 x86_64와 arm64 러너 모두에서 같다.
`ring`은 독립 실행(freestanding) 헤더 정도만 쓰고 `.S` 파일도 컴파일러로 어셈블하므로 이것으로 충분하다.

### libutil 문: 테스트 도우미가 릴리스 링크를 깰 뻔했다

5단계의 가짜 터미널 도우미(`fung_vm::term::pty`, 테스트 전용이지만 라이브러리 안에 있음)는 `openpty`를 쓰려고 Linux에서 `libutil`을 링크했다.

```rust
#[cfg(target_os = "linux")]
#[link(name = "util")]
unsafe extern "C" {}
```

비어 있는 `extern` 블록이라도 `#[link]`가 붙어 있으면 **이 크레이트를 링크하는 모든 실행 파일**에 `-lutil`이 붙는다. 명령이 `pty`를 한 번도 부르지
않아도 `fung` 바이너리가 `libutil`을 찾는다. glibc에서는 문제가 없다(glibc 2.34부터 `openpty`는 libc 안에 있고 `libutil`은 빈 껍데기로 남아 있다).
musl은 `openpty`를 `libc.a` 안에 두고 `libutil`이 없으며, Rust의 self-contained musl도 `libutil.a`를 싣지 않는다. 그대로 두면 musl 링크가 실패한다.

고친 것: 조건을 `#[cfg(all(target_os = "linux", target_env = "gnu"))]`로 좁혔다. `libc` 크레이트는 musl에서 `openpty`를 자기 `#[link]` 없이 선언하므로
musl에서는 libc 자체에서 찾는다. glibc 빌드와 테스트는 이전과 똑같다. 5단계 노트가 "6단계에서 확인한다"고 남겨 둔 바로 그 항목이다.

## 4. 릴리스 파이프라인

자세한 절차는 `docs/notes/release.md`(2·6절)에 있다. 여기서는 설계의 이유만 적는다.

### 플랫폼별 러너

| 묶음 | 러너 | 대상 |
|---|---|---|
| linux amd64 | `ubuntu-24.04` | `x86_64-unknown-linux-musl` |
| linux arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-musl` |
| darwin arm64·amd64 | `macos-latest`(Apple 실리콘) | `aarch64-apple-darwin`, `x86_64-apple-darwin` |

macOS 두 개는 한 러너에서 만든다. Xcode의 도구가 x86_64도 대상으로 삼고, Intel macOS 러너는 GitHub이 거둬들이는 중이라 기대면 어느 날 릴리스가 깨질 수 있다.
대가는 x86_64 바이너리를 Rosetta가 있을 때만 실제로 돌려 본다는 것이다. 없으면 `file`로 CPU 종류만 확인한다(아래 아키텍처 검사).

묶음 이름(`fung_<ver>_<os>_<arch>.tar.gz`)·`SHA256SUMS`·빌드 증명·`install.sh`는 0.1과 같다. 사용자 쪽에서 보면 설치 방법이 바뀌지 않는다.
`tests/install_test.go`는 가짜 릴리스를 스스로 만들어 `install.sh`를 검사하므로 Go를 고칠 필요가 없었다.

### 아무것도 게시하지 않는 시험 실행

릴리스 워크플로는 태그를 푸시해야만 돌면 시험할 길이 없다. 처음 고친 워크플로를 처음 돌리는 날이 곧 릴리스 날이 된다.
그래서 `workflow_dispatch`(손으로 실행)를 더했다: `gh workflow run release.yml -f version=0.0.0-dryrun`. 같은 과정을 돌리고 결과를 아티팩트로 남긴다.

여기서 리뷰가 중요한 것을 잡았다. 처음 판은 시험 실행에서도 빌드 증명(`attest-build-provenance`)을 남겼다. 빌드 증명은 Sigstore의 **공개 로그에 영구히**
기록되므로, 시험할 때마다 `0.0.0-dryrun` 묶음에 대한 공개 기록이 쌓인다. 고친 판은 작업을 넷으로 나눴다.
- `test` → `build` → `release`: 읽기 권한만. 묶음을 모아 `SHA256SUMS`를 쓰고 검사해 아티팩트로 남긴다. 시험 실행은 여기서 끝난다.
- `publish`: 쓰기 권한(`contents`·`id-token`·`attestations`)을 가진 **유일한** 작업이고, 태그 푸시일 때만 돈다(작업 수준 `if`). 증명과 게시 단계에도
  같은 `if`를 한 번 더 걸어, 나중에 누가 단계를 옮겨도 시험 실행이 공개 기록을 남기지 않게 했다.

권한을 필요한 작업 하나에만 주는 것(최소 권한)은 보안의 기본이기도 하다. 테스트나 빌드 단계에서 무엇이 잘못 돌아도 릴리스를 만들거나 서명할 수 없다.

### 아키텍처 검사

`scripts/check-release.sh`는 묶음마다 이름·내용·해시·정적 링크·실행을 본다. 처음 판의 TDD에서 흥미로운 것이 보였다. **옛 Go 스크립트로 만든 묶음은
이름·내용·해시 검사를 모두 통과했다.** 실패한 것은 마지막의 실행 검사(`import conc; print(conc.pmap(…))`가 Go에서 E0301) 하나뿐이었다.
그런데 실행 검사는 그 CPU에서 돌릴 수 있을 때만 한다. 리뷰는 "macOS 러너가 x86_64용이라며 arm64 바이너리를 묶어도 Rosetta가 없으면 아무도 모른다"를
지적했고, 이제 `file -b`로 바이너리의 형식과 CPU를 대상과 대조한다(Mach-O arm64/x86_64, ELF x86-64/aarch64). 돌려 볼 수 없는 바이너리도 최소한
"맞는 종류의 파일"인지는 확인한다. ELF가 아닌 파일에 `readelf`를 쓰면 엉뚱한 결과가 나오므로 `readelf -h`가 먼저 성공해야 한다.

`ci.yml`에도 `musl` 작업을 더했다. PR마다 linux/amd64 musl 묶음을 만들고 검사하므로, glibc에만 링크되는 것이나 새 C 의존성이 끼면 릴리스 날이 아니라 그 PR에서 깨진다.

## 5. 벤치마크 읽는 법

`bench/README.md`의 표(Apple M2 Max, macOS, 9회 중앙값, 초):

| | Go | Rust | Rust / Go |
|---|---|---|---|
| `fib.fung` 재귀 호출 | 0.204 | 0.197 | 0.97 |
| `loop.fung` 산술 반복 | 0.241 | 0.217 | 0.90 |
| `map.fung` 맵 20만 개 | 0.461 | 0.370 | 0.80 |
| `str.fung` 문자열 잇기 | 0.057 | 0.073 | 1.28 |
| `chan.fung` 채널 주고받기 10만 번 | 0.090 | 0.258 | 2.87 |
| `spawn.fung` 태스크 2,000개 | 0.010 | 0.127 | 12.7 |

읽는 법:
- **계산(fib·loop·map)은 비슷하거나 빠르다.** Go 구현은 1·2단계 최적화(할당 0 호출 등)를 거친 뒤이고, Rust는 값이 상자 없는 `enum`이라 그 최적화를
  처음부터 갖고 시작했다. 맵이 가장 많이 빨라진 것은 영속 구조의 경로 복사가 `Rc::make_mut`로 참조가 하나일 때 제자리 수정이 되는 덕으로 보인다.
- **`str`은 1.28배 느리다.** 원인은 아직 분석하지 않았다. 숫자 하나 걸러 문자열 하나를 만드는 작업이라 할당과 문자열 복사 경로가 후보다.
- **`spawn`과 `chan`이 느린 것은 운영체제 스레드 때문이다.** 3a에서 태스크마다 운영체제 스레드 하나 + 실행권 잠금 하나를 골랐다(R3a). 태스크를 만들 때마다
  스레드를 만들고(1 GiB 스택 예약 포함), 채널로 값을 넘길 때마다 실행권이 스레드 사이를 오가며 운영체제가 문맥을 바꾼다. Go의 고루틴은 런타임이 직접
  주차·재개하는 작은 단위라 이 비용이 없다.
- **비율보다 단위 비용이 정확하다.** 표의 시간은 프로세스 시작부터 끝까지라 시작 비용(수 ms)이 들어 있다. Go의 `spawn` 0.010초는 대부분 시작 비용이다.
  시작 비용을 빼고 나누면 Rust는 태스크 하나에 약 58 µs(`(0.127 − 0.010) / 2000`), 채널 한 번에 약 2.5 µs로 R3a 9절의 측정(약 55 µs, 약 2.5 µs)과 맞는다.
  태스크 생성만 놓고 보면 Go(약 2 µs)의 약 25배다.
- **이것이 R7의 동기다.** 계산은 이미 충분하고, 남은 큰 차이는 동시성 비용과 스레드 천장(에이전트 수천 개면 E0801)이다. 둘 다 태스크를 운영체제
  스레드가 아니라 VM이 주차·재개하는 단위로 바꾸면(그린 스레드) 풀린다. 이 표는 그때의 기준선이다(open-questions "R7 스케줄러").

주의할 점: 기계 하나, macOS 동적 링크 빌드에서 잰 값이다. 릴리스되는 Linux musl 빌드의 열은 비어 있다. 이 기계에 musl 대상도 교차 도구도 없어 만들 수 없었고,
추정치는 적지 않았다. musl 할당기가 `map`·`str`을 얼마나 늦추는지는 Linux에서 재야 안다(3절).

## 6. 실제로 부딪힌 함정

- **빈 `extern` 블록의 `#[link]`가 실행 파일에도 붙는다.** 테스트 도우미 때문에 넣은 `-lutil`이 릴리스 바이너리의 musl 링크를 깰 뻔했다(3절).
- **`cc`가 고르는 musl C 컴파일러 이름이 러너에 없다.** `cc` 크레이트는 aarch64 musl 대상에 `aarch64-linux-musl-gcc`를 찾는데, Ubuntu `musl-tools`가 주는 것은
  `musl-gcc` 하나다. 빌드 스크립트가 호스트와 같은 CPU의 musl 대상이면 `CC_<대상>=musl-gcc`를 넣는다(이미 설정돼 있으면 그대로 둠).
- **`RUSTFLAGS`는 공백으로 나뉜다.** 경로를 지우는 `--remap-path-prefix=<저장소>=/fung`의 저장소 경로에 공백이 있으면 플래그가 쪼개진다.
  `CARGO_ENCODED_RUSTFLAGS`(0x1f로 구분)로 넘긴다. `--target`을 주면 `RUSTFLAGS`가 빌드 스크립트에는 가지 않는 것도 알아 둘 점이다(대상 코드에만 적용).
- **macOS의 `tar`는 확장 속성을 `._fung` 같은 숨은 파일로 넣는다.** Linux에서 풀면 정체 모를 파일이 생긴다. `COPYFILE_DISABLE=1`과 `tar --no-xattrs`.
- **출력 디렉터리를 비우는 `rm`.** 실수로 엉뚱한 디렉터리를 주면 지워진다. 디렉터리 안이 모두 `fung_*`이나 `SHA256SUMS`일 때만 진행하고, 아니면 아무것도 지우지 않고 멈춘다.
- **시험 실행이 공개 기록을 남긴다.** 빌드 증명은 Sigstore 공개 로그에 영구히 남는다(4절). "게시만 빼면 된다"가 아니었다.
- **옛 바이너리가 검사 대부분을 통과한다.** 이름·내용·해시는 무엇이 들었는지 모른다. 돌려 볼 수 없는 대상에는 `file`로 CPU 종류를 확인한다(4절).
- **아무도 문서를 검사하지 않고 있었다.** `builtins.html`은 손으로 적은 서명이었고, 검사를 켜자마자 어긋난 곳이 나왔다(2절).
- **Go 테스트가 예제를 돌리고 있었다.** 예제를 `agent`·`conc`로 고치자 Go의 `TestExamplesPassTheirTests`가 세 파일에서 `<load>` 실패를 냈다.
  Go가 돌리는 사본에서 Rust 전용 예제 셋을 뺐다(이 단계의 유일한 Go 테스트 수정; Go 코드는 고치지 않음).
- **`import conc`를 쓰는 프로그램을 Go와 비교하기.** Go에는 `conc` 모듈이 없다. 테스트 도우미 `go_source`가 Go에 넘길 때 `import conc` 줄을 **빈 줄로**
  바꾸고 `conc.`를 지운다. 줄을 지우지 않고 비우는 것은 에러 위치의 줄 번호가 두 구현에서 같게 하려는 것이다.
- **4 GiB 스택 예약과 `ulimit -v`.** fung은 프로그램을 4 GiB 스택을 예약한 스레드에서 돌린다(1단계부터, 깊은 재귀를 Go의 1 GB 고루틴 한도보다 넉넉히).
  가상 메모리를 엄격히 제한한 환경에서는 이 예약이 거부된다. CHANGELOG를 쓰면서 코드를 다시 보니, 실행 스레드를 만들지 못하면 `expect`가 패닉하는데
  `main`이 그보다 먼저 패닉 메시지를 끄는 훅(VM의 패닉 울타리가 E0801을 찍으므로)을 걸어 두어 **아무 메시지 없이 종료 코드 101**로 끝난다.
  CHANGELOG "알려진 차이"에 적고, 메시지를 내도록 고치는 일은 open-questions에 남겼다.

## 7. 다음

6b: Go 구현을 지우고, Go와 비교하던 테스트를 얼린 기대값 파일로 바꾸고, `spec/builtins.txt`·`errors.txt`를 Rust 코드에서 생성한다.
그 뒤 브랜치를 main에 스쿼시 머지하고 0.2.0 태그를 붙인다(그때 사용자 확인). 태그 전에 릴리스 워크플로를 시험 실행으로 한 번 돌려 본다.
그 다음이 R7 스케줄러다. 목록은 open-questions "Rust 이전"의 6a·6b 항목.
