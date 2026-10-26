# 매뉴얼 1단계 (뼈대) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 원고(`manual/`)에서 `docs/latest/`를 만드는 생성기, 버전 체계, 새 페이지 틀(사이드바·서랍·브레드크럼·이전/다음)을 만들고, 지금 매뉴얼 네 장을 내용 그대로 옮긴다.

**Architecture:**
- 생성기는 `fung-cli`의 시험 코드다(`crates/fung-cli/tests/manual_site/`). 새 크레이트도 의존성도 없다.
  시험 `manual_build`가 원고로 사이트를 메모리에 만들고 커밋된 `docs/`와 비교한다. `FUNG_UPDATE_MANUAL=1`이면 대신 쓴다(`spec/*.txt`의 `FUNG_UPDATE_SPEC`와 같은 방식).
- 원고는 마크다운의 작은 부분집합에 날 HTML을 허용한 형식이다. 실행되는 코드는 반드시 펜스로 쓰고, 생성기가 예제를 실제로 돌려 출력 칸을 채운다.
- 목차는 `manual/toc.txt` 하나다. 사이드바·브레드크럼·이전/다음·표지 카드가 모두 여기서 나온다.

**Tech Stack:** Rust 표준 라이브러리(`std::io::pipe`, MSRV 1.94). 페이지는 정적 HTML·CSS·JS(외부 라이브러리 없음).

**Spec:** `docs/superpowers/specs/2026-10-20-manual-structure-design.md`(특히 4·5·6절). 승인된 시안: `.superpowers/proto/`(브라우저로 보는 결과)와 그 원본 `.superpowers/proto-src/build.py`·`manual.css`(커밋하지 않는 폴더, 이 작업 트리에 있음). 페이지 틀의 마크업과 CSS·JS는 이 시안을 기준으로 옮긴다.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`): 작업마다 TDD, 그 작업의 시험만 돌리고 작업 단위로 커밋한다. 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast` debug·release, `cargo deny`·`audit`)와 브랜치 전체 검토는 끝에 한 번.
- 사용자와의 대화는 한국어. 코드·주석·커밋은 영어, 매뉴얼 원고는 한국어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 크레이트를 더하지 않는다(`deps.rs`·`deny.toml`).
- 릴리스 전 내용의 표기는 영어 **unreleased**다. "미릴리스"라는 말은 페이지 어디에도 쓰지 않는다(사용자 결정).
- 1단계는 **옮기기만** 한다. 문장을 고쳐 쓰지 않는다. 링크 경로 수정, `<pre>` → 펜스 변환, 예제 출력 칸이 새로 생기는 것만 바뀐다.
- 생성된 페이지의 내부 링크·자산 경로는 모두 상대 경로다(GitHub Pages의 `/fung/` 아래와 `file://` 둘 다에서 열려야 한다).
- 모든 확인용 실행에 바깥 `timeout`을 건다(시험 하나 `timeout 300`, 전체 `timeout 1800`).

## Review Focus

1. **실행할 때마다 출력이 달라지는 예제**(시각, 환경 변수, 임시 경로, OS마다 다른 에러 문장): 생성기가 두 번 돌려 다르면 라벨과 함께 실패하고 `hide-output`을 권한다. 흔들리는 HTML이 커밋되면 안 된다. → 작업 2 `nondeterministic_output_is_refused`.
2. **생성기가 모르는 원고 문법**(모르는 펜스 종류, 날 HTML 안의 `<pre`·`<h2`, `{#id}` 없는 `##`, 같은 id 두 번): 조용히 버리지 않고 `파일:줄: 이유`로 실패한다. → 작업 1 `unknown_markup_is_an_error`.
3. **깨진 내부 링크**(옮기면서 `internals.html#concurrency` 같은 옛 페이지 링크가 남는 것): 생성기가 모든 상대 `href`의 파일과 `#id`를 확인한다. → 작업 3 `broken_links_are_errors`.
4. **옛 주소의 앵커**(`docs/builtins.html#fn-map`을 북마크한 사람): 같은 앵커로 `latest`에 도착한다. → 작업 4 `old_addresses_forward_with_the_anchor`.
5. **원고에서 빠진 페이지의 HTML이 `docs/latest/`에 남는 것**: 비교가 남는 파일도 차이로 잡고, 갱신 모드는 지운다. → 작업 4 `stray_files_are_reported`.

---

## 파일 구조

```
manual/                         원고 (latest의 원본)
  toc.txt                       목차 하나
  index.md                      표지
  reference/language.md         ← docs/syntax.html
  reference/functions.md        ← docs/builtins.html
  vm/internals.md               ← docs/internals.html
  assets/style.css              ← docs/assets/style.css + 시안 manual.css
  assets/manual.js              ← docs/assets/site.js + 시안의 nav JS
  assets/wordmark.svg           사이드바·표지에 인라인으로 넣는 워드마크

crates/fung-cli/tests/
  manual_build.rs               시험: 비교/갱신/얼리기
  manual_site/mod.rs            pub mod markup, examples, toc, render, site
  manual_site/markup.rs         원고 → 블록 (의존 없음, tools_expected도 #[path]로 씀)
  manual_site/examples.rs       예제 실행
  manual_site/toc.rs            toc.txt
  manual_site/render.rs         페이지 HTML
  manual_site/site.rs           사이트 조립, 링크 검사, 버전, 옛 주소
  data/manual/                  생성기 시험용 작은 원고 (toc.txt, a.md, b.md …)

docs/
  versions.js                   버전 목록 (공유)
  index.html syntax.html builtins.html internals.html   옛 주소 → latest (생성됨)
  latest/**                     생성됨
  assets/logo/                  그대로 (공유)
```

## 원고 형식 (작업 1이 구현하고 이후 작업이 따르는 약속)

- 맨 앞 선택적 머리: `---` 줄 사이의 `key: value`(`lede`, `description`). 첫 블록은 `# 제목`(필수, 페이지 제목).
- `## 글 {#id}`, `### 글 {#id}`: id 필수. `####`은 id 선택.
- 문단, `- ` 목록, `1. ` 목록. 인라인: `` `code` ``(내용은 이스케이프), `**굵게**`, `[글](주소)`, 그 밖의 인라인 HTML은 그대로 통과.
- 날 HTML 블록: `<`로 시작하는 줄부터 빈 줄까지 그대로 통과. 그 안에 `<pre`·`<h1`·`<h2`가 있으면 에러. 날 HTML 사이에 빈 줄을 두면 그 사이의 펜스·문단은 원고로 처리된다(예: `<div class="fn" …>` / 빈 줄 / 펜스 / 빈 줄 / `</div>`).
- `{{parts}}` 한 줄: 표지의 부분 카드(toc에서).
- 펜스 ```` ``` ```` 정보 문자열:

| 정보 | 뜻 |
|---|---|
| `fung run` | check·fmt --check·실행, 출력 칸 채움. 속성: `args="a b"`, `stdin=파일`, `env="K=V K2=V2"`, `exit=N`(기본 0, 다르면 에러), `hide-output` |
| `fung check` | check·fmt --check만. 바로 뒤의 `expect` 펜스가 있으면 "검사하지 않은 예상 출력"으로 보임 |
| `fung frag` | 검사 없음 |
| `dump ast\|core\|bytecode` | 내용은 fung 소스, `fung dump <단계>` 결과를 출력 칸에 |
| `shell`, `plain`, `core` | 검사 없음(`shell`은 bash 표시, `core`는 코어 트리 색) |
| `file 이름` | 다음 `run`/`check` 예제의 준비물 파일(여러 개 가능). 페이지에는 그 예제 위의 접힌 "준비물" 상자로 |
| `expect` | 바로 앞 `check`의 예상 출력(다른 명령의 출력, 검사하지 않음) |
| `expect check` | 바로 앞 `check` 예제의 `fung check` 출력. 페이지를 만들 때 실제 출력과 견준다(2026-10-26) |

- 예제 라벨: `원고경로:n`(`run`·`check`·`dump`를 1부터 셈). 예: `reference/functions.md:12`.

---

### Task 1: 원고 해석기 (`markup.rs`)

**Files:**
- Create: `crates/fung-cli/tests/manual_site/mod.rs`, `crates/fung-cli/tests/manual_site/markup.rs`, `crates/fung-cli/tests/manual_build.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct Page { pub path: String, pub title: String, pub lede: Option<String>,
                    pub description: Option<String>, pub blocks: Vec<Block> }
  pub enum Block { Html(String), Heading { level: u8, id: Option<String>, html: String },
                   Code(Code), Parts }
  pub struct Code { pub line: usize, pub kind: Kind, pub src: String, pub args: Vec<String>,
                    pub stdin: Option<String>, pub env: Vec<(String, String)>, pub exit: i32,
                    pub hide_output: bool, pub files: Vec<(String, String)>, pub expect: Option<String> }
  pub enum Kind { Run, Check, Frag, Dump(String), Shell, Plain, Core }
  pub fn parse(path: &str, text: &str) -> Result<Page, String>   // Err: "path:line: reason"
  pub fn examples(page: &Page) -> Vec<(String, &Code)>          // labels "path:n", Run|Check|Dump only
  pub fn escape(s: &str) -> String                               // & < > "
  ```
  `markup.rs`는 `std`만 쓰고 형제 모듈을 부르지 않는다(작업 5에서 `tools_expected.rs`가 `#[path]`로 넣는다).

- [ ] **Step 1: 실패하는 시험** (`markup.rs` 안의 `#[cfg(test)] mod tests`, `manual_build.rs`는 `mod manual_site;`만)
  - `headings_need_ids`: `"# T\n\n## 가 {#a}\n"` → `Heading{level:2, id:Some("a"), html:"가"}`; `"## 가\n"` → `Err` 에 `":3:"`(줄 번호)와 `"{#id}"`.
  - `inline_markup`: `` "`a<b` **c** [d](e.html#f) <kbd>x</kbd>" `` → `<code>a&lt;b</code> <strong>c</strong> <a href="e.html#f">d</a> <kbd>x</kbd>`.
  - `raw_html_passes_through_and_resumes`: `"<div class=\"fn\" id=\"fn-x\"><code>x()</code>\n\n```fung run\nprint(1)\n```\n\n</div>\n"` → `[Html(..div..), Code{kind:Run, src:"print(1)"}, Html("</div>")]`.
  - `fence_attributes`: ```` ```fung run args="a b" stdin=in.txt env="K=V" exit=1 hide-output ```` → 각 필드; ```` ```file in.txt ```` 두 개 뒤의 `run` → `files` 길이 2; `check` 뒤 `expect` → `expect: Some(..)`.
  - `unknown_markup_is_an_error`: 모르는 펜스 `fung runn`, 날 HTML 안 `<pre>`, 날 HTML 안 `<h2`, 같은 id 두 번, 짝 없는 `file`(뒤에 run/check 없음), 닫히지 않은 펜스 → 모두 `Err`이고 줄 번호를 담는다.
  - `example_labels`: run·frag·check·dump·shell 순서의 원고 → 라벨 `p.md:1`(run), `p.md:2`(check), `p.md:3`(dump).
- [ ] **Step 2: 실패 확인** — `timeout 300 cargo test -p fung-cli --test manual_build markup` → 컴파일 에러 또는 FAIL.
- [ ] **Step 3: 구현** — 줄 단위 상태 기계(문단/목록/날 HTML/펜스). 인라인은 코드 스팬을 먼저 떼고 나머지에 `**`·링크만 바꾼다.
- [ ] **Step 4: 통과 확인** — 같은 명령, PASS.
- [ ] **Step 5: 커밋** — `test(manual): parse the manual's manuscript format`

### Task 2: 예제 실행 (`examples.rs`)

**Files:**
- Create: `crates/fung-cli/tests/manual_site/examples.rs`
- Create: `crates/fung-cli/tests/data/manual/` 아래 시험용 원고 (이 작업에서는 `examples.md` 하나)

**Interfaces:**
- Consumes: `markup::{Code, Kind}`
- Produces:
  ```rust
  pub struct Outcome { pub output: String, pub code: i32 }
  pub fn run_all(bin: &Path, examples: &[(String, &Code)]) -> Result<HashMap<String, Outcome>, Vec<String>>
  ```
  - 예제마다 새 빈 임시 디렉터리(`std::env::temp_dir()` 아래, 끝나면 지움)에 `example.fung`과 준비물 파일을 쓰고 그곳을 작업 디렉터리로 돈다. `TZ=UTC`(`common::TZ`), 표준 입력은 `stdin` 파일 또는 닫힘, 한도 20초(넘으면 에러).
  - 순서: `fung check example.fung`(종료 0이고 `error[` 없음) → `fung fmt --check example.fung`(Run·Check만) → 실행(`run example.fung args…` 또는 `dump <단계> example.fung`).
  - 출력은 표준 출력과 표준 에러를 **한 파이프**(`std::io::pipe`)로 받아 순서를 지킨다. 임시 디렉터리의 절대 경로(정규화한 것과 아닌 것 모두)는 `.`으로 바꾼다.
  - `hide_output`이 아니면 두 번 돌려 출력이 다르면 에러: `"{label}: output differs between runs; mark the fence hide-output"`.
  - 종료 코드가 `exit`와 다르면 에러(출력 포함). 에러는 모두 모아서 돌려준다. 병렬로 돈다(`common::par_map`).

- [ ] **Step 1: 실패하는 시험** (`manual_build.rs`의 `#[test]`, 원고는 `data/manual/examples.md`)
  - `run_output_is_captured_in_order`: `print("a")` / `eprint("b")` / `print("c")` → `output == "a\nb\nc\n"`, `code == 0`.
  - `fixtures_args_and_stdin`: `file in.txt`(내용 `x\ny\n`) + `run args="1 2" stdin=in.txt`, 소스 `print(args)` / `print(read_lines("in.txt")!)` → 실제 출력 그대로(`["1", "2"]`, `["x", "y"]`).
  - `exit_code_must_match`: `parse_int("abc")` (`exit=1`) → `code == 1`, 출력에 `E0606`. 같은 것을 `exit` 없이 → `Err`에 라벨.
  - `sandbox_paths_are_dot`: `print(path.abs("in.txt"))`(`import path`, 준비물 `in.txt`) → `output == "./in.txt\n"`.
  - `nondeterministic_output_is_refused`: `print(time.now())`(`import time`) → `Err`에 `hide-output`; 같은 것에 `hide-output` → `Ok`.
  - `check_and_fmt_failures_are_reported`: `print(nope)` → `Err`에 `check`; `x=1` → `Err`에 `fmt`.
  - `dump_runs_the_stage`: `dump bytecode`, 소스 `fn double(x) = x * 2` → 출력에 `== double ==`와 `MUL`.
- [ ] **Step 2: 실패 확인** — `timeout 300 cargo test -p fung-cli --test manual_build examples` → FAIL.
- [ ] **Step 3: 구현** — `examples.rs`. 시험 바이너리 경로는 `common::fung_bin()`(`manual_build.rs`가 `mod common;`도 선언).
- [ ] **Step 4: 통과 확인** — 같은 명령, PASS.
- [ ] **Step 5: 커밋** — `test(manual): run the manuscript's examples in a sandbox`

### Task 3: 목차와 페이지 틀 (`toc.rs`, `render.rs`, 자산)

**Files:**
- Create: `crates/fung-cli/tests/manual_site/toc.rs`, `crates/fung-cli/tests/manual_site/render.rs`
- Create: `manual/assets/style.css`(지금 `docs/assets/style.css` + 시안 `manual.css`를 한 파일로, 쓰지 않게 된 `.pages`·`.toc` 사이드바 규칙은 지움), `manual/assets/manual.js`(지금 `site.js` + 시안 `build.py`의 `NAV_JS`: 버전 선택, 읽는 절 강조, 서랍), `manual/assets/wordmark.svg`(지금 `docs/index.html`의 인라인 워드마크)
- Modify: `crates/fung-cli/tests/data/manual/`(toc.txt, `a.md`, `b.md`, `c.md` 추가)

**Interfaces:**
- Consumes: `markup::{Page, Block}`, `examples::Outcome`
- Produces:
  ```rust
  // toc.rs
  pub struct Toc { pub parts: Vec<Part> }
  pub struct Part { pub key: String, pub glyph: String, pub numbered: bool,
                    pub title: String, pub blurb: String, pub who: Option<String>, pub entries: Vec<Entry> }
  pub enum Entry { Group(String), Page(String) }        // Page: manuscript path "vm/internals.md"
  pub fn parse(text: &str) -> Result<Toc, String>
  // render.rs
  pub struct Version { pub dir: String, pub name: String, pub tag: Option<String> }
  pub struct Ctx<'a> { pub toc: &'a Toc, pub pages: &'a BTreeMap<String, Page>,
                       pub results: &'a HashMap<String, Outcome>, pub version: &'a Version, pub wordmark: &'a str }
  pub fn page_html(ctx: &Ctx, path: &str) -> String
  pub fn out_path(manuscript: &str) -> String            // "vm/internals.md" -> "vm/internals.html"
  ```
- `toc.txt` 문법: `#` 주석, `part <key> <glyph> [numbered] <제목> | <설명> [| <누가>]`, `group <이름>`, `page <경로>`. `index.md`는 toc에 넣지 않는다(표지). 같은 페이지 두 번, 없는 key 형식, `part` 앞의 `page`는 에러.
- 페이지 마크업은 시안(`build.py`의 `page`·`sidebar`·`crumbs`·`pager`·`where`)과 같은 클래스 이름을 쓴다: `banner`, `mbar`·`mbar-open`·`mbar-title`·`mbar-sec`, `scrim`, `side`(id `side`)·`side-close`, `ver`·`ver-box`·`ver-name`·`ver-tag`(`select#ver[data-current]`), `parts-nav`·`chapters`·`sub`·`toc2`, `crumbs`·`crumb-part`·`sep`, `pager`·`prev`·`next`, `cover`, `run`·`out`(`err`와 `data-code`), `shell`, `dump`, `unchecked`, `fixture`. `<html>` 바로 뒤에 `js` 클래스를 붙이는 한 줄 스크립트.
- 규칙: `banner`와 `ver-tag`는 `version.tag`가 있을 때만. 사이드바에는 toc의 부분만 보이고(내용 없는 부분은 toc에 없다), 지금 부분만 장 목록을 펼친다. `numbered` 부분은 장 앞에 `n. `, 브레드크럼 끝에 `n장`. 지금 페이지 아래 `toc2`는 그 페이지의 `##` 목록. 이전/다음은 같은 부분 안에서만, 라벨은 numbered면 "이전 장/다음 장", 아니면 "이전/다음". 페이지 하나뿐인 부분은 pager 없음. 출력 칸의 텍스트는 `escape`한다. 자산 경로는 `{up}assets/…`, 공유 파일은 `{up}../versions.js`, `{up}../assets/logo/…`.
- `index.md`는 시안 표지처럼: `# 제목` 자리에 워드마크를 큰 `h1`로(`hero`), `lede`는 `p.lede`, 브레드크럼 없음, `{{parts}}` 자리에 toc 부분 카드(`who`가 있으면 그 줄), 그리고 지금 `docs/index.html`의 `og:*`·`twitter:card` 메타를 `<head>`에 (`og:url`은 `https://blog.tunarider.net/fung/latest/`).

- [ ] **Step 1: 실패하는 시험** (`manual_build.rs`, 시험 원고: 부분 둘 — `learn`(numbered, `a.md`·`b.md`), `reference`(`c.md` 하나) — 와 `index.md`)
  - `toc_parses_and_rejects`: 위 toc → 부분 둘·항목 수; 잘못된 줄들 → `Err`.
  - `latest_has_banner_and_tag_frozen_does_not`: `Version{dir:"latest", name:"latest", tag:Some("unreleased")}`로 만든 `a.html`에 `class="banner"`와 `>unreleased<`; `Version{dir:"9.9.9", name:"9.9.9", tag:None}`에는 둘 다 없고 `data-current="9.9.9"`.
  - `sidebar_marks_the_current_page`: `b.html`에 `aria-current="page"`가 `2. ` 장에, `toc2` 안에 `b.md`의 `##` id들, `reference` 부분의 장 목록은 없음.
  - `crumbs_and_pager`: `a.html` 브레드크럼이 `crumb-part` + `1장`; pager에 `다음 장`과 `b.html` 링크, `이전` 없음. `c.html`에는 `class="pager"` 없음.
  - `cover_lists_the_parts`: `index.html`에 `class="cover"`와 두 부분의 첫 페이지 링크, `og:url`.
  - `outputs_are_escaped`: 출력이 `<b>&`인 예제 → HTML에 `&lt;b&gt;&amp;`.
- [ ] **Step 2: 실패 확인** — `timeout 300 cargo test -p fung-cli --test manual_build` → FAIL.
- [ ] **Step 3: 구현** — `toc.rs`, `render.rs`, 자산 세 파일.
- [ ] **Step 4: 통과 확인** — 같은 명령, PASS.
- [ ] **Step 5: 커밋** — `test(manual): contents file and the page frame`

### Task 4: 사이트 조립, 비교·갱신·얼리기 (`site.rs`)

**Files:**
- Create: `crates/fung-cli/tests/manual_site/site.rs`
- Modify: `crates/fung-cli/tests/manual_build.rs`

**Interfaces:**
- Consumes: 작업 1~3 전부
- Produces:
  ```rust
  pub fn build(manual: &Path, bin: &Path, v: &Version) -> Result<BTreeMap<String, Vec<u8>>, String>
      // keys relative to docs/<v.dir>/: "index.html", "reference/language.html", "assets/style.css", …
  pub fn shared(versions: &[Version]) -> BTreeMap<String, Vec<u8>>
      // keys relative to docs/: "versions.js", and the old addresses (OLD below)
  pub fn versions_js(versions: &[Version]) -> String
  pub fn read_versions(js: &str) -> Vec<Version>
  pub fn check_links(files: &BTreeMap<String, Vec<u8>>) -> Vec<String>
  ```
- `versions.js` 형식(한 줄 하나, 시안과 같은 키):
  `window.FUNG_VERSIONS = [` / `{"dir": "latest", "name": "latest", "tag": "unreleased"},` / … / `];`. 순서: `latest`, 그 뒤 얼린 버전 최신순.
- 옛 주소(`OLD`): `index.html` → `latest/index.html`, `syntax.html` → `latest/reference/language.html`, `builtins.html` → `latest/reference/functions.html`, `internals.html` → `latest/vm/internals.html`. 각 파일: `<script>location.replace(목적지 + location.hash)</script>`, `<meta http-equiv="refresh" content="0; url=목적지">`, `<link rel="canonical">`, 본문에 링크 하나. `index.html`에는 지금의 `og:*`·`twitter:card` 메타도(`og:url`은 `https://blog.tunarider.net/fung/`). 앵커 대응표는 2단계에서 생긴다(지금은 앵커가 같다).
- `check_links`: 각 HTML의 `href`·`src` 중 `http(s):`·`mailto:`·`#`만이 아닌 것, `../versions.js`·`../assets/logo/…`(공유)를 뺀 상대 경로가 `files`에 있는지, `#id`가 그 파일에 있는지. `/`로 시작하는 경로는 무조건 에러.
- `manual_build.rs`의 시험:
  - `the_manual_is_up_to_date`: `build(repo/manual, fung_bin, latest)`와 `shared(read_versions(docs/versions.js))`를 `docs/latest/**`·`docs/versions.js`·`OLD` 네 파일과 비교. 다른 파일, 없는 파일, **남는 파일**(`docs/latest/` 아래 생성 목록에 없는 것)을 모두 차이로 보고하고, 메시지 끝에 `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build` 안내. `FUNG_UPDATE_MANUAL=1`이면 쓰고 남는 파일을 지운다. `docs/<얼린 버전>/`은 읽지도 쓰지도 않는다. `docs/versions.js`가 없으면 `latest` 하나로 본다.
  - `freeze_a_release`: `FUNG_MANUAL_FREEZE=x.y.z`가 없으면 아무것도 안 하고 통과. 있으면 `docs/x.y.z/`가 이미 있을 때 실패, 없으면 `Version{dir:x.y.z, name:x.y.z, tag:None}`로 만들어 쓰고 `versions.js`에 넣는다.
- [ ] **Step 1: 실패하는 시험** (시험 원고 `data/manual` 사용, 디스크 대신 메모리와 임시 디렉터리)
  - `broken_links_are_errors`: `a.md`에 `[x](nope.html)`, `[y](b.html#nope)`, `[z](/abs.html)` → `build`가 `Err`이고 셋 다 담음. 올바른 `[w](b.html#<있는 id>)`는 통과.
  - `versions_round_trip`: `read_versions(&versions_js(&[latest, 0.4.0]))` == 입력.
  - `old_addresses_forward_with_the_anchor`: `shared(..)["builtins.html"]`에 `location.replace("latest/reference/functions.html" + location.hash)`와 refresh 메타.
  - `stray_files_are_reported`: 임시 디렉터리에 생성 결과 + `latest/old.html`을 두고 비교 함수 → 차이에 `old.html`; 갱신 모드 → 지워짐.
  - `freeze_writes_a_new_version_only_once`: 임시 `docs`에 얼리기 두 번 → 첫 번째는 `9.9.9/index.html`과 `versions.js`에 `9.9.9`, 두 번째는 에러.
  (비교·갱신·얼리기는 `docs` 루트를 인자로 받는 함수로 만들어 임시 디렉터리로 시험한다. `the_manual_is_up_to_date`와 `freeze_a_release`는 그 함수를 저장소 `docs`에 부른다.)
- [ ] **Step 2: 실패 확인** — `timeout 300 cargo test -p fung-cli --test manual_build` → FAIL. (`the_manual_is_up_to_date`는 원고가 생기는 작업 5에서 더한다.)
- [ ] **Step 3: 구현** — `site.rs`, `manual_build.rs`의 비교·갱신·얼리기 함수.
- [ ] **Step 4: 통과 확인** — 같은 명령, PASS.
- [ ] **Step 5: 커밋** — `test(manual): assemble the site, check links, compare, freeze`

### Task 5: 지금 매뉴얼을 원고로 옮기고 전환

**Files:**
- Create: `manual/toc.txt`, `manual/index.md`, `manual/reference/language.md`, `manual/reference/functions.md`, `manual/vm/internals.md`, `docs/versions.js`, `docs/latest/**`(생성)
- Modify(생성으로 덮어씀): `docs/index.html`, `docs/syntax.html`, `docs/builtins.html`, `docs/internals.html`
- Delete: `docs/assets/style.css`, `docs/assets/site.js`
- Modify: `crates/fung-cli/tests/manual_build.rs`(`the_manual_is_up_to_date`), `crates/fung-cli/tests/tools_expected.rs`, `crates/fung-cli/tests/manual.rs`, `crates/fung-cli/tests/changelog.rs`, `crates/fung-cli/tests/repo_rust_only.rs`, `crates/fung-cli/tests/expected/fmt/stdin.txt`(bless)

**옮기는 법:** 일회용 변환 스크립트(커밋하지 않음, 스크래치 디렉터리)로 각 페이지의 `<main>` 안쪽을 꺼내 날 HTML로 두고, 다음만 바꾼다.
- `<h1>` → `# 제목`, `<h2 id="x">글</h2>` → `## 글 {#x}`(h3도 같게). 표지의 `hero`(워드마크 `h1`과 `p.lede`)는 `# fung`과 머리의 `lede:`로 바꾼다(생성기가 워드마크 히어로로 그린다). 그 아래 `twofold` 그림은 날 HTML로 두고 안의 두 `<pre>`만 펜스로.
- `<pre><code>` → `fung run`(지금 실행 검사를 받던 것), `class="frag"` → `fung frag`, `class="plain"` → `plain`, `class="core"` → `core`, `data-args=". 3"` → `args=". 3"`. 펜스 앞뒤에 빈 줄. HTML 엔티티는 풀어서 쓴다.
- 실행해 보니 출력이 흔들리는 예제(시각·호스트 경로 등, 생성기가 알려 준다)는 `hide-output`, 호스트의 파일에 기대는 예제(`fs.list("/var/log")` 등)는 `fung check`로.
- 페이지 사이 링크: `syntax.html#x` → `reference/language.html#x` 상대 경로(`../reference/language.html#x` 등), `builtins.html#x` → `functions.html#x`, `internals.html#x` → `vm/internals.html#x`. 생성기의 링크 검사가 남은 것을 알려 준다.
- `toc.txt`(1단계):
  ```
  part reference § 레퍼런스 | 찾아보는 곳 · 언어 명세 | 정확한 규칙을 찾는 사람
  page reference/language.md
  page reference/functions.md
  part vm VM fung VM | 언어는 어떻게 만들어지나 | 언어 구현이 궁금한 사람
  page vm/internals.md
  ```
  (페이지 제목은 지금 그대로: "문법", "내장 함수", "내부 구조".)

**시험 쪽 전환:**
- `manual_build.rs`: `the_manual_is_up_to_date`(작업 4의 설명대로).
- `tools_expected.rs`: `manual_examples()`가 `manual/**/*.md`를 `#[path = "manual_site/markup.rs"] mod markup;`으로 읽어 `Run`·`Check` 예제를 `(라벨, 소스)`로 돌려준다(라벨 `원고경로:n`). `manual_examples_check_format_and_run`·`manual_example_args`·`excluded`·HTML `unescape`와 그 시험(`manual_examples_are_extracted`의 HTML 부분)은 지운다(검사는 이제 생성기가 한다). `manual_examples_are_extracted`는 원고에서 `fn slug(s) = lower(s) |> split(" ")`이 나오는지만 본다. fmt 말뭉치는 그대로 `manual_examples()`를 쓴다 → `FUNG_EXPECTED=bless cargo test -p fung-cli --test tools_expected` 뒤 `expected/fmt/stdin.txt`의 라벨 diff를 검토하고 `FMT_INPUTS`·`FMT_STDIN_CASES`를 새 수로.
- `manual.rs`: 읽는 파일을 `docs/latest/reference/functions.html`로(생성된 HTML의 `<div class="fn" id="fn-ID"><code>SIG</code>` 형태는 원고의 날 HTML이 지킨다). 머리 주석도.
- `changelog.rs`의 `covers_the_manual_version`: `docs/versions.js`를 읽어 `tag`가 없는 첫 버전(가장 최근에 얼린 것)에 CHANGELOG 절이 있는지 본다. 없으면 통과. 시험 이름은 `covers_the_newest_frozen_manual`.
- `repo_rust_only.rs`: `COMPARISONS`의 `docs/index.html`·`docs/syntax.html`·`docs/assets/style.css` 항목을 원고(`manual/index.md`, `manual/reference/language.md`, `manual/assets/style.css`)와 생성 결과(`docs/latest/index.html`, `docs/latest/reference/language.html`, `docs/latest/assets/style.css`) 양쪽으로. 낡은 용어 검사의 문서 목록(`docs/index.html` 등 네 개)은 `manual/` 아래 모든 `.md`로. 검사 대상 최상위 목록에 `manual`을 더한다.

- [ ] **Step 1: 실패하는 시험** — `the_manual_is_up_to_date`를 더하고 `tools_expected.rs`·`manual.rs`·`changelog.rs`·`repo_rust_only.rs`를 위처럼 바꾼다.
- [ ] **Step 2: 실패 확인** — `timeout 600 cargo test -p fung-cli --test manual_build --test manual --test changelog --test repo_rust_only` → FAIL(원고·생성물이 없음).
- [ ] **Step 3: 원고 작성** — 변환 스크립트로 네 원고와 `toc.txt`를 만들고, 생성기의 에러(검사 실패, 흔들리는 출력, 깨진 링크)를 하나씩 원고에서 고친다. 문장은 바꾸지 않는다.
- [ ] **Step 4: 생성** — `timeout 600 env FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build the_manual_is_up_to_date`, 그리고 `git rm docs/assets/style.css docs/assets/site.js`.
- [ ] **Step 5: 눈으로 확인** — 브라우저(앱의 브라우저 창, 정적 서버는 `.claude/launch.json`의 `python3 -m http.server --directory docs`)로 `latest/`의 네 페이지를 데스크톱·모바일(375px)·어두운 화면에서 열어, 옛 페이지와 내용이 같은지, 서랍·버전 선택·읽는 절 강조가 시안처럼 도는지, 옛 주소 `builtins.html#fn-map`이 그 항목에 도착하는지 본다.
- [ ] **Step 6: 통과 확인** — `timeout 600 cargo test -p fung-cli --test manual_build --test manual --test changelog --test repo_rust_only`, 그리고 bless 뒤 `timeout 900 cargo test -p fung-cli --test tools_expected` PASS.
- [ ] **Step 7: 커밋** — `docs!: generate the manual from manuscripts under docs/latest`

### Task 6: 문서와 마무리

**Files:**
- Modify: `CLAUDE.md`(매뉴얼 항목 93~96줄, 54줄의 `docs/internals.html`), `docs/notes/release.md`(7절 2번), `README.md`(19줄 "원본" 링크 → `manual/`), `CHANGELOG.md`(`[Unreleased]`), `docs/design/open-questions.md`(매뉴얼 전문 검색 보류, 2~5단계 남은 일), `docs/superpowers/specs/2026-10-20-manual-structure-design.md`(상태: 1단계 끝)

- [ ] **Step 1: 고쳐 쓴다**
  - CLAUDE.md: 매뉴얼은 `manual/`의 원고에서 `docs/latest/`로 생성(`FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`), 원고 형식은 설계 문서와 이 계획의 "원고 형식", 예제는 펜스 종류에 따라 생성기가 검사·실행, 함수 레퍼런스는 `manual.rs`가 지킨다, 버전은 `docs/versions.js`.
  - release.md 7절 2번: "매뉴얼 사이드바의 버전 표기를 고친다"를 "`FUNG_MANUAL_FREEZE=x.y.z cargo test -p fung-cli --test manual_build freeze_a_release`로 `docs/x.y.z/`를 얼린다(0.4.0부터). `changelog.rs`가 가장 최근에 얼린 버전의 CHANGELOG 절을 확인한다"로.
  - CHANGELOG `[Unreleased]`에 한 항목(한국어, 기존 문체): 매뉴얼이 `https://blog.tunarider.net/fung/latest/`로 옮겼고 옛 주소는 넘겨 준다, 버전 선택·좁은 화면의 목차 서랍, 예제마다 실제 출력이 붙는다.
- [ ] **Step 2: 전체 검사** — `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(와 `--release`), `timeout 1800 cargo test --locked --no-fail-fast`(와 `--release`), 설치되어 있으면 `cargo deny check`·`cargo audit`. 모두 통과.
- [ ] **Step 3: 커밋** — `docs: the manual's new layout in the project documents`
- [ ] **Step 4: 브랜치 전체 검토** — 가장 능력 있는 모델의 새 검토자에게 브랜치 diff와 이 계획·설계 문서를 주고, 받은 지적을 고친 뒤 커밋.
