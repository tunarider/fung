# L17. 표시 너비가 글자 단위라 ZWJ 이모지·VS16에서 터미널과 다르다

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

fung은 화면의 칸 수를 코드 포인트 하나씩 UAX #11로 센다(넓은 글자 2칸, 결합·서식 문자 0칸, 01 "글자 수준 규칙", 08 포매터 "정렬").
요즘 터미널은 이모지를 **글자 묶음**(grapheme cluster, UAX #29) 단위로 그린다. ZWJ(U+200D)로 이은 가족 이모지는 한 묶음 2칸이고, VS16(U+FE0F)이 붙은 `❤️`는 이모지 모양이라 2칸이다.
fung의 셈은 가족 이모지를 6칸(👨 2 + ZWJ 0 + 👩 2 + ZWJ 0 + 👧 2), `❤️`를 1칸(U+2764는 1칸, U+FE0F는 Mn이라 0칸)으로 본다. 릴리스 바이너리(2026-10-26), `fung fmt -`의 꼬리 주석 정렬:

```
$ printf 'a = "한한"  # k\nb = "👨‍👩‍👧"  # zwj\nc = "❤️"  # vs16\nd = "abcd"  # ascii\n' | fung fmt -
a = "한한"    # k
b = "👨‍👩‍👧"  # zwj
c = "❤️"       # vs16
d = "abcd"    # ascii
```

fung이 보기에는 네 줄 모두 `#`가 14칸째다. 묶음 단위로 그리는 터미널에서는 `b` 줄의 `#`가 4칸 왼쪽, `c` 줄이 1칸 오른쪽에 보인다.

진단의 밑줄은 더 단순하다. `write_excerpt`가 **글자 수**로 칸을 센다(UAX #11도 쓰지 않음). 그래서 한글 같은 넓은 글자 뒤의 밑줄도 어긋난다.

```
$ fung -e 'x = "한글" + nope'        # 밑줄 앞 공백 11개, ^ 4개
error[E0204]: undefined name `nope`
  --> -e:1:12
  |
1 | x = "한글" + nope
  |            ^^^^ not found in this scope
```

터미널에서 `nope`는 13칸째(0부터 세면)에서 시작하는데 밑줄은 11칸째에서 시작한다. 가족 이모지 줄은 공백 14개다(터미널에서 `nope`는 10칸째). `❤️` 줄은 공백 11개로 우연히 맞는다(글자 둘이 2칸).
`crates/fung-front/src/diag.rs`의 시험(`print(없는)` 아래 `^^`)이 지금 모양을 고정하고 있다. 넓은 두 글자(4칸) 아래 밑줄이 2칸이다.
REPL 줄 편집기는 커서를 칸 수로 옮기지만(08 "REPL") 버퍼가 코드 포인트 단위(`Vec<char>`)다. 그래서 화살표 한 번이 묶음의 가운데로 들어가고, 가족 이모지 하나를 지우는 데 Backspace가 다섯 번 든다.
원칙 7(규칙 하나에서 결과가 따라 나온다)에 어긋난다. 같은 "칸"이 포매터·REPL에서는 UAX #11, 진단에서는 글자 수, 터미널에서는 묶음이다.

## 지금 동작과 원인

- 너비 표: `fung_front::unicode::width`(`crates/fung-front/src/unicode/mod.rs` 87행). `spec/unicode.txt`의 `wide`(East Asian Width W·F)·`zero`(Mn·Me·Cf, U+00AD 빼고, 한글 중성·종성 자모, 피부색 수식) 범위를 찾는다.
  표는 UCD에서 만든다(`unicode/generate.rs`, `FUNG_UPDATE_SPEC=1 FUNG_UCD=<dir> cargo test -p fung-front unicode_spec_file`, 01). 지금 UCD 17.0.0, 묶음 경계 데이터는 없다.
- 포매터: `crates/fung-front/src/format.rs`의 `units`·`display_width`(161~176행)가 코드 포인트마다 `width`를 더한다. 잘못된 UTF-8은 부분마다 1칸.
- REPL: `crates/fung-cli/src/repl/editor.rs`의 `buf: Vec<char>`와 `cur`(코드 포인트 색인), `repl/line.rs`의 `redraw`(118행)가 커서 뒤 글자들의 `width` 합만큼 왼쪽으로 옮긴다.
- 진단: `crates/fung-front/src/diag.rs`의 `write_excerpt`(217행). `Pos::col`(코드 포인트, `source.rs`)까지 글자를 세고 탭만 `TAB_WIDTH`로 펼친다. 밑줄 길이 `d.len`도 글자 수다.
- `len`·`s[i]`·`chars`는 코드 포인트다(`len("👨‍👩‍👧")`이 5). 이것은 표시와 다른 문제이고 이 문서의 범위 밖이다(정할 것 4).

## 고려할 점

- **정답이 하나가 아니다.** 터미널마다 다르다. 묶음 단위로 그리는 것(iTerm2·WezTerm·kitty·Ghostty 등의 설정)이 늘고 있지만, 코드 포인트 단위(`wcwidth`)로 그리는 것도 남아 있다. 어느 쪽을 택해도 어떤 터미널과는 다르다.
  다만 유니코드의 권고(UTS #51 이모지, UAX #29 묶음)와 최근 터미널의 흐름은 "묶음 하나 = 그 묶음의 너비"다.
- **묶음 경계는 데이터가 크다.** UAX #29의 확장 묶음 규칙(GB9c 인도계 결합, GB11 이모지 ZWJ, GB12·13 국기 쌍)과 `Grapheme_Cluster_Break`·`Extended_Pictographic`·`Emoji_Presentation`·`Indic_Conjunct_Break` 데이터가 필요하다.
  앞단(`fung-front`)은 크레이트 0개라(CLAUDE.md 의존성 정책) 규칙과 표를 직접 짜고, 표는 지금처럼 UCD에서 만들어 `spec/unicode.txt`에 넣는다.
- **묶음의 너비 규칙.** 흔한 방식: 묶음에 VS16이 있거나 첫 글자가 `Emoji_Presentation`이면 2칸, 국기 쌍은 2칸, 그 밖은 첫 글자의 UAX #11 너비(결합 문자는 0). VS15(U+FE0E)는 텍스트 모양으로 1칸.
- **진단의 위치.** `Pos::col`을 바꾸면 에러 출력의 `-->` 줄(`-e:1:12`)도 바뀐다. 편집기·LSP가 쓰는 칸 번호(코드 포인트·UTF-16·바이트)는 표시 칸과 다른 문제라 섞지 않는 편이 낫다.
- 다른 언어: Rust 컴파일러는 밑줄에 `unicode-width`(코드 포인트 UAX #11)를 쓴다. fish·zsh 같은 셸의 줄 편집기는 묶음을 한 단위로 지우고 옮긴다. Swift는 `count`가 묶음 수다.

## 선택지

(가) **그대로 두고 적는다.** 장점: 할 일 없음. 단점: 진단의 한글 밑줄 어긋남(이모지보다 흔함)도 그대로.
(나) **진단만 UAX #11로 맞춘다.** `write_excerpt`가 `width`로 칸을 세고 밑줄 길이도 칸으로. 장점: 작고, 한글·한자 문제를 고친다. 단점: ZWJ·VS16은 여전히 다르다.
(다) **묶음 단위 너비를 하나 두고 셋이 모두 쓴다.** `fung_front::unicode`에 UAX #29 묶음 나누기와 묶음 너비 함수를 두고, 포매터·진단·REPL(버퍼를 묶음 단위로 옮기고 지움)이 같은 함수를 쓴다.
  장점: "칸"의 규칙이 하나, 유니코드 권고와 최근 터미널에 맞음. 단점: 묶음 규칙·표를 직접 짜야 함(앞단 크레이트 0개), `wcwidth` 터미널과는 이모지에서 다름.

추천: **(다)**. 칸 수를 쓰는 세 곳이 서로 다른 규칙을 쓰는 지금이 원칙 7에 가장 어긋난다. (나)는 (다)의 일부이므로 따로 할 이유가 없다.
코드 포인트 단위 터미널과 다른 점은 피할 수 없으므로 01에 "fung의 칸은 UAX #29 묶음 단위"라고 적고 그 터미널에서의 차이를 한계로 남긴다.

## 정할 것

1. 묶음의 너비 규칙. 추천: VS16이 있거나 첫 글자가 `Emoji_Presentation`이면 2칸, 지역 표시 글자 쌍(국기) 2칸, VS15가 있으면 1칸, 그 밖은 첫 글자의 UAX #11 너비. 잘못된 UTF-8 부분은 지금처럼 1칸.
2. 진단의 `-->` 줄의 칸 번호. 추천: 지금처럼 코드 포인트로 두고(표시와 분리), 밑줄을 그릴 때만 칸으로 바꾼다. 밑줄 길이는 묶음 너비의 합.
3. REPL에서 화살표·Backspace·Delete가 묶음 단위로 움직이는가. 추천: 그렇다(셸 줄 편집기와 같음). 히스토리에 저장하는 글은 그대로.
4. `len`·`s[i]`·`chars`를 묶음 단위로 바꾸는가. 추천: 이번에는 바꾸지 않는다. 표시와 별개의 언어 결정이고 L4(문자열과 바이트)와 함께 봐야 한다. 필요하면 `graphemes(s)`를 따로 묻는다.
5. 표의 범위. 추천: `spec/unicode.txt`에 `Grapheme_Cluster_Break` 갈래들, `Extended_Pictographic`, `Emoji_Presentation`, `Indic_Conjunct_Break`를 더하고 같은 UCD 판(17.0.0)에서 만든다. 필요한 파일(`GraphemeBreakProperty.txt`·`emoji-data.txt`·`DerivedCoreProperties.txt`)을 01의 명령 설명에 적는다.

## 할 일

1. 시험 먼저: `fung-front`에 UAX #29의 `GraphemeBreakTest.txt` 사례를 도는 시험(파일은 UCD에서 오므로 `FUNG_UCD`가 있을 때만 전체, 평소에는 대표 사례를 시험 코드에 적음).
   묶음 너비 단위 시험(가족 이모지 2, `❤️` 2, `❤︎` 1, 국기 2, `é` 분해형 1, 한글 2, 한글 자모 조합 2). 포매터 정렬 시험(위 재현), 진단 밑줄 시험(`diag.rs`의 `print(없는)` 기대를 `^^^^`로), REPL 편집기 시험(`editor.rs`·`line.rs`).
2. `unicode/generate.rs`가 새 표를 만들게 하고 `spec/unicode.txt`를 다시 만든다. `unicode/mod.rs`에 묶음 나누기와 묶음 너비를 둔다.
3. `format.rs`의 `units`, `diag.rs`의 `write_excerpt`, `repl/editor.rs`의 버퍼와 커서, `repl/line.rs`의 `redraw`가 그 함수를 쓰게 바꾼다.
4. 설계 문서: 01("글자 수준 규칙"의 표시 너비), 08(포매터 "정렬", REPL, 진단 모양). open-questions "알려진 한계"의 "도구" 줄을 지우고 닫은 기록으로 옮긴다.
5. 매뉴얼: 레퍼런스의 포매터·REPL·진단 설명, VM 부록(`manual/vm/07-diagnostics.md`·`15-formatter.md`·`16-repl.md`)을 고치고 다시 만들어 diff를 본다.
   기대 출력(`crates/fung-cli/tests/expected/`)에 넓은 글자가 든 진단이 있으면 `FUNG_EXPECTED=bless`로 다시 쓰고 diff를 검토한다. CHANGELOG `[Unreleased]`에 적는다.

## 끝의 기준

- 위 `fmt` 재현에서 묶음 단위 터미널로 보면 네 줄의 `#`가 같은 칸이다. 진단의 밑줄이 한글·가족 이모지·`❤️` 뒤에서도 `nope` 바로 아래 4칸이다.
- REPL에서 가족 이모지 하나가 Backspace 한 번에 지워지고, 화살표가 묶음 가운데로 들어가지 않는다.
- 포매터·진단·REPL이 같은 함수 하나로 칸을 센다(코드에서 `unicode::width`를 직접 부르는 곳이 그 함수 안뿐).
- `GraphemeBreakTest.txt` 사례가 통과하고, open-questions에 이 한계가 없으며, 커밋 전 검사가 통과한다.
