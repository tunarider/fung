---
description: 값의 종류와 그 성질, 문자열·리스트·맵·튜플·태그·범위.
lede: 값의 종류와 그 성질, 문자열·리스트·맵·튜플·태그·범위.
---
# 값과 타입

<table>
  <thead><tr><th>타입</th><th>예</th><th>설명</th></tr></thead>
  <tbody>
    <tr><td><code>int</code></td><td><code>42</code></td><td>64비트 정수. 넘치면 에러.</td></tr>
    <tr><td><code>float</code></td><td><code>3.14</code></td><td>64비트 실수. <code>NaN</code>·<code>Inf</code>가 되면 에러.</td></tr>
    <tr><td><code>str</code></td><td><code>"a"</code></td><td>불변 바이트 열, UTF-8로 읽는 글자 단위 인덱스.</td></tr>
    <tr><td><code>bool</code></td><td><code>true</code></td><td>조건 자리에는 bool만 온다.</td></tr>
    <tr><td><code>nil</code></td><td><code>nil</code></td><td>"없음"이라는 정상 값. <code>==</code>, <code>??</code>, 출력, 컬렉션에 담기 외의 연산(<code>!</code>·<code>?</code> 포함)은 에러.</td></tr>
    <tr><td><code>sym</code></td><td><code>'ok</code></td><td>이름만 있는 값.</td></tr>
    <tr><td><code>dur</code></td><td><code>1h30m</code></td><td>기간. 기간끼리 <code>+ -</code>와 <code>/ // %</code>, 숫자와 <code>* /</code>, 순서 비교.</td></tr>
    <tr><td><code>time</code></td><td><code>time.now()</code></td><td>시각. 리터럴 없음. <code>± dur</code>, <code>time - time</code>은 기간, 순서 비교. 출력은 ISO 8601.</td></tr>
    <tr><td><code>list</code></td><td><code>[1, 2]</code></td><td>순서 있는 목록.</td></tr>
    <tr><td><code>map</code></td><td><code>{a: 1}</code></td><td>키는 항상 문자열. 넣은 순서를 지킨다.</td></tr>
    <tr><td><code>tuple</code></td><td><code>(1, "a")</code></td><td>고정 개수. 원소는 패턴으로만 꺼낸다. 하나짜리는 <code>(x,)</code>.</td></tr>
    <tr><td>태그</td><td><code>Ok(1)</code> <code>Circle(3)</code></td><td>이름 + 인자. 타입 이름은 태그 이름 자체.</td></tr>
    <tr><td><code>range</code></td><td><code>0..10</code> <code>0..=10</code></td><td>게으른 정수 범위. 리스트를 미리 만들지 않는다. 함수의 입력(<code>map</code>·<code>len</code>·<code>flatten</code>·<code>sum</code> 등)으로만 리스트처럼 쓰이고, 연산자 자리(<code>+</code>, 인덱스, <code>==</code>로 리스트와 비교)에서는 리스트가 아니다.</td></tr>
    <tr><td><code>fn</code></td><td><code>x =&gt; x</code></td><td>함수. 내장 함수도 같은 타입.</td></tr>
    <tr><td><code>module</code></td><td><code>fs</code></td><td>모듈의 공개 이름들로 된 불변 레코드.</td></tr>
    <tr><td><code>file</code></td><td><code>fs.open(p)!</code></td><td>열린 파일 핸들. 상태가 있는 자원.</td></tr>
    <tr><td><code>task</code> <code>scope</code> <code>sender</code> <code>receiver</code> <code>atom</code></td><td><code>scope(s =&gt; spawn(s, f))</code>, <code>(tx, rx) = channel()</code></td><td>동시성 값(<a href="../prelude/concurrency.html">내장 함수</a>). 채널은 보내는 끝과 받는 끝 두 값으로 다룬다. atom은 값을 보이지 않고 <code>&lt;atom&gt;</code>으로 찍힌다(<code>deref</code>로 읽는다).</td></tr>
  </tbody>
</table>
<p><code>type_of(x)</code>가 위 타입 이름을 심볼로 돌려준다: <code>type_of(1) == 'int</code>, <code>type_of(Circle(3)) == Circle</code>, <code>type_of('red) == 'sym</code>.</p>
<h3>실수</h3>
<ul>
  <li>실수는 늘 소수점이나 지수가 붙어 찍혀 정수와 구별된다(<code>1.0</code>). 절댓값이 10<sup>21</sup> 이상이거나 10<sup>-4</sup>보다 작으면 지수로 쓴다(<code>1e21</code>, <code>9e-5</code>), 그 사이는 소수다(<code>100000000000000000000.0</code>, <code>0.0001</code>). 자릿수는 같은 실수로 되읽히는 가장 짧은 것이다(<code>0.1 + 0.2</code>는 <code>0.30000000000000004</code>).</li>
  <li><code>-0.0</code>은 <code>-0.0</code>으로 찍히지만 <code>0.0 == -0.0</code>이고, 같은 값이므로 <code>unique([0.0, -0.0])</code>은 <code>[0.0]</code>이다.</li>
  <li><code>//</code>·<code>%</code>는 실수에도 쓴다. 피연산자 하나가 실수면 결과도 실수다: <code>7.5 // 2 == 3.0</code>, <code>7 % 2.0 == 1.0</code>.</li>
  <li>숫자는 int와 float를 넘어 값으로 같으므로, <code>match</code>의 리터럴 패턴 <code>1.0</code>은 <code>1</code>에도 맞고, <code>2.0 in 0..3</code>은 <code>true</code>다(<code>1.5 in 0..3</code>은 <code>false</code>).</li>
</ul>
<h3>맵</h3>
<ul>
  <li><code>{name: "a"}</code>는 <code>{"name": "a"}</code>의 축약이다. 식별자 모양이면 대소문자와 상관없이 축약된다(<code>{DB_HOST: h}</code>).</li>
  <li>값이 같은 이름의 변수면 키만 쓴다: <code>{name, port}</code>는 <code>{name: name, port: port}</code>. 대문자 키는 그런 변수가 없으므로 축약할 수 없다(<code>{HOME}</code>은 E0225).</li>
  <li>같은 키를 두 번 쓰면 컴파일 에러. 없는 키를 읽으면 런타임 에러이고, 기본값이 필요하면 <code>get(m, k, d)</code>를 쓴다.</li>
  <li>수정된 사본은 함수로 만든다: <code>put</code>, <code>merge</code>, <code>remove</code>.</li>
</ul>
<h3>비교와 동일성</h3>
<table>
  <thead><tr><th>타입</th><th><code>==</code></th><th><code>&lt;</code> 등</th></tr></thead>
  <tbody>
    <tr><td>int, float</td><td>값. 섞여도 숫자로 비교(<code>1 == 1.0</code>), 2<sup>53</sup>을 넘는 정수도 정확히</td><td>값</td></tr>
    <tr><td>str</td><td>내용(바이트)</td><td>바이트 순(UTF-8이면 코드 포인트 순과 같다)</td></tr>
    <tr><td>dur, time</td><td>값(time은 같은 순간이면 시간대 무관)</td><td>값</td></tr>
    <tr><td>list, tuple</td><td>원소가 차례로 같음</td><td>앞 원소부터 사전식</td></tr>
    <tr><td>map</td><td>키 집합과 값이 같음(순서 무시)</td><td>에러</td></tr>
    <tr><td>bool, nil, sym, 태그</td><td>값</td><td>에러</td></tr>
    <tr><td>range</td><td>같은 정수 열(<code>0..5 == 0..=4</code>, 빈 범위끼리는 같음). 출력은 쓴 모양 그대로</td><td>에러</td></tr>
    <tr><td>fn</td><td>에러 E0501(같음이 없다)</td><td>에러</td></tr>
    <tr><td>module, file, task, scope, sender, receiver, atom</td><td>같은 객체인가</td><td>에러</td></tr>
    <tr><td>타입이 다를 때</td><td>에러 E0517. 예외는 숫자끼리(int와 float)와 한쪽이 <code>nil</code>(<code>false</code>)</td><td>에러</td></tr>
  </tbody>
</table>
<p><code>sort</code>, <code>unique</code>, <code>contains</code>, <code>in</code>은 모두 이 표를 따른다(<code>unique</code>는 원소마다 <code>nil</code>이 아닌 첫 원소와 타입을 견준다). 함수는 같음이 없으므로 해시도 없어 <code>unique</code>에 넣으면 E0501이다.</p>
<p><strong>타입이 다른 두 값의 비교는 에러다(E0517).</strong> 다른 타입의 값은 결코 같지 않으므로 그런 비교는 거의 늘 실수다. JSON·파일·명령 출력·환경 변수에서 온 값은 늘 문자열이라 <code>back.kind == 'not_found</code>나 <code>env("PORT") == 8080</code>이 조용히 거짓이 되는 것이 가장 흔한 함정이다. 타입은 <code>type_of</code>가 주는 이름이되, 태그 값은 이름이 달라도 모두 한 타입으로 견준다: 이름은 값의 일부라 <code>Ok(1) == Err(1)</code>은 <code>false</code>다(태그와 다른 타입의 값, 예컨대 <code>Ok(1) == 1</code>은 에러). 예외는 둘이다: 숫자끼리는 수로 견주고(<code>1 == 1.0</code>), <code>nil</code>은 무엇과도 견줄 수 있다(<code>x == nil</code>). 리스트·튜플·맵·태그 안에서 맞붙어도, <code>in</code>도 같다. 다만 <code>==</code>가 실제로 견주는 쌍만 본다. 첫 차이에서 멈추므로 <code>[1, 'a] == [2, "a"]</code>는 <code>false</code>다. 한쪽을 바꿔서(<code>int(s)</code>, <code>str(n)</code>, <code>sym(s)</code>) 견준다. 타입을 가리려면 <code>type_of</code>나 패턴을 쓴다: 리터럴 패턴(<code>'not_found =&gt;</code>, <code>1 =&gt;</code>)은 비교가 아니라 타입까지 보는 시험이라서 타입이 다르면 그대로 맞지 않을 뿐이다.</p>

## 경계 사례와 에러 {#edges}

맵의 키는 문자열이다. 다른 값을 키로 쓰면 어느 자리에서든(`m[k]`, `k in m`, `get`·`put`·`remove`, `group_by`의 키 함수) 바꾸라는 힌트와 함께 E0514로 멈춘다. 심볼 키(`m['a]`)면 힌트가 문자열로 쓰는 법을 보인다.

```fung run exit=1 error=E0514
print(group_by(["ab", "c"], s => "${len(s)}"))
print(group_by(["ab", "c"], s => len(s)))
```

타입이 다른 두 값은 견줄 수 없다. 심볼과 문자열, 바깥에서 온 글과 수가 흔한 경우다.

```fung run exit=1 error=E0517
print('ok == 'ok, 1 == 1.0, nil == 'ok)
print("8080" == 8080)
```

함수는 같음이 없다.

```fung run exit=1 error=E0501
f = x => x
print(f == f)
```

정수는 64비트이고 넘치면 에러다(조용히 감싸지 않는다).

```fung run exit=1 error=E0503
print(9223372036854775807)
print(9223372036854775807 + 1)
```
