# fung

개인 학습 + 일상의 소소한 자동화를 목표로 하는 실용적 함수형 스크립트 언어.

이름은 한국어 "펑"(빠르게 만들고 치고 빠진다)과 **fun**ction + **g**o에서 왔다.

- 파일 확장자: `.fung`, 실행 명령: `fung`
- 구현: Go, 바이트코드 VM (Go 런타임의 GC·고루틴 활용)
- 용도: 간단한 스크립트(파일 일괄 처리 등), 간단한 웹서버, 추후 GUI 앱

설계 문서는 [docs/design](docs/design/README.md)에 있다. 마일스톤별 설명은 [docs/notes](docs/notes/)에.
매뉴얼(HTML)은 [docs/index.html](docs/index.html).

## 설치

Linux·macOS (amd64, arm64). 릴리스의 `SHA256SUMS`로 검증한 뒤 `~/.local/bin`에 설치한다.

```
curl -fsSL https://raw.githubusercontent.com/tunarider/fung/main/install.sh | sh
```

`FUNG_VERSION=0.1.0`, `FUNG_INSTALL_DIR=/usr/local/bin`으로 버전과 위치를 고른다. Go가 있으면
`go install github.com/tunarider/fung/cmd/fung@latest`도 된다.

## 사용

```
go build ./cmd/fung
./fung examples/rename.fung <dir> --dry-run
./fung -e 'print([3, 1, 2] |> sort)'
./fung test examples/          # test 블록 실행 (샘플: rename, logsum, tidy, envcheck)
./fung check script.fung       # 실행 없이 검사 + 경고
./fung                         # REPL
./fung -e 'print(pmap([1, 2, 3], x => x * x))'   # 태스크·채널·atom (docs/design/05)
```

## 릴리스

`v<major>.<minor>.<patch>` 태그를 푸시하면 `.github/workflows/release.yml`이 테스트, 빌드
(`scripts/build-release.sh`), 빌드 증명, GitHub 릴리스 게시를 한다. 릴리스 노트는 [CHANGELOG.md](CHANGELOG.md)의 해당 버전 절이다. 절차는 [docs/notes/release.md](docs/notes/release.md) 7절.

```
git tag v0.1.0 && git push origin v0.1.0
```

## 라이선스

MIT. [LICENSE](LICENSE) 참고.
