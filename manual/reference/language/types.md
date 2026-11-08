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
  <li>타입 이름: <code>int</code> <code>float</code> <code>num</code>(int 또는 float) <code>str</code> <code>bool</code> <code>nil</code> <code>sym</code> <code>dur</code> <code>time</code> <code>list</code> <code>map</code> <code>tuple</code> <code>range</code> <code>fn</code> <code>module</code> <code>task</code> <code>scope</code> <code>sender</code> <code>receiver</code> <code>atom</code> <code>file</code> <code>any</code>, 그리고 태그 이름. 태그 이름은 인자 없는 태그(심볼)도 받으므로 열거형을 쓸 수 있다: <code>type color = Red | Green | Blue</code>, <code>fn hex(c: color)</code>. 그래도 <code>type_of(Red)</code>는 <code>'sym</code>이다.</li>
  <li><code>|</code>로 유니온을 만든다. <code>list[int]</code>처럼 원소 타입을 적을 수 있지만 런타임은 바깥 타입만 검사한다. 안쪽 이름은 컴파일 때 있는 타입인지만 본다(E0212). 원소 표기는 <code>list</code>·<code>map</code>에만 붙는다(E0232).</li>
  <li>인자와 반환 타입에만 쓴다. 맞지 않으면 런타임 에러(E0403, E0404). 변수에는 쓰지 않는다. <code>...rest</code>는 늘 리스트라 <code>list</code>만 적을 수 있다(다른 표기는 E0228).</li>
  <li>타입 이름은 값 이름과 다른 이름 공간이다. <code>list = [1]</code>이라는 변수가 있어도 <code>x: list</code>는 타입이다.</li>
  <li>별칭 이름은 소문자이고(대문자는 태그), 위의 타입 이름은 별칭으로 쓸 수 없다(<code>type int = str</code>은 E0222).</li>
</ul>

## 경계 사례와 에러 {#edges}

표기한 타입이 아니면 호출 자리에서 멈춘다.

```fung run exit=1 error=E0403
fn twice(n: int) -> int = n * 2
print(twice(4))
print(twice("4"))
```

반환 타입이 맞지 않으면 반환하는 자리에서 멈춘다.

```fung run exit=1 error=E0404
fn half(n: int) -> int = n / 2
print(half(4))
```
