---
description: 매개변수와 반환의 타입 표기와 그 검사.
lede: 매개변수와 반환의 타입 표기와 그 검사.
---
# 타입 표기

```fung run
type shape = Circle | Rect  # 별칭: 같은 모듈 안에서만

fn area(s: shape) -> num = match s:
  Circle(r)  => 3 * r * r
  Rect(w, h) => w * h
;;
fn find_user(id: int) -> map | nil = id == 1 then {name: "ann"} else nil
fn load(p: str) -> Ok | Err = p == "" then Err("empty path") else Ok(p)

print(area(Circle(2)), area(Rect(2, 3)))
print(find_user(1), find_user(2))
print(load("a.txt"), load(""))
```

<ul>
  <li>타입 이름: <code>int</code> <code>float</code> <code>num</code>(int 또는 float) <code>str</code> <code>bool</code> <code>nil</code> <code>sym</code> <code>dur</code> <code>time</code> <code>list</code> <code>map</code> <code>tuple</code> <code>range</code> <code>fn</code> <code>module</code> <code>task</code> <code>scope</code> <code>sender</code> <code>receiver</code> <code>atom</code> <code>file</code> <code>any</code>, 그리고 태그 이름. 태그 이름은 인자 없는 태그(심볼)도 받으므로 열거형을 쓸 수 있다: <code>type color = Red | Green | Blue</code>, <code>fn hex(c: color)</code>. 그래도 <code>type_of(Red)</code>는 <code>'sym</code>이다. 결과값의 이름 <code>Ok</code>·<code>Err</code>만은 값 하나를 담은 태그만 받는다(<code>-&gt; Ok | Err</code>에 <code>sym("Ok")</code>를 돌려주면 E0404).</li>
  <li><code>|</code>로 유니온을 만든다. 원소 타입은 적지 않는다: 검사하지 않는 표기는 두지 않으므로 <code>list[int]</code>는 컴파일 에러 E0130이고 <code>list</code>라고 쓴다.</li>
  <li>인자와 반환 타입에만 쓴다. 맞지 않으면 런타임 에러(E0403, E0404). 변수에는 쓰지 않는다. <code>...rest</code>는 늘 리스트라 <code>list</code>만 적을 수 있다(다른 표기는 E0228).</li>
  <li>타입 이름은 값 이름과 다른 이름 공간이다. <code>list = [1]</code>이라는 변수가 있어도 <code>x: list</code>는 타입이다.</li>
  <li>별칭 이름은 소문자이고(대문자는 태그), 위의 타입 이름은 별칭으로 쓸 수 없다(<code>type int = str</code>은 E0222).</li>
  <li>내장 타입 이름은 소문자다. 대소문자만 다른 <code>Int</code>·<code>String</code>·<code>List</code>·<code>Bool</code>·<code>Nil</code>은 태그 이름으로 읽혀 어떤 값도 맞지 않으므로 컴파일 에러 E0212다(도움말이 <code>int</code>를 알려 준다).</li>
</ul>

## 경계 사례와 에러 {#edges}

표기한 타입이 아니면 호출 자리에서 멈춘다.

```fung run exit=1 error=E0403
fn twice(n: int) -> int = n * 2
print(twice(4))
print(twice("4"))
```

원소 타입은 적지 않는다. 런타임이 바깥 타입만 검사하므로 적어도 지켜지지 않기 때문이다.

```fung run exit=2 error=E0130
fn total(xs: list) -> int = sum(xs)
print(total([1, 2]))
fn names(xs: list[str]) = xs
```

반환 타입이 맞지 않으면 반환하는 자리에서 멈춘다.

```fung run exit=1 error=E0404
fn half(n: int) -> int = n / 2
print(half(4))
```
