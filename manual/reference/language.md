---
description: fung의 문법: 어휘, 블록과 줄 이어짐, 값과 연산자, 함수, 패턴 매칭, 결과값, 모듈, 형식 문법.
lede: 겉은 Go와 Python을 닮았고, 블록은 <code>:</code>에서 <code>;;</code>까지다. 들여쓰기에는 의미가 없다.
---
# 문법

```fung run
# 한눈에 보는 fung
import fs

fn total_size(dir):
  fs.list(dir)!
    |> map(f => fs.stat("${dir}/${f}")!.size)
    |> fold(0, (acc, n) => acc + n)
;;

size = total_size(".")
label = size > 1_000_000 then "big" else "small"
print("${size} bytes (${label})")
```

## 명세의 쪽들 {#pages}

규칙은 주제마다 한 쪽이다. 쪽마다 규칙, 실행한 예와 출력, 경계 사례와 그 자리의 에러를 함께 적는다.

<ul>
  <li><a href="language/lexical.html">어휘</a>: 토큰의 규칙: 이름, 숫자, 문자열, 기간, 주석, 예약어.</li>
  <li><a href="language/blocks.html">문장과 블록</a>: 문장과 블록(<code>:</code> … <code>;;</code>), 줄 이어짐.</li>
  <li><a href="language/values.html">값과 타입</a>: 값의 종류와 그 성질, 문자열·리스트·맵·튜플·태그·범위.</li>
  <li><a href="language/operators.html">연산자</a>: 산술·비교·논리 연산자, 파이프 <code>|&gt;</code>, 우선순위.</li>
  <li><a href="language/access.html">인덱스·슬라이스·필드</a>: <code>x[i]</code>, <code>x[a..b]</code>, <code>x.name</code>의 규칙과 범위 밖.</li>
  <li><a href="language/scope.html">변수와 스코프</a>: 대입, 블록 스코프, 바깥 변수, 최상위 이름.</li>
  <li><a href="language/functions.html">함수</a>: <code>fn</code>, 람다, 인자 규약, 기본값, 클로저, 꼬리 호출.</li>
  <li><a href="language/conditions.html">조건</a>: <code>if</code>, <code>then … else</code>, <code>and</code>·<code>or</code>, 값 자리의 조건.</li>
  <li><a href="language/match.html">match와 패턴</a>: <code>match</code>, 패턴의 종류, guard, 구조 분해 대입.</li>
  <li><a href="language/for.html">for</a>: <code>for</code>의 값, 순회 대상, 버려지는 자리.</li>
  <li><a href="language/results.html">결과값과 에러</a>: <code>Ok</code>·<code>Err</code>, <code>!</code>·<code>?</code>·<code>??</code>, 버려지는 <code>Err</code>.</li>
  <li><a href="language/modules.html">모듈</a>: <code>import</code>, 모듈 찾기, 공개 이름.</li>
  <li><a href="language/types.html">타입 표기</a>: 매개변수와 반환의 타입 표기와 그 검사.</li>
  <li><a href="language/testing.html">테스트와 디버깅</a>: <code>test</code> 블록, <code>assert</code>, <code>dbg</code>.</li>
  <li><a href="language/grammar.html">형식 문법</a>: fung의 EBNF.</li>
</ul>

## 옛 앵커 {#old-anchors}

이 쪽이 한 장이던 때의 주소(<code>language.html#values</code>)는 여기로 온다. 이어지는 곳:

<ul>
  <li id="lexical"><a href="language/lexical.html"><code>#lexical</code></a></li>
  <li id="blocks"><a href="language/blocks.html"><code>#blocks</code></a></li>
  <li id="continuation"><a href="language/blocks.html#continuation"><code>#continuation</code></a></li>
  <li id="values"><a href="language/values.html"><code>#values</code></a></li>
  <li id="operators"><a href="language/operators.html"><code>#operators</code></a></li>
  <li id="pipe"><a href="language/operators.html#pipe"><code>#pipe</code></a></li>
  <li id="precedence"><a href="language/operators.html#precedence"><code>#precedence</code></a></li>
  <li id="access"><a href="language/access.html"><code>#access</code></a></li>
  <li id="scope"><a href="language/scope.html"><code>#scope</code></a></li>
  <li id="functions"><a href="language/functions.html"><code>#functions</code></a></li>
  <li id="conditions"><a href="language/conditions.html"><code>#conditions</code></a></li>
  <li id="match"><a href="language/match.html"><code>#match</code></a></li>
  <li id="for"><a href="language/for.html"><code>#for</code></a></li>
  <li id="results"><a href="language/results.html"><code>#results</code></a></li>
  <li id="modules"><a href="language/modules.html"><code>#modules</code></a></li>
  <li id="types"><a href="language/types.html"><code>#types</code></a></li>
  <li id="testing"><a href="language/testing.html"><code>#testing</code></a></li>
  <li id="grammar"><a href="language/grammar.html"><code>#grammar</code></a></li>
</ul>
