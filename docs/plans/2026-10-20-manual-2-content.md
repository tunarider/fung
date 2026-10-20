# 매뉴얼 2~5단계 (내용) 작성 지침

설계: `docs/superpowers/specs/2026-10-20-manual-structure-design.md`(특히 1·3절). 뼈대: `docs/plans/2026-10-20-manual-1-skeleton.md`의 "원고 형식".
이 문서는 여러 작성자(에이전트)가 각자 맡은 원고를 같은 규칙으로 쓰기 위한 지침이다. 페이지 목록과 주소는 `manual/toc.txt`에 이미 고정되어 있다.

## 1. 독자와 목소리

- 기준 독자: fung을 전혀 모르는 사람. 지향점: fung이 **첫 프로그래밍 언어**인 사람도 따라온다. 그래서 일반 개념(값, 변수, 함수, 리스트, 에러, 모듈, 동시성 …)을 fung 문법보다 먼저 쉬운 말로 설명한다.
- 이 매뉴얼은 언어 입문서 한 권을 대신한다. 고급 최적화 팁은 목표가 아니다.
- 문체: 한국어 해라체 평서문("…다", "…한다"). 지금 매뉴얼(`manual/reference/language.md`)과 같다. 짧은 문장, 능동태, 전문 용어는 처음 나올 때 풀어 쓴다.
- 영어 용어는 처음 한 번 괄호로(예외(exception), 클로저(closure)). 코드·함수 이름·에러 번호는 `` `code` ``.
- **사실만 쓴다.** 동작은 반드시 직접 돌려 보거나(`target/debug/fung`) 설계 문서(`docs/design/*`)·노트(`docs/notes/*`)·코드에서 확인한다. 추측한 동작을 쓰지 않는다. 문서와 실제 동작이 다르면 실제 동작을 쓰고 보고에 적는다.
- **다른 언어 이름 규칙**: 저장소 검사(`repo_rust_only.rs`)가 구현 이전 언어의 이름을 현재 문서에서 금한다. 비교에 Python, Rust, JavaScript, Lisp, Erlang, Elixir, Lua, C, Java, shell 등은 써도 되지만 **"G" + "o"로 된 그 언어 이름은 절대 쓰지 않는다**(문장, 코드, 링크 모두). `crates/fung-cli/tests/repo_rust_only.rs`의 낡은 용어 목록(예: "token held", "Census")도 피한다. 작성 뒤 `cargo test -p fung-cli --test repo_rust_only`로 확인한다.

## 2. 원고 문법 (요약)

- 머리: `---` / `description: 한 문장` / `lede: 한두 문장(선택)` / `---`, 다음 줄 `# 제목`. **description은 모든 페이지에 필수**(시험이 지킨다).
- `## 제목 {#id}`, `### 제목 {#id}`(id 필수, 영문 소문자·숫자·`-`), `####`은 id 선택. 사이드바의 절 목록은 `##`만 쓴다.
- 문단, `- ` 목록, `1. ` 목록, 인라인 `` `code` ``, `**굵게**`, `[글](주소)`(글에 `` `code` `` 가능). 표는 날 HTML `<table>`로.
- 날 HTML은 `<`로 시작하는 줄부터 빈 줄까지 그대로 통과. 그 안에 `<pre>`·`<h1>`·`<h2>` 금지. **펜스·`:::`·제목 앞뒤에는 빈 줄**.
- 펜스(첫 줄 정보):
  - ```` ```fung run ```` 실행하고 실제 출력이 페이지에 붙는다. 속성: `args="a b"`, `stdin=파일`, `env="K=V"`, `exit=N`(0이 아닌 종료를 보여 줄 때 필수; 에러 메시지가 나오면 그 번호를 `error=E0205`처럼 함께 적는다 — 다른 에러가 나오면 생성이 실패한다; `exit=`이 있으면 `fung check`가 실패하는 컴파일 에러 예제도 된다 — 실행이 그 에러를 실제로 보여 준다), `hide-output`(실행마다 출력이 달라지는 것: 시각, 난수 등).
  - ```` ```fung check ```` 검사·포맷만(네트워크, `ssh`, 서버, 끝나지 않는 것, 호스트 파일에 기대는 것). 바로 뒤 ```` ```expect ````로 "예상 출력"을 보일 수 있다(검사하지 않는다는 문구가 자동으로 붙는다).
  - ```` ```fung frag ```` 검사하지 않는 조각(문법 설명용, 완전한 프로그램이 아닌 것).
  - ```` ```dump ast|core|bytecode ```` 내용은 fung 소스, `fung dump` 실제 출력이 붙는다(VM 책).
  - ```` ```shell ```` bash 비교, ```` ```plain ```` 그림·출력 예, ```` ```core ```` 손으로 쓴 코어 트리.
  - ```` ```file 이름 ```` 바로 다음 `run`/`check` 예제의 준비물 파일(여러 개 가능). 예제는 그 파일들만 있는 빈 디렉터리에서 돈다. 환경은 `PATH`·`TZ=UTC`·`HOME`·`TMPDIR`(그 디렉터리)뿐.
- `run`·`check` 예제는 `fung fmt` 스타일이어야 한다(아니면 생성이 실패한다). 쓰고 나서 `target/debug/fung fmt -`로 맞춘다.
- 컨테이너(`:::종류 인자` … `:::`, 중첩 가능, 앞뒤 빈 줄):
  - `:::fn 서명 [fung]` 함수 항목. 첫 문단 = 한 줄 요약(시그니처 위에 보인다). 서명은 **런타임의 실제 매개변수 표와 정확히 같아야 한다**(`manual.rs`가 지킨다; 지금 원고의 서명을 바꾸지 말 것). `fung` 표시는 fung으로 쓴 함수(지금 원고 그대로 유지).
  - `:::error E0606` 에러 항목(제목·카탈로그 메시지·도움말은 자동). 본문에 뜻, 흔한 원인, 예시(`exit=` 붙은 run), 고치는 법.
  - `:::box learn|mistake|tip|other|choice 제목` 상자: learn = 처음 배우는 분께, mistake = 자주 하는 실수, tip = 권장, other = 다른 언어는, choice = fung의 선택(이유).
  - `:::goals` 이 장에서 배울 것(목록 하나).
  - `:::exercise 제목` 연습 문제, 안에 `:::solution`(접힌 풀이, 실행되는 예제로).
- 링크는 상대 경로. 생성기가 깨진 링크·없는 앵커를 실패로 잡는다.

## 3. 페이지와 주소 (고정)

`manual/toc.txt`가 기준이다. 자주 링크할 곳:
- 레퍼런스: `reference/language.html`(언어 명세, 절 id: lexical, blocks, values, operators, access, scope, functions, conditions, match, for, results, modules, types, testing, grammar 등 — 파일을 열어 확인),
  `reference/functions.html`(모든 함수 목록), `reference/prelude/{io,convert,collections,maps,strings,results,loops,program,process,concurrency}.html`,
  `reference/modules/{fs,path,cli,json,time,regex,http,conc}.html`, `reference/errors.html`(`#E0606` 꼴), `reference/cli.html`.
  함수 앵커는 `#fn-이름`(모듈은 `#fn-fs-read`처럼 점을 `-`로).
- 배우기: `learn/01-programming` … `learn/14-testing`(toc.txt 참고). 예제 모음: `recipes/*.html`. VM: `vm/01-how-programs-run` … `vm/17-test-runner`, 부록 `vm/internals.html`.

## 4. 각 부분의 모양

### 배우기 장 (`learn/*.md`)
- 순서: `:::goals` → 본문(일반 개념 → fung 문법 → 실행 예제와 출력) → `:::box learn …`(일반 개념, 이미 아는 사람은 건너뜀) → `:::box mistake …`(에러 번호와 이유) → `:::box other …`(필요할 때만) → `:::exercise …`(1~2개, `:::solution` 안에 실행되는 풀이) → `## 정리 {#summary}` → 마지막 문단에 레퍼런스·VM 링크.
- 앞 장만 알면 읽힌다. 뒤 장 개념은 쓰지 않거나 "n장에서 다룬다"로 미룬다.
- 분량: 장당 대략 2,500~5,000자 본문, 예제 6~15개. 장마다 실무 맛이 나는 예(로그, 파일, 서버 목록 등)를 섞는다(독자는 SRE/DevOps 일을 한다).
- 1장은 "프로그래밍이 무엇인가, 왜 fung인가, 이 책을 읽는 법"(코드는 짧게), 2장은 설치·`fung` 실행·REPL·첫 스크립트·`fung check`·에러 메시지 읽는 법.

### 예제 모음 (`recipes/*.md`)
- 순서: 한 문단 문제 → `## 쉘로는 {#shell}`(```` ```shell ```` 한두 줄과 그 한계) → `## fung으로 {#code}`(전체 스크립트; 준비물 `file`로 돌 수 있으면 `run`, 아니면 `check`+`expect`) → `## 줄별 설명 {#how}` → `## 응용 {#vary}` → 배우기·레퍼런스 링크.
- `examples/`의 스크립트(`rename`·`bigfiles`·`logsum`·`checkurls`·`serve`·`api`·`dotenv`·`envcheck`·`kebab`·`loglines`)를 다듬어 쓸 수 있다(같은 코드를 두 곳에서 따로 관리하지 않도록, 원고에는 레시피에 맞게 고친 판을 둔다; `examples/` 파일은 고치지 않는다).
- 서버 접속은 외부 명령 `ssh`를 `exec`로 부르는 방식(`check`), 여러 서버는 `conc.pmap`과 `timeout`. 쉘 습관이 부딪히는 것(블록 안에서 바깥 변수 바꾸기 E0205 등)은 `:::box mistake`로.

### 레퍼런스 함수 항목 (`reference/prelude/*`, `reference/modules/*`)
- PHP 매뉴얼식: `:::fn 서명` 안에 한 줄 요약 → `#### 설명`(무엇을 하고, 제약, 경계 사례) → `#### 매개변수`(목록: `` - `이름`: … ``) → `#### 돌려주는 값` → `#### 에러`(Err의 kind, 런타임 에러 번호) → (필요하면 `:::box tip …` 권장) → `#### 예제`(2~4개, 각 예제 앞에 굵은 한 줄 제목) → `#### 같이 보기`(링크).
- 아주 단순한 함수(예: `upper`)는 설명·예제를 짧게 해도 된다. 각 페이지 머리(`lede`)와 첫 문단에 그 주제의 개념 설명(예: 맵이란, 불변이란).

### 에러 항목 (`reference/errors.md`)
- 95개 모두. 각 `:::error`: 뜻 한두 문장 → 흔한 원인 → 에러를 내는 짧은 예(`fung run exit=1` 또는 컴파일 에러면 `exit=2`; 실제 출력이 붙는다) → 고치는 법(고친 예). 절 머리(`## E01xx …`)에 그 분류의 설명.
- 에러를 실제로 재현할 수 없는 것(내부 오류, 환경 의존)은 예제 없이 설명만, 이유를 적는다.

### VM 책 (`vm/*.md`)
- 독자는 언어 구현을 처음 공부하는 사람(사용자 본인). 아무것도 모른다고 가정하고 개념부터.
- 장의 흐름: `:::goals` → 일반 개념 → `:::box other 다른 언어는 …`(CPython, JVM, Lua, JavaScript 엔진 등 비교) → `:::box choice …`(fung이 고른 것과 이유, **설계 문서·노트에서 가져온다**, 지어내지 않는다) → 실제 출력으로 따라가기(```` ```dump ```` 펜스, 같은 예 `fn double(x) = x * 2` / `print(double(21))`를 책 전체에서 따라가고, 장에 맞는 예를 더한다) → `## 코드에서 찾아보기 {#source}`(실제 파일 경로, 주요 타입·함수 이름; 존재를 확인) → `:::exercise` → `## 정리 {#summary}`.
- 장 맨 위(목표 앞)에 단계 띠: `<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li>코어</li><li class="on">바이트코드</li><li>실행</li></ol>` (해당 단계에 `class="on"`, 단계와 무관한 장은 생략).
- fung이 쓰지 않는 방식(가비지 컬렉터, 레지스터 머신, JIT, 1:1 스레드 등)도 비교로 짧게.
- 한 장 20~30분 분량. 4부(런타임)는 두툼하게.
- 원재료: `manual/vm/internals.md`(부록으로 남는다, 고치지 않는다), `docs/design/*`, `docs/notes/*`(특히 `R1-front-end`, `R2-vm`, `R7-scheduler`, `R7-language`, `M9-formatter`, `M10-persistent-collections`), 소스 `crates/`.

## 5. 작업 방법 (작성자 공통)

- 맡은 파일만 고친다. `manual/toc.txt`, 다른 사람의 원고, `crates/`, `docs/`(생성물) 는 고치지 않는다.
- 검증: `cargo build -p fung-cli` 뒤
  1. `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build the_manual_is_up_to_date` (원고 문법·예제 실행·링크 검사; 실패 메시지가 `파일:줄`을 알려 준다)
  2. `cargo test -p fung-cli --test manual --test repo_rust_only`
  3. 몇 페이지는 생성된 `docs/latest/...html`을 열어 눈으로 확인해도 좋다.
- 커밋: 맡은 `manual/...` 파일만 `git add`해서 커밋한다(영어 Conventional Commits, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`). **`docs/`와 `crates/`의 변경(생성물, bless)은 커밋하지 않는다** — 합칠 때 한 번에 다시 만든다. 커밋 전에 `git checkout -- docs crates`로 되돌린다.
- 보고: 쓴 페이지 목록, 예제 수, 실제 동작이 문서와 달랐던 점, 확신하지 못한 부분.
