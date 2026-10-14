# fung 로고

| 파일 | 용도 |
|---|---|
| `logo-light.svg` | 가로형 로고, 밝은 배경용 (배경 투명) |
| `logo-dark.svg` | 가로형 로고, 어두운 배경용 (배경 투명) |
| `icon-light.svg`, `icon-light-512.png` | 정사각형 아이콘, 밝은 판 (매뉴얼 파비콘) |
| `icon-dark.svg`, `icon-dark-512.png` | 정사각형 아이콘, 어두운 판 |
| `favicon-32.png`, `apple-touch-icon.png` | SVG 파비콘을 못 쓰는 브라우저, iOS 홈 화면 (180px) |
| `social-preview-light.png`, `social-preview-dark.png` | 링크 공유 미리보기 1280×640 (GitHub Settings › Social preview, 매뉴얼의 `og:image`) |

## 규격

- 글꼴: IBM Plex Mono Medium 500 (SIL Open Font License 1.1). 글자는 도형으로 바꿔 넣어 글꼴이 없어도 똑같이 보인다.
- 자간: 글자 사이 -0.04em, `;;` 사이 -0.32em (고정폭에서 `; ;`처럼 벌어지는 것을 좁힘). 아이콘의 `f;;`는 -0.06em.
- 색: 글자 `#1d2a2a`(밝은 배경) / `#dde6e2`(어두운 배경), `;;`는 양쪽 모두 `#198f7b`.
  배경 `#f3f5f2` / `#172221`, 밝은 아이콘 테두리 `#cbd4cf`. 강조색 대비: 밝은 배경 3.64, 어두운 배경 4.08.
- 아이콘: 둥근 사각형(반지름 22%), `f;;`를 글꼴 크기 62%로 잉크 영역 기준 정중앙에 둔다.

## 다시 만들기

`scripts/logo/`의 스크립트로 만든다. Python 표준 라이브러리만 쓴다.

```
python3 scripts/logo/gen.py IBMPlexMono-Medium.ttf out      # SVG 4개
cp scripts/logo/render.html out/
python3 scripts/logo/serve.py out                           # http://127.0.0.1:8765/render.html 을 열면 out/png/에 PNG
```

글꼴 파일은 <https://github.com/IBM/plex> 의 `packages/plex-mono/fonts/complete/ttf/IBMPlexMono-Medium.ttf`.
