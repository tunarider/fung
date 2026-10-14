# 배포 — 무엇을 만들었고 왜 그렇게 했나

2026-10-14. fung을 저장소 밖의 사람이 "Go 없이" 설치하게 하는 길. 결정: MIT 라이선스, 첫 버전 `v0.1.0`,
대상 Linux·macOS × amd64·arm64, 직접 쓴 빌드 스크립트 + GitHub Actions + 설치 스크립트.

## 1. 무엇을 배포하나: 정적 바이너리 하나

스크립트 언어를 배포할 때 가장 큰 짐은 보통 "런타임 + 표준 라이브러리 파일들"이다(Python의 `lib/`, Node의 `node_modules`).
fung은 처음부터 두 결정 덕에 이 짐이 없다.

- **Go 표준 라이브러리만 쓴다.** `CGO_ENABLED=0`이면 C 라이브러리(libc 포함)에 기대지 않는 **정적 바이너리**가 나온다.
  어느 리눅스 배포판에서든 그 파일 하나로 돈다. glibc 버전이 안 맞아 안 뜨는 문제가 없다.
- **extras를 embed한다**(M5·M7.5). `prelude.fung`·`fs`·`http` 같은 fung 소스가 바이너리 안에 들어 있다.

그래서 릴리스 산출물은 플랫폼마다 `fung` 파일 하나(+ LICENSE, README)를 묶은 tar.gz다.

## 2. 크로스 컴파일

Go는 `GOOS`·`GOARCH` 환경 변수만 바꾸면 한 기계에서 다른 OS·CPU용 바이너리를 만든다. 컴파일러가 모든 대상의 코드
생성기를 갖고 있고, cgo를 끄면 대상 OS의 C 도구가 필요 없기 때문이다. 그래서 `scripts/build-release.sh`는 Mac 한 대에서도,
Actions의 리눅스 러너 하나에서도 네 대상을 다 만든다. OS별 빌드 머신(매트릭스)이 필요 없다.

빌드 옵션:
- `-trimpath`: 바이너리에 내 컴퓨터의 경로(`/Users/...`)가 남지 않게 한다. 재현 가능한 빌드에도 필요하다.
- `-ldflags "-s -w"`: 디버그 심볼을 빼서 크기를 줄인다(패닉 스택 추적은 그대로 나온다).
- `-ldflags "-X main.linkedVersion=0.1.0"`: **링크할 때** 패키지 변수에 문자열을 넣는다. 소스를 고치지 않고 태그를 버전으로 박는 표준 방법이다.

## 3. 버전은 어디서 오나

`cmd/fung/version.go`의 `resolveVersion`이 순서대로 고른다.
1. 릴리스 빌드가 `-X`로 넣은 값.
2. `go install …@v0.1.0`으로 설치했다면 Go가 바이너리에 기록한 모듈 버전(`runtime/debug.ReadBuildInfo`).
3. 둘 다 없으면 `0.0.0-dev`. 저장소 안에서 `go build`하면 Go 1.24+가 커밋 기반 가짜 버전(`0.0.0-날짜-커밋+dirty`)을 기록하므로
   어느 커밋에서 만든 바이너리인지 보인다.

**왜 0.x인가.** 유의적 버전(SemVer)에서 `0.x`는 "아직 공개 API가 안정되지 않음"을 뜻한다. 마이너 버전(`0.1` → `0.2`)이 올라갈 때
호환이 깨질 수 있다. 문법을 아직 바꿀 수 있는 언어에 맞는 약속이다. `1.0`부터는 메이저 버전에서만 깨뜨린다.

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

- `ci.yml`: main 푸시와 PR마다 커밋 전 검사와 같은 것(gofmt, vet, 테스트)을 리눅스·macOS에서, `-race`는 리눅스에서.
  `tests/perf_test.go`의 시간 기준은 `-race`(수 배 느려짐)와 맞지 않아 `-race`는 `internal`·`cmd`만 돈다.
- `release.yml`: `v*` 태그를 푸시할 때만. 태그 모양 확인 → 테스트 → 빌드 → 증명 → 릴리스 게시.
  깨진 코드에 태그를 붙여도 테스트에서 멈춘다.
- 빌드 로직은 워크플로 YAML이 아니라 `scripts/build-release.sh`에 둔다. 로컬에서 똑같이 돌려 볼 수 있고, CI 서비스를 바꿔도 그대로다.

## 7. 릴리스 순서

1. `CHANGELOG.md`의 `[Unreleased]` 절을 `## [x.y.z] - 날짜`로 바꾸고 빈 `[Unreleased]`를 새로 둔다. 맨 아래 비교 링크도 고친다.
   이 절이 GitHub 릴리스 노트가 된다(`scripts/changelog-section.sh`). 절이 없으면 릴리스가 실패한다.
2. 매뉴얼 사이드바의 버전 표기(`fung 0.1.0`, `docs/*.html`)를 새 버전으로 고친다. `tests/changelog_test.go`가 이 버전의
   CHANGELOG 절이 있는지 확인하므로, 둘 중 하나만 고치면 테스트가 실패한다.
3. `git tag v0.1.0 && git push origin v0.1.0`.
4. Actions의 release 실행이 끝나면 Releases 페이지에 tar.gz 네 개와 `SHA256SUMS`가 올라온다.
