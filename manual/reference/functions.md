---
description: fung의 prelude 함수와 표준 모듈(fs, path, cli, json, time, regex, http, conc) 참조.
lede: import 없이 쓰는 prelude와, 이름으로 가져오는 표준 모듈. 모든 서명은 런타임의 실제 매개변수 표와 맞춰 본다(테스트가 지킨다).
---
# 내장 함수

## 읽는 법 {#reading}

<ul>
  <li><code>f(a, b = 1, ...rest)</code>: <code>b</code>는 기본값이 있고 <code>rest</code>는 남는 위치 인자를 리스트로 받는다. 내장 함수도 이름 붙인 인자를 받는다: <code>exec(cmd, cwd: "/tmp")</code>.</li>
  <li>모든 함수는 "대상이 첫 번째 인자"라서 파이프로 이어진다: <code>xs |&gt; filter(f) |&gt; map(g)</code>는 <code>map(filter(xs, f), g)</code>와 같다(<a href="language.html#pipe">파이프</a>).</li>
  <li>외부 요인으로 실패할 수 있는 함수는 <code>Ok(값)</code>/<code>Err({kind, message, …})</code>를 돌려준다. <code>kind</code>는 심볼이다: <code>'not_found</code> <code>'exists</code> <code>'permission</code> <code>'io</code> <code>'parse</code> <code>'invalid</code> <code>'timeout</code> <code>'closed</code> <code>'cancelled</code> <code>'error</code> <code>'exit</code> <code>'network</code>.</li>
  <li>인자 타입이 틀린 것은 코드 오류라 런타임 에러(E0403)다.</li>
  <li><span class="by" style="margin-left:0">fung으로 작성</span> 표시는 fung 코드로 쓴 함수다. 나머지는 Rust로 구현했다(<a href="../vm/internals.html#layers">네 층</a>).</li>
</ul>
<div class="filter">
  <label for="fn-search" class="note">이름이나 설명으로 찾기</label><br>
  <input id="fn-search" type="search" placeholder="예: split, 정렬, Err" autocomplete="off">
  <p id="fn-count">135개 함수</p>
</div>

## prelude: 늘 쓸 수 있는 함수 {#prelude}

<p>import 없이 쓴다. 같은 이름의 변수나 함수를 만들면 경고(W0103)가 난다.</p>
<section class="group" id="io">
<h3>출력과 입력</h3>
<p>표준 출력·에러에 쓰고, 표준 입력이나 파일 핸들에서 줄을 읽는다.</p>
<div class="fn" id="fn-print"><code>print(...args)</code>
  <p>인자들을 공백으로 이어 한 줄로 표준 출력에 쓴다. 문자열은 따옴표 없이, 나머지는 리터럴 형태로 찍는다. 태스크 여럿이 동시에 찍어도 줄이 섞이지 않는다.</p>

```fung run
print("total:", 3, [1, 2], 'ok)
```

</div>
<div class="fn" id="fn-eprint"><code>eprint(...args)</code>
  <p><code>print</code>와 같지만 표준 에러에 쓴다.</p>
</div>
<div class="fn" id="fn-read_line"><code>read_line(f = nil)</code>
  <p>한 줄을 읽어 <code>Ok(줄)</code>, 끝이면 <code>Ok(nil)</code>. 줄 끝의 <code>\n</code>·<code>\r</code>는 뗀다. <code>f</code>가 <code>nil</code>이면 표준 입력, 파일 핸들이면 그 파일. 닫힌 핸들이면 <code>Err({kind: 'closed})</code>. UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD(<code>�</code>) 하나로 바뀐다.</p>
</div>
<div class="fn" id="fn-read_lines"><code>read_lines(f = nil)</code>
  <p>남은 줄 전부를 <code>Ok(리스트)</code>로. 규칙은 <code>read_line</code>과 같다.</p>
</div>
<div class="fn" id="fn-dbg"><code>dbg(x)</code>
  <p><code>[파일:줄] 식 = 값</code>을 표준 에러에 찍고 <code>x</code>를 그대로 돌려준다. 식의 소스 텍스트는 컴파일러가 채운다.</p>
</div>
</section>
<section class="group" id="convert">
<h3>타입과 변환</h3>
<p><code>int</code>·<code>float</code>·<code>str</code>·<code>sym</code>은 잘못된 입력에 런타임 에러를 낸다(코드가 틀린 것으로 본다). 믿을 수 없는 입력은 <code>parse_*</code>로 검사한다.</p>
<div class="fn" id="fn-type_of"><code>type_of(x)</code>
  <p>값의 타입 이름을 심볼로: <code>'int</code> <code>'str</code> <code>'list</code> <code>'map</code> <code>'fn</code> <code>'atom</code> …, 태그 값이면 태그 이름(<code>'Ok</code>, <code>'Circle</code>).</p>

```fung run
print(type_of(1.5), type_of(Ok(1)), type_of('x))
```

</div>
<div class="fn" id="fn-int"><code>int(x)</code>
  <p>문자열(<code>"42"</code>, <code>"-1_000"</code>)이나 실수(0 쪽으로 버림)를 정수로. 변환할 수 없으면 E0513.</p>

```fung run
print(int("42") + 1, int(-3.9))
```

</div>
<div class="fn" id="fn-float"><code>float(x)</code>
  <p>정수나 문자열을 실수로. 문자열은 실수 리터럴 문법(앞의 부호, 정수 꼴 허용)만 받는다: <code>"2.5"</code>, <code>"-1e3"</code>, <code>"42"</code>. <code>".5"</code>·<code>"1."</code>·16진 실수는 거부한다.</p>
</div>
<div class="fn" id="fn-str"><code>str(x)</code>
  <p>문자열이면 그대로, 아니면 리터럴 형태의 문자열.</p>
</div>
<div class="fn" id="fn-sym"><code>sym(x)</code>
  <p>이름 모양의 문자열을 심볼로(<code>sym("ok") == 'ok</code>). 만든 심볼은 해제되지 않으므로 요청 값 같은 외부 입력을 그대로 넣지 않는다(정해진 이름만 바꾼다).</p>
</div>
<div class="fn" id="fn-parse_int"><code>parse_int(s)</code>
  <p><code>Ok(정수)</code> 또는 <code>Err({kind: 'parse, message})</code>. 공백은 허용하지 않는다.</p>

```fung run
print(parse_int("1_000"), parse_int("x"))
```

</div>
<div class="fn" id="fn-parse_float"><code>parse_float(s)</code>
  <p><code>Ok(실수)</code> 또는 <code>Err('parse)</code>. 문법은 <code>float</code>과 같다.</p>
</div>
<div class="fn" id="fn-parse_dur"><code>parse_dur(s)</code>
  <p>기간 리터럴 문법(<code>1h30m</code>, <code>500ms</code>, <code>-5s</code>, 앞뒤 공백 허용)을 <code>Ok(dur)</code> 또는 <code>Err('parse)</code>로. <code>--timeout 30s</code> 같은 옵션에 쓴다.</p>

```fung run
print(parse_dur("90s")!, parse_dur("soon"))
```

</div>
</section>
<section class="group" id="collections">
<h3>컬렉션</h3>
<p>리스트를 받는 함수는 범위와 채널도 받는다. 채널은 값이 올 때마다 처리하고, 일찍 멈추는 함수(<code>find</code>·<code>any</code>·<code>all</code>·<code>take</code>·<code>first</code>)는 남은 값을 채널에 둔다. 전체가 필요한 함수(<code>sort</code>·<code>reverse</code>·<code>last</code> 등)는 닫힐 때까지 받는다. 결과가 모든 원소를 담는 함수는 원소 1억 개를 넘는 범위를 E0806으로 거절한다. 원소를 도는 함수는 Rust로, 그 위의 조합은 fung으로 썼다.</p>
<div class="fn" id="fn-len"><code>len(x)</code>
  <p>문자열(글자 수), 리스트, 맵, 튜플, 범위의 길이.</p>

```fung run
print(len("참치"), len([1, 2]), len(0..10))
```

</div>
<div class="fn" id="fn-is_empty"><code>is_empty(x)</code><span class="by">fung으로 작성</span>
  <p><code>len(x) == 0</code>.</p>
</div>
<div class="fn" id="fn-map"><code>map(xs, f)</code>
  <p><code>f(x)</code>를 모은 새 리스트.</p>

```fung run
print([1, 2, 3] |> map(x => x * 10))
```

  <p><code>map</code>에 바로 쓴 람다 안의 <code>?</code>는 컴파일 에러 E0226이다(<code>Err</code>가 원소가 되므로). 실패할 수 있는 변환은 <a href="#fn-try_map"><code>try_map</code></a>.</p>
</div>
<div class="fn" id="fn-try_map"><code>try_map(xs, f)</code>
  <p><code>f(x)</code>가 돌려준 결과값을 모은다. 모두 <code>Ok(v)</code>면 <code>Ok([v…])</code>, 처음 나온 <code>Err</code>는 그대로 돌려주고 남은 원소는 부르지 않는다. <code>f</code>가 결과값이 아닌 값을 돌려주면 E0605. 이미 만든 결과값 리스트는 <code>try_map(rs, r =&gt; r)</code>.</p>

```fung run
print(["1", "2"] |> try_map(parse_int))
print(["1", "x"] |> try_map(parse_int))
```

</div>
<div class="fn" id="fn-filter"><code>filter(xs, f)</code>
  <p><code>f(x)</code>가 <code>true</code>인 원소만. <code>f</code>는 bool을 돌려줘야 한다.</p>
</div>
<div class="fn" id="fn-fold"><code>fold(xs, init, f)</code>
  <p>왼쪽부터 접는다: <code>f(f(init, x1), x2)</code>…</p>

```fung run
print(fold([1, 2, 3], 0, (acc, x) => acc + x))
```

</div>
<div class="fn" id="fn-each"><code>each(xs, f)</code>
  <p>부수효과용 순회. <code>nil</code>을 돌려준다(<code>for</code>는 리스트를 만든다). <code>f</code>의 결과는 버리므로, <code>Err</code>면 E0606이다(<a href="language.html#results">버려지는 <code>Err</code></a>).</p>
</div>
<div class="fn" id="fn-find"><code>find(xs, f)</code>
  <p><code>f(x)</code>가 참인 첫 원소, 없으면 <code>nil</code>.</p>
</div>
<div class="fn" id="fn-any"><code>any(xs, f)</code>
  <p>하나라도 참이면 <code>true</code>.</p>
</div>
<div class="fn" id="fn-all"><code>all(xs, f)</code>
  <p>모두 참이면 <code>true</code>. 빈 리스트는 <code>true</code>.</p>
</div>
<div class="fn" id="fn-sort"><code>sort(xs)</code>
  <p>비교 표의 순서로 정렬한 새 리스트. 안정 정렬이라 같은 값은 원래 순서를 지킨다. 비교할 수 없는 값이 섞이면 처음 실패한 비교에서 에러.</p>
</div>
<div class="fn" id="fn-sort_by"><code>sort_by(xs, f)</code>
  <p><code>f(x)</code>를 키로 안정 정렬. 내림차순은 키에 <code>-</code>를 붙이거나 <code>reverse</code>.</p>

```fung run
print(["bb", "a", "ccc"] |> sort_by(len))
```

</div>
<div class="fn" id="fn-reverse"><code>reverse(xs)</code><span class="by">fung으로 작성</span>
  <p>거꾸로 뒤집은 리스트.</p>
</div>
<div class="fn" id="fn-unique"><code>unique(xs)</code>
  <p>처음 나온 순서를 지키며 중복을 뺀다(<code>==</code> 기준, 선형 시간).</p>
</div>
<div class="fn" id="fn-flatten"><code>flatten(xs)</code>
  <p>한 단계만 펼친다: <code>[[1, [2]], 3]</code> → <code>[1, [2], 3]</code>.</p>
</div>
<div class="fn" id="fn-group_by"><code>group_by(xs, f)</code>
  <p><code>f(x)</code>(문자열이어야 한다)를 키로 묶은 맵. 키와 각 목록은 처음 나온 순서. 맵 키는 문자열이므로 수로 묶을 때는 <code>str</code>로 바꾼다(아니면 E0514): <code>group_by(xs, x =&gt; str(x % 2))</code>.</p>

```fung run
print(group_by(["ab", "c", "de"], s => str(len(s))))
```

</div>
<div class="fn" id="fn-zip"><code>zip(xs, ys)</code><span class="by">fung으로 작성</span>
  <p>짝지은 튜플의 리스트. 짧은 쪽 길이까지.</p>
</div>
<div class="fn" id="fn-first"><code>first(xs)</code><span class="by">fung으로 작성</span>
  <p>첫 원소, 비었으면 <code>nil</code>.</p>
</div>
<div class="fn" id="fn-last"><code>last(xs)</code><span class="by">fung으로 작성</span>
  <p>마지막 원소, 비었으면 <code>nil</code>.</p>
</div>
<div class="fn" id="fn-take"><code>take(xs, n)</code><span class="by">fung으로 작성</span>
  <p>앞에서 최대 <code>n</code>개. 넘쳐도 에러가 아니다(슬라이스와 다른 점).</p>
</div>
<div class="fn" id="fn-drop"><code>drop(xs, n)</code><span class="by">fung으로 작성</span>
  <p>앞의 <code>n</code>개를 뺀 나머지. 넘쳐도 에러가 아니다.</p>
</div>
<div class="fn" id="fn-range"><code>range(a, b = nil)</code><span class="by">fung으로 작성</span>
  <p><code>range(n)</code>은 <code>0..n</code>, <code>range(a, b)</code>는 <code>a..b</code>.</p>
</div>
<div class="fn" id="fn-sum"><code>sum(xs)</code><span class="by">fung으로 작성</span>
  <p>숫자의 합. 빈 리스트는 0.</p>
</div>
<div class="fn" id="fn-min"><code>min(xs)</code><span class="by">fung으로 작성</span>
  <p>가장 작은 값, 비었으면 <code>nil</code>.</p>
</div>
<div class="fn" id="fn-max"><code>max(xs)</code><span class="by">fung으로 작성</span>
  <p>가장 큰 값, 비었으면 <code>nil</code>.</p>
</div>
<div class="fn" id="fn-contains"><code>contains(c, x)</code><span class="by">fung으로 작성</span>
  <p><code>x in c</code>와 같다: 리스트 원소, 맵 키, 부분 문자열.</p>
</div>
</section>
<section class="group" id="maps">
<h3>맵</h3>
<p>맵은 불변이다. 아래 함수들은 모두 새 맵을 돌려준다.</p>
<div class="fn" id="fn-get"><code>get(m, k, default = nil)</code><span class="by">fung으로 작성</span>
  <p>키가 있으면 값, 없으면 <code>default</code>. 맵이 아니면 에러.</p>

```fung run
print(get({a: 1}, "b", 0))
```

</div>
<div class="fn" id="fn-put"><code>put(m, k, v)</code>
  <p><code>k</code>를 <code>v</code>로 둔 사본. 기존 키면 자리를 지킨다.</p>
</div>
<div class="fn" id="fn-merge"><code>merge(a, b)</code>
  <p>둘을 합친 사본. 같은 키는 <code>b</code>가 이긴다.</p>
</div>
<div class="fn" id="fn-remove"><code>remove(m, k)</code>
  <p><code>k</code>를 뺀 사본. 없는 키면 그대로.</p>
</div>
<div class="fn" id="fn-keys"><code>keys(m)</code><span class="by">fung으로 작성</span>
  <p>키의 리스트(넣은 순서).</p>
</div>
<div class="fn" id="fn-values"><code>values(m)</code><span class="by">fung으로 작성</span>
  <p>값의 리스트.</p>
</div>
<div class="fn" id="fn-pairs"><code>pairs(m)</code>
  <p><code>(키, 값)</code> 튜플의 리스트. <code>for (k, v) in pairs(m):</code>로 돈다.</p>
</div>
</section>
<section class="group" id="strings">
<h3>문자열</h3>
<p>인덱스와 길이는 글자 단위다.</p>
<div class="fn" id="fn-chars"><code>chars(s)</code>
  <p>글자 하나짜리 문자열의 리스트.</p>
</div>
<div class="fn" id="fn-split"><code>split(s, sep)</code>
  <p><code>sep</code>로 나눈다. 빈 조각도 남긴다. <code>sep</code>가 <code>""</code>면 글자 단위.</p>

```fung run
print(split("a,,b", ","))
```

</div>
<div class="fn" id="fn-join"><code>join(xs, sep = &quot;&quot;)</code>
  <p>원소를 <code>sep</code>로 잇는다. 문자열이 아닌 원소는 <code>str</code>로 바꾼다.</p>
</div>
<div class="fn" id="fn-lines"><code>lines(s)</code><span class="by">fung으로 작성</span>
  <p>줄 리스트. <code>\n</code>으로 나누고 줄 끝의 <code>\r</code>을 떼며, 마지막 줄바꿈은 빈 줄을 만들지 않는다. <code>split(s, "\n")</code> 대신 쓴다.</p>

```fung run
print(lines("a\nb\n"))
```

</div>
<div class="fn" id="fn-trim"><code>trim(s, cut = nil)</code>
  <p>양 끝의 공백을, <code>cut</code>을 주면 그 글자들을 지운다.</p>

```fung run
print(trim("  x "), trim("--x--", "-"))
```

</div>
<div class="fn" id="fn-upper"><code>upper(s)</code>
  <p>대문자로. 유니코드 기본 대소문자 변환(전체 매핑)이라 길이가 바뀔 수 있다: <code>upper("straße")</code>는 <code>"STRASSE"</code>.</p>
</div>
<div class="fn" id="fn-lower"><code>lower(s)</code>
  <p>소문자로. 전체 매핑이고, 단어 끝의 시그마는 <code>ς</code>가 된다: <code>lower("ΟΔΟΣ")</code>는 <code>"οδος"</code>.</p>
</div>
<div class="fn" id="fn-replace"><code>replace(s, old, new)</code>
  <p>모든 <code>old</code>를 <code>new</code>로. 패턴이 필요하면 <code>regex.replace</code>. 결과가 1억 글자를 넘으면 만들기 전에 E0806.</p>
</div>
<div class="fn" id="fn-starts_with"><code>starts_with(s, prefix)</code>
  <p>접두사 검사.</p>
</div>
<div class="fn" id="fn-ends_with"><code>ends_with(s, suffix)</code>
  <p>접미사 검사.</p>
</div>
<div class="fn" id="fn-repeat"><code>repeat(s, n)</code>
  <p><code>s</code>를 <code>n</code>번 이어 붙인다(<code>"a" * 3</code>은 없다). 결과가 1억 글자를 넘으면 E0806.</p>
</div>
<div class="fn" id="fn-pad_left"><code>pad_left(s, n, pad = &quot; &quot;)</code><span class="by">fung으로 작성</span>
  <p>왼쪽을 <code>pad</code>로 채워 <code>n</code>글자로. 이미 길면 그대로.</p>

```fung run
print(pad_left("7", 3, "0"), "[" + pad_right("ab", 4) + "]")
```

</div>
<div class="fn" id="fn-pad_right"><code>pad_right(s, n, pad = &quot; &quot;)</code><span class="by">fung으로 작성</span>
  <p>오른쪽을 채운다.</p>
</div>
</section>
<section class="group" id="results">
<h3>결과값</h3>
<p><code>Ok</code>·<code>Err</code>는 평범한 태그다. 풀 때는 주로 <code>!</code> <code>?</code> <code>??</code>나 <code>match</code>를 쓴다(<a href="language.html#results">문법</a>).</p>
<div class="fn" id="fn-is_ok"><code>is_ok(x)</code><span class="by">fung으로 작성</span>
  <p><code>Ok(…)</code>이면 <code>true</code>.</p>
</div>
<div class="fn" id="fn-is_err"><code>is_err(x)</code><span class="by">fung으로 작성</span>
  <p><code>Err(…)</code>이면 <code>true</code>.</p>
</div>
</section>
<section class="group" id="loops">
<h3>반복 패턴</h3>
<p><code>while</code> 대신 쓰는 꼬리 재귀 도우미들. 모두 fung으로 썼다.</p>
<div class="fn" id="fn-until"><code>until(x, f, pred)</code><span class="by">fung으로 작성</span>
  <p><code>pred(x)</code>가 참이 될 때까지 <code>x = f(x)</code>를 되풀이한 값.</p>

```fung run
print(until(1, x => x * 2, x => x > 100))
```

</div>
<div class="fn" id="fn-iterate"><code>iterate(x, f, n)</code><span class="by">fung으로 작성</span>
  <p><code>f</code>를 <code>n</code>번 적용한 값.</p>
</div>
</section>
<section class="group" id="program">
<h3>에러와 종료</h3>
<div class="fn" id="fn-fail"><code>fail(...args)</code>
  <p>인자를 <code>print</code>처럼 이은 메시지로 런타임 에러(E0803)를 낸다. "여기 오면 버그"를 말할 때 쓴다: <code>x ?? fail("x is required")</code>.</p>
</div>
<div class="fn" id="fn-with"><code>with(acquire, use, release)</code>
  <p><code>acquire()</code>로 얻은 것을 <code>use</code>에 넘기고, <code>use</code>가 어떻게 끝나든(값, <code>Err</code>, 런타임 에러, 취소, <code>exit</code>) <code>release</code>를 부른다. 결과는 <code>use</code>의 값이다. <code>release</code>는 그것을 부른 취소나 기한에 끊기지 않는다. <code>release</code>의 실패는 버리지 않는다: <code>use</code>도 실패했으면 <code>use</code>의 에러에 <code>note: also failed in release</code>로 붙고, 아니면 <code>release</code>의 에러가 난다. <code>release</code>의 결과는 버리므로 <code>Err</code>를 돌려주면 그것이 <code>release</code>의 에러 E0606이다(실패해도 되는 정리는 <code>fn(c): _ = close(c) ;;</code>). 프로그램 끝의 정리도 이것으로 한다(<code>at_exit</code>는 없다).</p>

```fung run
print(with(() => "conn", c => c + " used", c => print("closed", c)))
```

</div>
<div class="fn" id="fn-exit"><code>exit(code = 0)</code>
  <p>어느 태스크에서 불러도 프로그램을 끝낸다. 남은 태스크는 취소되고, 되감기가 지나는 모든 <a href="#fn-with"><code>with</code></a>의 <code>release</code>가 돈 뒤 그 코드로 끝난다.</p>
</div>
<div class="fn" id="fn-args"><code>args</code>
  <p>스크립트 경로 뒤의 명령행 인자(문자열 리스트). 함수가 아니라 값이다.</p>
</div>
</section>
<section class="group" id="process">
<h3>프로세스와 환경</h3>
<p>명령은 쉘을 거치지 않는 <code>exec</code>가 기본이다. 종료 코드 0이 아니면 <code>Err</code>이므로 <code>exec(…)!</code>가 쉘의 <code>set -e</code>와 같다.</p>
<div class="fn" id="fn-exec"><code>exec(cmd, cwd = nil, env = nil, stdin = nil)</code>
  <p><code>cmd</code>는 문자열 리스트(<code>["git", "status"]</code>). 성공하면 <code>Ok({out, err, code})</code>, 0이 아닌 종료는 <code>Err({kind: 'exit, code, out, err, message})</code>, 실행 파일이 없거나 시작할 수 없으면 <code>Err('not_found)</code>(이유는 <code>command not found in PATH</code>, <code>not an executable file (run scripts with sh)</code> 등). <code>env</code>는 현재 환경에 덧붙일 맵. 부른 태스크가 취소되면(<code>timeout</code>, <code>exit</code>, 범위의 취소) 명령에 SIGTERM, 2초 뒤에도 남아 있으면 SIGKILL을 보내고 취소로 되감긴다. 터미널 앞(fung이 터미널의 전경 그룹)에서는 명령도 fung과 같은 그룹이라 비밀번호 프롬프트처럼 터미널을 쓸 수 있고 Ctrl-C도 셸처럼 명령에 간다. 이때 취소는 명령 하나에만 간다. 터미널이 없으면(서비스·컨테이너·CI·cron) 명령은 자기 프로세스 그룹으로 돌고, 취소는 그룹 전체에 가서 셸이 띄운 손자까지 끝난다.</p>

```fung run
r = exec(["echo", "hi"])!
print(trim(r.out))
```

</div>
<div class="fn" id="fn-sh"><code>sh(cmd, cwd = nil, env = nil, stdin = nil)</code>
  <p><code>/bin/sh -c cmd</code>. 파이프나 글로브 같은 쉘 기능이 필요할 때만.</p>
</div>
<div class="fn" id="fn-env"><code>env(name = nil)</code>
  <p>환경 변수 값(문자열) 또는 <code>nil</code>. 인자가 없으면 전체를 맵으로.</p>

```fung run
port = env("PORT") ?? "8080"
print(port)
```

</div>
</section>
<section class="group" id="concurrency">
<h3>동시성</h3>
<p>태스크는 범위(<code>scope</code>·<code>supervise</code>) 안에서만 띄운다. 범위는 본문과 그 안의 태스크가 모두 끝나야 돌아오므로, 태스크의 실패는 언제나 범위를 연 코드로 간다. 실행 스레드 하나가 모든 태스크를 차례로 돌린다(태스크는 운영체제 스레드가 아니다, <a href="../vm/internals.html#concurrency">런타임</a>). 값은 불변이라 태스크 사이에 그냥 넘긴다. 태스크 사이의 상태는 채널이나 <code>atom</code>으로 나눈다. 병렬 <code>map</code> 같은 도우미는 <a href="#conc"><code>conc</code> 모듈</a>에 있다.</p>
<div class="fn" id="fn-scope"><code>scope(f)</code>
  <p>범위를 열고 <code>f(s)</code>를 부른다(<code>s</code>는 범위 핸들). 본문과 <code>spawn(s, g)</code>로 띄운 태스크가 모두 끝나야 돌아오고, 결과는 본문의 반환값이다. 태스크 하나가 런타임 에러로 죽으면 나머지와 본문을 취소하고 기다린 뒤 그 에러를 여기서 다시 낸다(<code>note: scope opened at …</code>가 붙는다). 함수가 <code>Err</code> 값을 돌려준 것은 실패가 아니다. <code>cancel(s)</code>로 취소된 범위는 <code>Err('cancelled)</code>를 돌려준다.</p>

```fung run
r = scope(fn(s):
  a = spawn(s, () => 6 * 7)
  b = spawn(s, () => 1 + 1)
  [await(a), await(b)]
;;)
print(r)
```

</div>
<div class="fn" id="fn-supervise"><code>supervise(f, on_fail)</code>
  <p><code>scope</code>와 같지만 태스크의 런타임 에러가 다른 태스크나 본문에 번지지 않고 <code>on_fail(e)</code>로 간다(<code>e = {kind: 'error, code, message, at}</code>: 에러 번호 <code>"E0502"</code>, 문장, 난 곳 <code>"x.fung:3"</code>). <code>await</code>도 그 <code>Err</code>를 준다. <code>on_fail</code>은 실패 순서대로 하나씩 돈다. <code>on_fail</code>이 런타임 에러를 내거나 <code>Err</code>를 돌려주면(결과를 버리므로 E0606) 범위가 실패한다. 범위가 취소되어 처리하지 못한 실패는 <code>task failed:</code>로 보고되고 종료 코드가 1이 된다.</p>

```fung run
supervise(fn(s):
  spawn(s, () => fail("boom"))
  spawn(s, () => print("still runs"))
;;, e => eprint("handled: ${e.message}"))
```

</div>
<div class="fn" id="fn-spawn"><code>spawn(s, f)</code>
  <p>범위 <code>s</code> 안에서 <code>f()</code>를 새 태스크로 실행하고 태스크 핸들을 돌려준다. 범위가 기다리는 동안에는 태스크도 같은 범위에 <code>spawn</code>할 수 있다. <code>scope(</code>가 돌아온 뒤의 범위에 <code>spawn</code>하면 E0706이다.</p>
</div>
<div class="fn" id="fn-await"><code>await(t)</code>
  <p>태스크의 결과. 정상 종료면 반환값, 런타임 에러로 죽었으면 <code>Err({kind: 'error, code, message, at})</code>(<a href="#fn-supervise"><code>supervise</code></a>와 같은 맵), 취소됐으면 <code>Err('cancelled)</code>. 여러 번 불러도 같은 결과. 기다리는 시간을 정하려면 <a href="#fn-timeout"><code>timeout</code></a>으로 감싼다(태스크는 계속 돈다).</p>

```fung run
print(scope(s => await(spawn(s, () => 6 * 7))))
```

</div>
<div class="fn" id="fn-timeout"><code>timeout(d, f)</code>
  <p><code>f()</code>의 값. <code>d</code>가 지나면 <code>f</code> 안의 모든 것(그 안에서 연 범위의 태스크 포함)을 취소하고 <code>Err({kind: 'timeout})</code>을 준다. 겹치면 이른 기한이 이긴다: 바깥 기한이 먼저 지나면 바깥 <code>timeout</code>이 답한다. <code>d</code>가 0 이하면 언제나 <code>Err({kind: 'timeout})</code>이다. 외부 명령(<code>exec</code>·<code>sh</code>)은 끝낸다. 경로만 읽는 파일 함수(<code>fs.read</code>·<code>read_lines</code>·<code>list</code>·<code>glob</code>·<code>walk</code>·<code>exists</code>·<code>stat</code>)와 정규식 컴파일도 곧바로 끊긴다. 다만 운영체제 스레드는 멈출 수 없어서, 그 읽기는 뒤에서 끝까지 돈다: FIFO나 장치(<code>/dev/stdin</code> 등)를 읽던 것이면 나중에 온 바이트를 가져가 버린다. 무엇을 바꾸는 파일 함수(쓰기, 지우기, 옮기기)와 핸들 읽기·쓰기(<code>read_line(f)</code> 등)는 이미 시작했으면 끝날 때까지 끊기지 않고, 끝나는 즉시 끊긴다. 취소된 쓰기가 나중에 일어나지 않게 하려는 것이다(<a href="../vm/internals.html#concurrency">런타임</a>).</p>

```fung run
print(timeout(1s, () => 1 + 1))
(_tx, rx) = channel()
print(timeout(10ms, () => recv(rx)))
```

</div>
<div class="fn" id="fn-cancel"><code>cancel(t)</code>
  <p>태스크나 범위를 취소한다. 그 아래의 태스크에도 전파된다. 취소는 실패가 아니라 종료 코드에 영향이 없다.</p>
</div>
<div class="fn" id="fn-channel"><code>channel(n = 0)</code>
  <p>채널의 두 끝 <code>(tx, rx)</code>: 보내는 끝과 받는 끝. <code>n</code>은 버퍼 크기이고, 0이면 받는 쪽이 있을 때까지 <code>send</code>가 막힌다. 출력은 <code>&lt;sender&gt;</code>·<code>&lt;receiver&gt;</code>, <code>type_of</code>는 <code>'sender</code>·<code>'receiver</code>, 타입 표기도 <code>sender</code>·<code>receiver</code>. 끝은 복사할 수 있다(생산자 여럿, 일꾼 여럿). 받는 끝은 <code>for</code>와 <code>map</code> 등으로 순회할 수 있다. 값이 올 때마다 처리하고 닫히면 끝난다.</p>
  <p><b>자동 닫힘</b>: 보내는 끝의 마지막 사본이 사라지면 채널이 닫힌다(받는 쪽은 남은 값을 받은 뒤 <code>Err({kind: 'closed})</code>). 받는 끝의 마지막 사본이 사라지면 닫히고 버퍼를 버리며, 그 뒤의 <code>send</code>는 <code>Err({kind: 'closed})</code>다. 지역 변수는 마지막으로 읽힌 문장 뒤에(한 번도 읽지 않으면 함수가 돌아올 때), 태스크가 붙잡은 값은 태스크가 끝날 때, 최상위 변수는 프로그램이 끝날 때 사라진다(<a href="language.html#scope">값의 수명</a>). 그래서 함수 안에서 만든 채널의 보내는 끝을 생산 태스크에만 넘기면 생산이 끝날 때 저절로 닫힌다. 교착이 나면 보고가 "받는 쪽이 보내는 끝을 스스로 쥐었다"를 알려 준다.</p>

```fung run
fn numbers(s, n):
  (tx, rx) = channel()
  spawn(s, () => each(0..n, i => send(tx, i)))
  rx
;;

scope(fn(s):
  for x in numbers(s, 3):
    print(x)
  ;;
;;)
```

</div>
<div class="fn" id="fn-send"><code>send(tx, v)</code>
  <p><code>Ok(nil)</code>, 닫혔거나 받는 끝이 모두 사라졌으면 <code>Err({kind: 'closed})</code>. <code>tx</code>는 보내는 끝이어야 한다(받는 끝이면 E0403). 채널에 그 채널의 끝을 보내면(어느 끝이든, <code>v</code> 안 어디에 들어 있어도) 런타임 에러 E0705다. 메모리를 참조 계수로 회수하므로 자기 끝을 담은 채널은 닫히지도 해제되지도 않기 때문이다. 다른 채널의 끝(응답 채널)은 보낼 수 있다. 채널 끝 같은 바뀌는 것을 품지 않은 값은 훑지 않는다.</p>
</div>
<div class="fn" id="fn-recv"><code>recv(rx)</code>
  <p><code>Ok(v)</code>, 닫혔고 남은 값이 없으면 <code>Err({kind: 'closed})</code>. <code>rx</code>는 받는 끝이어야 한다. 기다리는 시간은 <a href="#fn-timeout"><code>timeout</code></a>으로 정한다: <code>timeout(1s, () =&gt; recv(rx))</code>.</p>

```fung run
(tx, rx) = channel(1)
send(tx, "hi")
print(recv(rx)!)
```

</div>
<div class="fn" id="fn-close"><code>close(tx)</code>
  <p>채널을 닫는다. 그 끝의 다른 사본이 살아 있어도 지금 닫는다(최상위에서 만든 채널처럼 사본이 오래 살 때 쓴다). 받는 끝은 닫지 않는다(E0403). 파일 핸들은 <code>fs.close</code>로 닫는다(이름이 같지만 다른 함수다).</p>
</div>
<div class="fn" id="fn-stop_signal"><code>stop_signal()</code>
  <p>정지 요청을 받는 끝(<code>receiver</code>). 정지 요청은 "하던 일을 마치고 끝내라"이다. 첫 SIGINT·SIGTERM(터미널의 Ctrl-C, <code>kill</code>, 컨테이너 종료)이나 <a href="#fn-request_stop"><code>request_stop</code></a>이 보낸다. 오면 이 끝이 닫히므로 <code>recv</code>는 <code>Err({kind: 'closed})</code>, 이 끝을 도는 <code>for</code>는 끝난다. 기다리기만 할 때는 <code>_ = recv(stop)</code>로 쓴다(문장으로 쓴 <code>recv(stop)</code>은 그 <code>Err</code>를 버리므로 E0606). 취소("지금 멈춰라")와 다르다. 받은 쪽이 스스로 정리하고 끝낸다.</p>
  <p>이 끝의 사본이 하나라도 살아 있으면 "보는 쪽이 있다". 보는 쪽이 있으면 정지 요청은 이 끝을 닫기만 하고, 프로그램은 진입 파일이 끝날 때 끝난다(종료 코드 0). 보는 쪽이 없으면 정지 요청은 프로그램을 취소한다(<code>with</code>의 해제는 돈다, 종료 코드 130·143). 그래서 정지 요청을 모르는 스크립트의 Ctrl-C는 바로 멈춘다. 두 번째 신호는 해제를 기다리지 않고 곧바로 끝낸다(128 + 신호 번호). 이 끝에서만 기다리는 것은 교착이 아니다(신호가 깨운다). 정지 요청이 이미 왔으면 닫힌 끝을 준다.</p>

```fung run
stop = stop_signal()
request_stop()
print(recv(stop))
```

</div>
<div class="fn" id="fn-request_stop"><code>request_stop()</code>
  <p>코드에서 정지 요청을 보낸다. 첫 신호와 똑같이 동작한다: 보는 쪽이 있으면 <a href="#fn-stop_signal"><code>stop_signal</code></a>의 끝들을 닫고 <code>nil</code>, 없으면 <code>exit(0)</code>처럼 프로그램을 끝낸다. 관리용 요청이나 헬스체크 실패로 스스로 내려가는 서비스, 정상 종료를 시험하는 <code>fung test</code>에 쓴다. <code>fung test</code>는 테스트 블록마다 정지 상태를 새로 둔다. REPL에서도 같다: 보는 쪽이 없으면 <code>exit(0)</code>처럼 세션을 끝낸다.</p>
</div>
<div class="fn" id="fn-atom"><code>atom(v)</code>
  <p>값 <code>v</code>를 가리키는 칸. 태스크 사이에 나누는 바뀌는 상태는 이것으로 둔다. 출력은 <code>&lt;atom&gt;</code>, <code>type_of</code>는 <code>'atom</code>, 타입 표기는 <code>a: atom</code>, <code>==</code>는 같은 칸인지. 안에는 바뀌는 것(atom, 채널 끝, 태스크·범위 핸들, 파일, 소켓, 그리고 그것들을 품은 리스트·맵·튜플·태그·클로저)을 넣지 못한다(E0708). 검사는 값이 커도 빠르다.</p>
</div>
<div class="fn" id="fn-swap"><code>swap(a, f)</code>
  <p>비교 후 교체: <code>f(지금 값)</code>을 계산해 그사이 아무도 바꾸지 않았으면 바꾸고, 바꿨으면 새 값으로 <code>f</code>를 다시 돌린다. 새 값을 돌려준다. 그래서 여러 태스크가 동시에 바꿔도 갱신이 사라지지 않는다. <code>f</code>는 여러 번 돌 수 있으므로 <b>순수 계산만</b> 한다: 출력, 표준 입력, <code>exit</code>, 태스크·채널 연산, <code>sleep</code>, 다른 atom의 <code>swap</code>·<code>reset</code>, 파일·네트워크·외부 프로세스를 부르면 E0707이다(에러는 부른 함수의 이름을 댄다: <code>fs.with_open</code>, <code>http.get</code>, <code>take</code>; 원시 함수를 바로 부르면 그 이름 <code>read</code>). 읽기(<code>deref</code>, 시간, 환경 변수)는 된다. <code>f</code>가 실패하면 상태는 그대로다.</p>

```fung run
hits = atom(0)
each(1..=3, _ => swap(hits, n => n + 1))
print(deref(hits))
```

</div>
<div class="fn" id="fn-reset"><code>reset(a, v)</code>
  <p>지금 값을 <code>v</code>로 바꾸고 <code>v</code>를 돌려준다. 입출력이 필요한 갱신은 갱신 태스크 하나가 <code>reset</code>하고 나머지는 <code>deref</code>하는 모양으로 쓴다.</p>
</div>
<div class="fn" id="fn-deref"><code>deref(a)</code>
  <p>지금 값. 기다리지 않는다.</p>
</div>
<div class="fn" id="fn-sleep"><code>sleep(d)</code>
  <p>기간만큼 기다린다. 취소되면 깨어난다.</p>
</div>
</section>

## 모듈 {#modules}

<p>표준 모듈은 이름으로 가져온다: <code>import fs</code>. 모두 실행 파일에 들어 있다.</p>
<section class="group" id="fs">
<h3>fs — 파일과 디렉터리</h3>
<p><code>import fs</code>. 실패할 수 있는 연산은 <code>Ok</code>/<code>Err</code>를 돌려주고, <code>Err</code>의 <code>kind</code>는 <code>'not_found</code> <code>'exists</code> <code>'permission</code> <code>'io</code> 중 하나다. 파일 내용의 UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD 하나로 바뀐다. <code>fs.remove</code>·<code>fs.close</code>는 prelude의 <code>remove</code>(맵에서 키 빼기)·<code>close</code>(채널 닫기)와 이름만 같은 다른 함수다.</p>
<div class="fn" id="fn-fs-read"><code>fs.read(path)</code>
  <p>파일 전체를 <code>Ok(문자열)</code>로.</p>
</div>
<div class="fn" id="fn-fs-read_lines"><code>fs.read_lines(path)</code><span class="by">fung으로 작성</span>
  <p>파일을 줄 리스트로(<code>lines</code>와 같은 규칙).</p>
</div>
<div class="fn" id="fn-fs-write"><code>fs.write(path, text)</code>
  <p>덮어쓴다. <code>Ok(nil)</code>.</p>
</div>
<div class="fn" id="fn-fs-append"><code>fs.append(path, text)</code>
  <p>끝에 덧붙인다(없으면 만든다).</p>
</div>
<div class="fn" id="fn-fs-list"><code>fs.list(dir, full = false)</code>
  <p>디렉터리 안의 이름들(이름순). <code>full: true</code>면 <code>dir</code>을 붙인 경로.</p>
</div>
<div class="fn" id="fn-fs-glob"><code>fs.glob(pattern)</code>
  <p>패턴에 맞는 경로들(디렉터리마다 이름의 바이트 순서). <code>*</code>(<code>/</code> 밖의 아무 글자들) <code>?</code>(한 글자) <code>[…]</code>(<code>[^…]</code>는 여집합, <code>a-z</code> 범위) <code>\</code>(다음 글자 그대로). <code>!</code>는 보통 글자다. 잘못된 패턴은 파일을 읽기 전에 <code>Err('invalid)</code>, 읽지 못하는 디렉터리는 건너뛴다.</p>
</div>
<div class="fn" id="fn-fs-walk"><code>fs.walk(dir)</code>
  <p>아래의 모든 파일 경로(디렉터리 제외, 재귀).</p>
</div>
<div class="fn" id="fn-fs-exists"><code>fs.exists(path)</code>
  <p><code>true</code>/<code>false</code>(결과값이 아니다).</p>
</div>
<div class="fn" id="fn-fs-stat"><code>fs.stat(path)</code>
  <p><code>Ok({name, size, is_dir})</code>.</p>
</div>
<div class="fn" id="fn-fs-mkdir"><code>fs.mkdir(path)</code>
  <p>중간 디렉터리까지 만든다.</p>
</div>
<div class="fn" id="fn-fs-remove"><code>fs.remove(path)</code>
  <p>파일이나 빈 디렉터리를 지운다.</p>
</div>
<div class="fn" id="fn-fs-remove_all"><code>fs.remove_all(path)</code>
  <p>디렉터리를 통째로 지운다.</p>
</div>
<div class="fn" id="fn-fs-move"><code>fs.move(from, to)</code>
  <p>이름을 바꾸거나 옮긴다.</p>
</div>
<div class="fn" id="fn-fs-copy"><code>fs.copy(from, to)</code>
  <p>파일 하나를 복사한다(권한 유지).</p>
</div>
<div class="fn" id="fn-fs-open"><code>fs.open(path, mode = &#x27;read)</code>
  <p>파일 핸들을 <code>Ok(file)</code>로. <code>mode</code>는 <code>'read</code> <code>'write</code> <code>'append</code>. 핸들은 태스크 사이에 공유해도 연산 단위로 직렬화된다.</p>
</div>
<div class="fn" id="fn-fs-close"><code>fs.close(f)</code>
  <p>닫는다(쓴 내용을 비운다). 닫힌 핸들에 읽고 쓰면 <code>Err('closed)</code>. 닫지 않은 핸들은 마지막 사본이 사라질 때 닫히지만, 그때는 버퍼에 남은 쓰기를 비우지 않는다.</p>
</div>
<div class="fn" id="fn-fs-write_line"><code>fs.write_line(f, line)</code>
  <p>핸들에 한 줄 쓴다.</p>
</div>
<div class="fn" id="fn-fs-with_open"><code>fs.with_open(path, f, mode = &#x27;read)</code><span class="by">fung으로 작성</span>
  <p>열고 <code>f(핸들)</code>을 부른 뒤 결과가 무엇이든 닫고 그 결과를 돌려준다. <code>defer</code> 대신 쓰는 모양이다.</p>

```fung frag
fs.with_open("notes.txt", fn(h):
  line = read_line(h)?
  print(line)
;;)
```

</div>
<div class="fn" id="fn-fs-with_temp_dir"><code>fs.with_temp_dir(f)</code><span class="by">fung으로 작성</span>
  <p>임시 디렉터리를 만들어 <code>f(경로)</code>를 부르고 지운다.</p>
</div>
</section>
<section class="group" id="path">
<h3>path — 경로 계산</h3>
<p><code>import path</code>. 파일 시스템을 건드리지 않는 순수한 경로 연산이다.</p>
<div class="fn" id="fn-path-join"><code>path.join(...parts)</code>
  <p>이어 붙이고 정리한다(<code>..</code>·<code>.</code>·겹친 <code>/</code>). 뒤의 절대 경로가 앞을 지우지 않는다: <code>path.join("a", "/c")</code>는 <code>"a/c"</code>.</p>
</div>
<div class="fn" id="fn-path-base"><code>path.base(p)</code>
  <p>마지막 요소: <code>"a/b.txt"</code> → <code>"b.txt"</code>.</p>
</div>
<div class="fn" id="fn-path-dir"><code>path.dir(p)</code>
  <p>마지막 요소를 뺀 부분.</p>
</div>
<div class="fn" id="fn-path-ext"><code>path.ext(p)</code>
  <p>점을 포함한 확장자: <code>"a.tar.gz"</code> → <code>".gz"</code>. 점으로 시작하는 이름은 확장자가 없다: <code>".hidden"</code> → <code>""</code>.</p>
</div>
<div class="fn" id="fn-path-stem"><code>path.stem(p)</code>
  <p>확장자를 뺀 이름: <code>"a.tar.gz"</code> → <code>"a.tar"</code>.</p>
</div>
<div class="fn" id="fn-path-abs"><code>path.abs(p)</code>
  <p>현재 디렉터리 기준 절대 경로(문자열).</p>
</div>
</section>
<section class="group" id="cli">
<h3>cli — 명령행 옵션</h3>
<p><code>import cli</code>. fung으로 썼다.</p>
<div class="fn" id="fn-cli-parse"><code>cli.parse(argv, bools = [])</code><span class="by">fung으로 작성</span>
  <p><code>{flags, rest}</code>를 돌려준다. <code>--name value</code>, <code>--name=value</code>, 값 없는 <code>--name</code>(<code>true</code>)를 받고, <code>bools</code>에 있는 이름은 값을 먹지 않는다. <code>--</code> 뒤는 전부 <code>rest</code>. 이름의 <code>-</code>는 <code>_</code>가 된다. 값은 문자열이므로 변환은 직접 한다.</p>

```fung run
import cli
opts = cli.parse(["--dry-run", "--n", "3", "a.txt"], bools: ["dry-run"])
print(opts.flags.dry_run, int(opts.flags.n), opts.rest)
```

</div>
</section>
<section class="group" id="json">
<h3>json — JSON</h3>
<p><code>import json</code>. 파서는 성능 때문에 Rust로 썼다.</p>
<div class="fn" id="fn-json-parse"><code>json.parse(s)</code>
  <p><code>Ok(값)</code> 또는 <code>Err({kind: 'parse, line, col, message})</code>. 객체는 키 순서를 지키는 맵, 소수점·지수 없는 숫자는 정수(int64를 넘으면 <code>Err</code>), 그 외 숫자는 실수, <code>null</code>은 <code>nil</code>. 같은 키가 두 번 나오거나 짝 없는 서로게이트(<code>\ud800</code>)가 있거나 중첩이 10,000단계를 넘으면 <code>Err</code>.</p>

```fung run
import json
data = json.parse(`{"name": "fung", "tags": ["a"], "n": 3}`)!
print(data.name, data.tags, data.n + 1)
```

</div>
<div class="fn" id="fn-json-stringify"><code>json.stringify(v, indent = nil)</code>
  <p>JSON 문자열. 튜플은 배열, 심볼은 문자열, 시각은 ISO 8601 문자열이 된다. 기간·함수·태그·채널·atom 등은 런타임 에러(E0513). <code>indent</code>는 들여쓰기 칸 수(음수는 E0403). 문자열은 <code>"</code>·<code>\</code>·U+0020 미만만 이스케이프한다. 결과가 1억 글자를 넘으면 E0806.</p>

```fung run
import json
print(json.stringify({ok: true, ids: (1, 2), at: 'now}))
```

</div>
</section>
<section class="group" id="time">
<h3>time — 시각</h3>
<p><code>import time</code>. 시각은 언어의 값 타입이라 <code>==</code>, <code>&lt;</code>, <code>t + 1h</code>, <code>t2 - t1</code>(기간)이 바로 된다.</p>
<div class="fn" id="fn-time-now"><code>time.now()</code>
  <p>현재 시각(로컬 시간대). 단조 시계를 싣지 않으므로, 두 <code>now()</code> 사이에 시스템 시각이 바뀌면(NTP 보정 등) 그 차이로 잰 경과 시간이 틀릴 수 있다.</p>
</div>
<div class="fn" id="fn-time-parse"><code>time.parse(s, format = nil)</code>
  <p><code>Ok(time)</code> 또는 <code>Err('parse)</code>. 형식에 시간대 토큰이 없으면 로컬 시간으로 읽고, 서머타임 경계의 없는 시각은 빈틈만큼 뒤로(<code>02:30</code> → <code>03:30</code>), 두 번 있는 시각은 이른 순간이다. 형식이 없으면 RFC 3339를 엄격하게 읽는다: <code>2026-10-04T09:05:07Z</code>, <code>2026-10-04T09:05:07.5+09:00</code>(필드는 두 자리, 소수는 <code>.</code>와 1~9자리, 공백 구분 없음). 숫자 오프셋은 쓴 그대로 남는다. 범위를 넘는 값(13월, 2월 30일)은 <code>Err</code>.</p>

```fung run
import time
t = time.parse("2026-10-04 09:05", "YYYY-MM-DD HH:mm")!
print(time.format(t + 90m, "HH:mm"))
```

</div>
<div class="fn" id="fn-time-format"><code>time.format(t, format = nil)</code>
  <p>문자열로. 형식이 없으면 ISO 8601.</p>
</div>
<div class="fn" id="fn-time-utc"><code>time.utc(t)</code>
  <p>같은 순간을 UTC로.</p>
</div>
<div class="fn" id="fn-time-local"><code>time.local(t)</code>
  <p>같은 순간을 로컬 시간대로.</p>
</div>
<div class="fn" id="fn-time-unix"><code>time.unix(t)</code>
  <p>유닉스 초(정수).</p>
</div>
<div class="fn" id="fn-time-from_unix"><code>time.from_unix(n)</code>
  <p>유닉스 초에서 시각으로(로컬 시간대).</p>
</div>
<div class="fn" id="fn-time-parts"><code>time.parts(t)</code>
  <p><code>{year, month, day, hour, minute, second, weekday}</code>. <code>weekday</code>는 <code>'mon</code>…<code>'sun</code>.</p>
</div>
</section>
<section class="group" id="regex">
<h3>regex — 정규식</h3>
<p><code>import regex</code>. 문법은 Rust <code>regex</code> 크레이트의 것(RE2 계열)이다: 역참조와 전후방 탐색이 없는 대신 항상 선형 시간에 돈다. <code>\d</code>·<code>\w</code>·<code>\s</code>·<code>\b</code>는 유니코드를 알아본다(ASCII만 원하면 <code>(?-u:\d)</code>). 위치는 글자 단위 인덱스이고, 잘못된 패턴은 런타임 에러(E0515)다. 컴파일한 패턴은 캐시된다.</p>
<div class="fn" id="fn-regex-is_match"><code>regex.is_match(s, pat)</code>
  <p>맞는 곳이 있으면 <code>true</code>. (<code>match</code>는 예약어라 이 이름이다.)</p>
</div>
<div class="fn" id="fn-regex-find"><code>regex.find(s, pat)</code>
  <p>첫 매치 <code>{text, start, end, groups, named}</code> 또는 <code>nil</code>. <code>groups</code>는 캡처 그룹 리스트(안 잡힌 그룹은 <code>nil</code>), <code>named</code>는 이름 있는 그룹 맵.</p>

```fung run
import regex
m = regex.find("port=8080", `(?P<key>\w+)=(?P<val>\d+)`)
print(m.named.key, int(m.named.val) + 1)
```

</div>
<div class="fn" id="fn-regex-find_all"><code>regex.find_all(s, pat)</code>
  <p>겹치지 않는 모든 매치의 리스트.</p>
</div>
<div class="fn" id="fn-regex-replace"><code>regex.replace(s, pat, repl)</code>
  <p>모든 매치를 바꾼다. <code>repl</code>에서 <code>$1</code>, <code>${name}</code>으로 그룹을 쓰고 <code>$$</code>는 <code>$</code>다. <code>$1a</code>는 이름이 <code>1a</code>인 그룹이므로 뒤에 글자를 붙이려면 <code>${1}a</code>로 쓴다.</p>

```fung run
import regex
print(regex.replace("2026-10-04", `(\d+)-(\d+)-(\d+)`, "$3/$2/$1"))
```

</div>
<div class="fn" id="fn-regex-split"><code>regex.split(s, pat)</code>
  <p>매치로 나눈다. 빈 조각도 남긴다. 문자열 맨 앞과 맨 끝의 빈 매치로는 자르지 않으므로 <code>regex.split("abc", "")</code>는 <code>["a", "b", "c"]</code>다.</p>
</div>
</section>
<section class="group" id="http">
<h3>http — HTTP</h3>
<p><code>import http</code>. 소켓·TLS 원시 기능 위에 HTTP/1.1을 fung으로 썼다. 응답을 받았으면 상태 코드와 무관하게 <code>Ok</code>다(404도 <code>Ok</code>). 전송 실패는 <code>Err('network)</code>, 시간 초과는 <code>Err('timeout)</code>. 에러 문구는 <code>cannot request `URL`: 이유</code>. 응답 헤더 키는 소문자이고, 같은 이름이 여럿이면 <code>", "</code>로 잇는다. 취소되면 E0701.</p>
<div class="fn" id="fn-http-get"><code>http.get(url, headers = {}, timeout = 30s)</code><span class="by">fung으로 작성</span>
  <p><code>Ok({status, headers, body})</code>. <code>timeout</code>은 연결·TLS 핸드셰이크·리다이렉트까지 포함한 요청 전체의 한도 하나이고, <code>0s</code>면 제한 없음. 바깥에 더 짧은 <a href="#fn-timeout"><code>timeout</code></a>이 있으면 그쪽이 이긴다.</p>

```fung frag
import http
r = http.get("https://example.com")!
print(r.status, r.headers["content-type"])
```

</div>
<div class="fn" id="fn-http-post"><code>http.post(url, body, headers = {}, timeout = 30s)</code><span class="by">fung으로 작성</span>
  <p>본문은 문자열(JSON은 <code>json.stringify</code>로).</p>
</div>
<div class="fn" id="fn-http-request"><code>http.request(method, url, body = nil, headers = {}, timeout = 30s)</code><span class="by">fung으로 작성</span>
  <p>임의의 메서드.</p>
</div>
<div class="fn" id="fn-http-serve"><code>http.serve(addr, handler, ready = nil, drain = 10s, reuse_port = false)</code><span class="by">fung으로 작성</span>
  <p><code>addr</code>(<code>":8080"</code>)에서 요청을 받으며 막힌다. 요청마다 새 태스크에서 <code>handler(req)</code>를 부른다(동시에 돌 수 있으니 공유 상태는 <code>atom</code>에). <code>req</code>는 <code>{method, path, query, headers, body}</code>(<code>query</code>는 이름마다 첫 값, <code>headers</code>는 소문자 키), 응답은 문자열(200, <code>text/plain; charset=utf-8</code>) 또는 <code>{status = 200, headers = {}, body = ""}</code>. 응답에 <code>Date</code>와 <code>Content-Length</code>가 붙는다. 핸들러가 에러를 내면 500으로 답하고 표준 에러에 <code>http: handler failed: …</code>(다른 타입을 돌려주면 <code>http: bad handler response: …</code>) 한 줄을 쓴 뒤 서버는 계속 돈다. <b>정상 종료</b>: 정지 요청(<a href="#fn-stop_signal"><code>stop_signal</code></a>: 첫 SIGINT·SIGTERM이나 <code>request_stop()</code>)이 오면 새 연결을 그만 받고(리스너를 닫음), 처리 중인 요청을 최대 <code>drain</code>까지 마친 뒤, 남은 것은 취소하고(503으로 답함) <code>nil</code>을 돌려준다. 서버가 취소되면(바깥 범위, <code>timeout</code>, <code>exit</code>) 기다리지 않고 곧바로 멈춘다. 처리기 안의 <code>exit</code>도 503. <code>reuse_port</code>는 <code>SO_REUSEPORT</code>를 켜서 프로세스 여럿이 한 포트를 받게 한다(Linux는 연결을 나눠 주고, macOS는 받게만 한다). <code>ready</code>는 리스닝 직후 실제 주소로 한 번 불린다(<code>":0"</code>의 포트를 아는 방법). 헤더는 10초 안에 와야 하고 요청 머리는 1 MiB까지(넘으면 431), 본문은 16 MiB까지(넘으면 413), 형식 오류는 400. 서버가 스스로 내는 답의 본문은 <code>&lt;코드&gt; &lt;이유 문구&gt;</code>이고, 표준 이유 문구가 없는 코드는 빈 이유 문구로 보낸다.</p>

```fung frag
import http
http.serve(":8080", req => match req.path:
  "/health" => "ok"
  _         => {status: 404, body: "not found"}
;;)
```

</div>
<h4>약속하는 것과 하지 않는 것</h4>
<ul>
  <li><b>클라이언트</b>: 요청에 <code>Host</code>, <code>User-Agent: fung/&lt;버전&gt;</code>, 본문이 있으면 <code>Content-Length</code>. 사용자 헤더는 맵 순서대로 나가고, 같은 이름이면 사용자 것이 이긴다(빈 <code>User-Agent</code>는 보내지 않음). 사용자 <code>Host</code>도 자동 값을 대신하고, 리다이렉트가 다른 호스트로 가면 버린다. <code>Content-Length</code>·<code>Transfer-Encoding</code>·<code>Trailer</code>는 클라이언트가 쓴다. URL의 <code>user:pass@</code>는 <code>Authorization: Basic</code>이 된다. 응답 본문은 <code>Content-Length</code>, 조각 전송, 연결 닫힘 중 하나로 읽는다.</li>
  <li><b>리다이렉트</b>: 301·302·303은 <code>GET</code>으로(본문 버림), 307·308은 메서드와 본문을 지킨다. 10번까지 따라가고 11번째는 <code>Err('network)</code>(<code>too many redirects (limit 10)</code>). 리다이렉트 응답의 본문은 읽지 않는다. 다른 호스트로 넘어가면 <code>Authorization</code>·<code>Cookie</code> 같은 민감한 헤더를 버린다.</li>
  <li><b>프록시</b>: <code>HTTP_PROXY</code>·<code>HTTPS_PROXY</code>·<code>NO_PROXY</code>(소문자도). <code>localhost</code>와 루프백 주소는 거치지 않는다. SOCKS 프록시와 https 프록시 너머의 https는 지원하지 않는다(<code>Err('network)</code>).</li>
  <li><b>https</b>: 운영체제 루트 인증서(<code>SSL_CERT_FILE</code>·<code>SSL_CERT_DIR</code>이 있으면 그것)와 SNI. 인증서 오류는 <code>Err('network)</code>(<code>unknown certificate authority</code>, <code>certificate is not valid for `host`</code>, <code>certificate has expired (valid until …)</code>).</li>
  <li><b>응답 본문 상한 64 MiB</b>. 넘으면 <code>Err('network)</code>.</li>
  <li>본문은 문자열이다. UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD 하나로 바뀌므로 바이너리 본문은 그대로 받을 수 없다.</li>
  <li>없는 것: 연결 재사용(keep-alive)과 파이프라이닝, HTTP/2, 압축. 서버는 연결마다 요청 하나만 받으므로 HTTP/1.1 응답에 늘 <code>Connection: close</code>가 붙는다.</li>
</ul>
</section>
<section class="group" id="conc">
<h3>conc — 동시성 도우미</h3>
<p><code>import conc</code>. prelude의 동시성 함수 위에 fung으로 쓴 조합이다. prelude에는 확실한 것만 두므로 모듈로 뺐다.</p>
<div class="fn" id="fn-conc-pmap"><code>conc.pmap(xs, f, limit = 0)</code><span class="by">fung으로 작성</span>
  <p>병렬 <code>map</code>. 자기 범위 안에서 돈다. <code>limit</code>은 동시에 도는 태스크 수(0은 제한 없음). 결과 순서는 입력 순서. <code>f</code>의 런타임 에러는 <code>pmap</code>의 실패가 되고, <code>f</code>가 돌려준 <code>Err</code>는 결과에 그대로 들어간다.</p>

```fung run
import conc
print(conc.pmap([1, 2, 3], x => x * x, limit: 2))
```

</div>
<div class="fn" id="fn-conc-fan_in"><code>conc.fan_in(s, rxs)</code><span class="by">fung으로 작성</span>
  <p>여러 받는 끝을 하나로 모은 받는 끝. 전달 태스크는 범위 <code>s</code> 안에서 돈다. 모두 닫히면 닫힌다. 돌려준 받는 끝이 사라지면 전달을 멈추고 입력을 놓으므로, 생산자의 <code>send</code>도 <code>Err</code>를 받는다.</p>
</div>
<div class="fn" id="fn-conc-retry"><code>conc.retry(f, times = 3, delay = 0ms)</code><span class="by">fung으로 작성</span>
  <p><code>f()</code>가 <code>Err</code>가 아닌 값을 돌려줄 때까지 최대 <code>times</code>번, 사이에 <code>delay</code>만큼 쉬며 부른다. 모두 실패하면 마지막 <code>Err</code>.</p>

```fung run
import conc
print(conc.retry(() => Err("down"), times: 2))
```

</div>
</section>
