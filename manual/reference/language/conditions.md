---
description: if, then … else, and·or, 값 자리의 조건.
lede: <code>if</code>, <code>then … else</code>, <code>and</code>·<code>or</code>, 값 자리의 조건.
---
# 조건

```fung run
fn sign(n):
  if n > 0:
    "+"
  else if n < 0:
    "-"
  else:
    "0"
  ;;
;;
print(sign(5), 3 > 2 then "yes" else "no")
```

<ul>
  <li><code>if</code>는 식이다. <code>else</code>가 없고 조건이 거짓이면 값은 <code>nil</code>. 그래서 값을 쓰는 자리(대입의 오른쪽, 인자, <code>return</code>, 연산자, <code>fn f() = …</code>와 람다 <code>=&gt;</code>의 본문 등)의 <code>if</code>는 <code>else</code>가 있어야 한다(없으면 컴파일 에러 E0223, <code>nil</code>이 의도라면 <code>else: nil</code>). 같은 자리의 블록이 비었거나 대입 같은 문장으로 끝나도 E0224다. 함수 본문의 마지막 항목도 값을 쓰는 자리다(반환값이므로, 블록 본문 함수도). 돌려줄 것이 없는 함수는 <code>else: nil</code>이나 마지막 줄 <code>nil</code>로 맺는다. 값이 버려지는 자리, 곧 문장으로 쓴 <code>if</code>, 문장 <code>for</code>의 본문, <code>test</code> 블록의 끝(받는 쪽이 없다)은 <code>else</code> 없이 써도 된다. 문장 <code>for</code>는 리스트를 만들지 않으므로(<a href="for.html">for</a>) 본문의 값이 쓰이지 않는다. 같은 순회를 <code>each(xs, x =&gt; if c: print(x) ;;)</code>로 쓰면 E0223이다: 람다는 값을 돌려주는 함수이고, 그 값을 <code>each</code>가 버린다는 것은 컴파일러가 알 수 없다. 부수효과만 하는 순회는 문장 <code>for</code>로 쓴다.</li>
  <li><code>c then a else b</code>는 한 줄짜리 조건 식이다. <code>else</code>가 반드시 있어야 하고, 사슬로 이을 수 있다.</li>
  <li>조건은 bool이어야 한다. 다른 값이면 런타임 에러(E0505). <strong>피연산자의 타입만으로 정해지는 에러는, 피연산자가 그 자리에 쓴 리터럴이면 컴파일할 때 같은 번호와 문구로 멈춘다.</strong> 조건(<code>if 1:</code>·<code>not 1</code>·<code>1 and true</code>·<code>assert 1</code>, E0505), 부를 수 없는 값(<code>1(2)</code>, E0401), 두 값에 뜻이 없는 연산(<code>1 + "a"</code>·<code>1.0..3</code>·<code>1 in "abc"</code>, E0501), 수가 아닌 값의 <code>-</code>(E0506), 담을 수 없는 값의 <code>in</code>(<code>"a" in 5</code>, E0512), 인덱스·필드·슬라이스(<code>(1, 2)[0]</code>·<code>"a".b</code>는 E0509, <code>5[1..]</code>는 E0511, 문자열이 아닌 맵 키는 E0514), 결과가 아닌 값의 <code>!</code>·<code>?</code>(E0605, <code>nil</code>이면 E0604), 펼칠 수 없는 값(<code>f(...1)</code>, E0506), 결코 <code>nil</code>이 아닌 값 뒤의 <code>??</code>(E0243)가 모두 그렇다. 연산을 가리지 않는 한 규칙이다. 여기서 리터럴은 문법이 타입을 보이는 모든 값이다: 수·문자열(<code>${…}</code>가 든 것도)·불·<code>nil</code>·심볼·기간, 리스트·맵·튜플 리터럴, 태그 값(<code>Red(1)</code>), 범위(<code>1..2</code>), 람다·익명 <code>fn</code>·부분 적용(타입 <code>fn</code>). 그 자리에 직접 쓴 리터럴만 본다. 이름은 따라가지 않는다: <code>c = 1</code> 뒤의 <code>if c:</code>는 그 줄이 실행될 때 E0505다(끄는 값이 <code>nil</code>인 설정 <code>log_path = nil</code>을 <code>if log_path != nil:</code>로 확인하고 쓰는 코드가 컴파일되어야 하기 때문이다). 호출 결과나 계산한 값, 매개변수도 실행할 때 본다. 에러의 문구와 도움말도 실행했을 때와 같다. 두 리스트나 두 튜플의 순서 비교처럼 원소가 정하는 것은 실행할 때 본다. 값이 정하는 에러(<code>1 // 0</code>, 정수 넘침)는 실행할 때 난다.</li>
</ul>

## 경계 사례와 에러 {#edges}

`else` 없는 `if`를 값으로 쓰면, 조건이 거짓일 때 `nil`이 되므로 에러다.

```fung run exit=2 error=E0223
x = 5
label = if x > 3: "big" else: "small" ;;
print(label)
bad = if x > 3: "big" ;;
```

조건은 `bool`이어야 한다. `0`이나 `""`를 거짓으로 보지 않는다.

```fung run exit=1 error=E0505
n = len([])
print(n == 0 then "zero" else "some")
print(n then "yes" else "no")
```
