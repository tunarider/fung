# 배포 — 무엇을 만들었고 왜 그렇게 했나

2026-10-14. fung을 저장소 밖의 사람이 "개발 도구 없이" 설치하게 하는 길. 결정: MIT 라이선스, 첫 버전 `v0.1.0`,
대상 Linux·macOS × amd64·arm64, 직접 쓴 빌드 스크립트 + GitHub Actions + 설치 스크립트.

2026-10-17 개정(Rust 이식 6a단계): 0.2.0부터 릴리스하는 실행 파일은 Rust 구현이다. 1~3절과 6절을 Rust 파이프라인으로 고쳤다.
묶음 이름·`SHA256SUMS`·빌드 증명·`install.sh`(4절)는 그대로다. 왜 musl인지, 함정은 무엇이었는지는 `docs/notes/R6a-release-prep.md`.
0.1(Go)의 방식은 각 절 끝에 짧게 남겼다.

## 1. 무엇을 배포하나: 정적 바이너리 하나

스크립트 언어를 배포할 때 가장 큰 짐은 보통 "런타임 + 표준 라이브러리 파일들"이다(Python의 `lib/`, Node의 `node_modules`).
fung은 처음부터 두 결정 덕에 이 짐이 없다.

- **Linux 바이너리는 정적 링크다.** Rust의 musl 대상(`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`)으로 빌드하면
  C 라이브러리(musl `libc.a`)까지 실행 파일 안에 들어간다. 어느 리눅스 배포판에서든 그 파일 하나로 돈다. glibc 버전이 안 맞아 안 뜨는 문제가 없다.
  macOS는 운영체제가 정적 링크를 허락하지 않으므로(시스템 라이브러리 `libSystem`은 늘 동적) 기본 대상으로 빌드한다. macOS 안에서는 그것이 표준이다.
- **extras를 embed한다**(M5·M7.5). `prelude.fung`·`fs`·`http`·`conc`·`agent` 같은 fung 소스가 바이너리 안에 들어 있다(Rust는 `include_str!`).

그래서 릴리스 산출물은 플랫폼마다 `fung` 파일 하나(+ LICENSE, README)를 묶은 tar.gz다.

0.1(Go): Go 표준 라이브러리만 쓰고 `CGO_ENABLED=0`으로 빌드해 같은 결과(정적 바이너리)를 얻었다.

## 2. 플랫폼별 러너에서 빌드한다

Go는 `GOOS`·`GOARCH`만 바꾸면 한 기계에서 네 대상을 다 만들었다. Rust 구현은 그렇게 못 한다. TLS의 암호 부분(`ring` 크레이트)이
C와 어셈블리 코드를 품고 있어서, 빌드할 때 **대상용 C 컴파일러**가 필요하기 때문이다. 교차 C 도구를 갖추는 대신 대상마다 그 CPU·OS의
GitHub Actions 러너에서 빌드한다(open-questions "Rust 이전"의 배포 결정).

| 묶음 | 러너 | Rust 대상 |
|---|---|---|
| `linux_amd64` | `ubuntu-24.04` | `x86_64-unknown-linux-musl` (C 컴파일러는 `musl-tools`의 `musl-gcc`) |
| `linux_arm64` | `ubuntu-24.04-arm` | `aarch64-unknown-linux-musl` (같음) |
| `darwin_arm64`, `darwin_amd64` | `macos-latest`(Apple 실리콘) | `aarch64-apple-darwin`, `x86_64-apple-darwin` |

macOS 두 개는 한 러너에서 만든다. Xcode의 clang·링커가 x86_64도 대상으로 삼기 때문이다(Intel macOS 러너는 GitHub이 거둬들이는 중이다).
대가로 x86_64 바이너리는 러너에 Rosetta가 있을 때만 실제로 돌려 보고, 없으면 형식(Mach-O x86_64)·내용·해시만 확인한다.

빌드는 `scripts/build-release.sh <버전> [출력 디렉터리]`가 한다. `TARGETS`(기본은 네 개 전부, 예 `TARGETS=darwin/arm64`)로 대상을 고르고,
대상마다 `cargo build --release --locked -p fung-cli --target <대상>`을 돌린다. 설치되지 않은 대상이 하나라도 있으면 아무것도 만들기 전에
`rustup target add …` 줄을 알려 주고 멈춘다(일부만 만든 릴리스가 조용히 나가지 않게). 빌드 옵션은 Go 때의 것과 짝을 이룬다.

| 하는 일 | Rust | 0.1(Go) |
|---|---|---|
| 바이너리에 내 컴퓨터 경로를 남기지 않기 | `--remap-path-prefix=<저장소>=/fung`, `$CARGO_HOME` → `/cargo` | `-trimpath` |
| 디버그 심볼 빼기 | `CARGO_PROFILE_RELEASE_STRIP=symbols` | `-ldflags "-s -w"` |
| 버전 박기 | 컴파일할 때 `FUNG_BUILD_VERSION` 환경 변수(3절) | `-ldflags "-X main.linkedVersion=…"` |

경로가 공백을 품어도 깨지지 않게 플래그는 `RUSTFLAGS`가 아니라 `CARGO_ENCODED_RUSTFLAGS`(0x1f로 구분)로 넘긴다.
macOS에서 만든 묶음에 AppleDouble(`._fung`)·확장 속성이 끼지 않게 `COPYFILE_DISABLE=1`과 `tar --no-xattrs`를 쓴다.
`SHA256SUMS`는 `scripts/sha256sums.sh`가 따로 쓴다. 세 러너의 묶음을 한곳에 모은 뒤에 한 번 써야 하기 때문이다.

## 3. 버전은 어디서 오나

`crates/fung-cli/src/version.rs`가 고른다.
1. 릴리스 빌드가 컴파일할 때 넣은 `FUNG_BUILD_VERSION`(`option_env!`로 읽는다; 앞의 `v` 하나는 뗀다). cargo는 이 변수를 빌드 입력으로 추적하므로
   값이 바뀌면 다시 컴파일한다. 이름이 `FUNG_VERSION`이 아닌 것은 `install.sh`의 `FUNG_VERSION`(설치할 버전 고르기)과 겹치지 않게 하려는 것이다(6a 마무리).
   `crates/fung-cli/tests/build_version.rs`가 지키고, CI는 이 변수를 넣고 한 번 더 돌린다.
2. 없으면 `0.0.0-dev`. 크레이트 버전(`CARGO_PKG_VERSION`)은 쓰지 않는다. 작업 공간 크레이트는 모두 `0.0.0`이라 뜻이 없다.

0.1(Go)은 `-X`로 넣은 값 → `go install …@v0.1.0`이 기록한 모듈 버전 → `0.0.0-dev` 순이었다. `go install` 길은 0.2.0에서 사라졌다.

**왜 0.x인가.** 유의적 버전(SemVer)에서 `0.x`는 "아직 공개 API가 안정되지 않음"을 뜻한다. 마이너 버전(`0.1` → `0.2`)이 올라갈 때
호환이 깨질 수 있다. 문법을 아직 바꿀 수 있는 언어에 맞는 약속이다. `1.0`부터는 메이저 버전에서만 깨뜨린다. 0.2.0이 바로 그 예다(`atom` 제거).

## 4. 받은 파일을 믿을 수 있나

`curl … | sh`는 편하지만 "인터넷에서 받은 것을 그대로 실행"이다. 위험은 두 층이다.
- **전송 중 변조·깨진 다운로드**: 릴리스에 `SHA256SUMS`를 같이 올리고, `install.sh`가 받은 tar.gz의 해시를 대조한다.
  다르면 아무것도 설치하지 않는다. `tests/install_test.go`가 일부러 틀린 해시로 이 거부를 확인한다.
- **릴리스 자체가 가짜**: 해시 파일도 같은 곳에서 오므로 해시만으로는 못 막는다. 그래서 Actions가 **빌드 증명**(build provenance,
  SLSA)을 남긴다(`actions/attest-build-provenance`). "이 파일은 tunarider/fung 저장소의 이 커밋에서 이 워크플로가 만들었다"는
  서명된 기록이고, `gh attestation verify <파일> --repo tunarider/fung`로 누구나 확인한다. 서명 키를 직접 관리하지 않아도 된다
  (GitHub의 OIDC 토큰으로 서명).

설치 스크립트의 다른 선택:
- `~/.local/bin`이 기본이라 sudo가 필요 없다. `curl | sh`에 root 권한을 주지 않는 것이 안전하다.
- 새 파일을 옆에 복사한 뒤 `mv`로 바꾼다. 같은 파일 시스템 안의 `mv`는 원자적이라, 실행 중인 fung이 반쯤 쓰인 파일을 보지 않는다.
- 최신 버전은 `github.com/…/releases/latest`가 `…/tag/v0.1.0`으로 넘겨 주는 주소에서 읽는다. GitHub API를 쓰지 않아 호출 한도·토큰 문제가 없다.

## 5. macOS의 Gatekeeper

브라우저로 받은 파일에는 macOS가 "인터넷에서 옴" 표시(quarantine 속성)를 붙이고, 서명·공증이 없으면 실행을 막는다.
`curl`로 받은 파일에는 이 표시가 붙지 않아 설치 스크립트 경로는 문제가 없다. 서명·공증은 Apple 개발자 계정(유료)이 필요해 보류했다.

## 6. CI와 릴리스를 나눈 이유

- `ci.yml`: main·작업 브랜치(`claude/**`) 푸시와 PR마다 커밋 전 검사와 같은 것을 돈다.
  - `rust`(Linux·macOS): Linux에서 `cargo-deny`·`cargo-audit`, 두 운영체제에서 `cargo fmt --check`, clippy, 디버그·릴리스 테스트.
    (6b 전에는 Go 구현을 검사하는 `test` 작업도 있었다.)
  - `musl`(Linux): 릴리스와 같은 스크립트로 linux/amd64 musl 바이너리를 만들고 `check-release.sh`로 검사한다. C 의존성이 새로 생기거나 glibc에만
    링크되는 것이 끼면 릴리스 날이 아니라 그 PR에서 깨진다.
- `release.yml`: 네 작업이 차례로 돈다.
  1. `test`: 태그 모양(`vX.Y.Z`) 확인, 릴리스 노트 뽑기(절이 없으면 여기서 멈춤), 테스트(디버그·릴리스).
  2. `build`: 위 표의 러너 셋에서 묶음을 만들고 그 자리에서 `check-release.sh`로 검사한다. Linux 작업은 `file`·`ldd`·`readelf` 출력도 기록에 남긴다.
  3. `release`: 묶음 넷을 모아 `SHA256SUMS`를 쓰고 넷을 함께 다시 검사한 뒤 `release` 아티팩트로 남긴다. 읽기 권한만 있다.
  4. `publish`: **태그를 푸시했을 때만** 돈다. 쓰기 권한(`contents`·`id-token`·`attestations`)은 이 작업에만 있다. 받은 파일을 한 번 더 검사하고,
     빌드 증명을 남기고, `gh release create`로 게시한다.
- **태그 없이 시험하기**: Actions에서 release 워크플로를 손으로 실행하거나 `gh workflow run release.yml -f version=0.0.0-dryrun`.
  `test`~`release`까지 같은 과정을 돌리고 결과(묶음·`SHA256SUMS`·`[Unreleased]` 절의 노트)를 아티팩트로 남긴다. `publish`는 돌지 않으므로
  릴리스도, 공개 로그(Sigstore)에 영구히 남는 빌드 증명도 생기지 않는다.
- `scripts/check-release.sh <버전> [디렉터리]`가 묶음마다 보는 것: 묶음이 있고 다른 묶음은 없는지, 안에 `fung`·`LICENSE`·`README.md`만 한 폴더에 있는지,
  `fung`이 그 대상의 CPU용인지(`file`로: Mach-O arm64/x86_64, ELF x86-64/aarch64), `SHA256SUMS` 줄이 맞는지, 실행 권한, Linux는 정적인지
  (`readelf`로 `INTERP`·`NEEDED`가 없음; 없으면 `file`의 "statically linked"/"static-pie linked"), 돌릴 수 있는 기계면 `fung version`과
  `conc.pmap` 한 줄(Rust 바이너리이고 스레드가 도는지).
- 빌드 로직은 워크플로 YAML이 아니라 스크립트에 둔다. 로컬에서 똑같이 돌려 볼 수 있고(`TARGETS=darwin/arm64 scripts/build-release.sh 9.9.9 /tmp/out`),
  CI 서비스를 바꿔도 그대로다.

0.1(Go): `release.yml`은 리눅스 러너 하나에서 테스트 → 네 대상 교차 빌드 → 증명 → 게시를 한 작업으로 했고, 수동 실행 길은 없었다.

## 7. 릴리스 순서

1. `CHANGELOG.md`의 `[Unreleased]` 절을 `## [x.y.z] - 날짜`로 바꾸고 빈 `[Unreleased]`를 새로 둔다. 맨 아래 비교 링크도 고친다.
   이 절이 GitHub 릴리스 노트가 된다(`scripts/changelog-section.sh`). 절이 없으면 릴리스가 실패한다.
2. 매뉴얼 사이드바의 버전 표기(`fung 0.1.0` 꼴, `docs/*.html`)를 새 버전으로 고친다. `tests/changelog_test.go`가 이 버전의
   CHANGELOG 절이 있는지 확인하므로, 둘 중 하나만 고치면 테스트가 실패한다.
3. (처음이거나 워크플로를 바꿨다면) 태그 전에 위 6절의 수동 실행으로 시험한다.
4. `git tag vX.Y.Z && git push origin vX.Y.Z`.
5. Actions의 release 실행이 끝나면 Releases 페이지에 tar.gz 네 개와 `SHA256SUMS`가 올라온다.
