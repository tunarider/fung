---
description: import, 모듈 찾기, 공개 이름.
lede: <code>import</code>, 모듈 찾기, 공개 이름.
---
# 모듈

```fung frag
import fs                              # 표준 라이브러리: 이름
import "./lib/rename"                  # 내 파일: import하는 파일 기준 상대 경로, .fung 생략
import "./lib/rename" as rn            # 별칭
import "./lib/rename" (slug, to_kebab) # 이름만 직접 가져오기
```

<ul>
  <li>파일 하나가 모듈 하나다. 모듈 이름은 파일명이고, 변수 이름이 될 수 없으면(<code>my-lib.fung</code>, 예약어 <code>if.fung</code>, 대문자로 시작하는 <code>Util.fung</code>) <code>as</code>가 필요하다(E0305).</li>
  <li><code>_</code>로 시작하는 최상위 이름은 비공개. 나머지는 공개다. 가져온 이름은 다시 내보내지 않는다.</li>
  <li>import된 파일의 최상위에는 <code>import</code>, <code>fn</code>, <code>type</code>, <code>test</code>, <code>이름 = 식</code>만 올 수 있다. 처음 import될 때 한 번 실행되고 캐시된다. 같은 파일을 심볼릭 링크나 대소문자만 다른 경로로 가져와도 같은 모듈 하나다(파일 자체로 가린다).</li>
  <li>순환 import는 컴파일 에러이고 순환 경로 전체를 보여 준다. 진입 파일을 import할 수도 없다.</li>
  <li>무엇이 묶이는가: <code>import m</code>과 <code>import "./m"</code>은 모듈 이름 <code>m</code>을, <code>as c</code>는 <code>c</code>를 묶는다. 이름 목록만 쓰면(<code>import "./m" (f)</code>) 목록의 이름만 묶이고 모듈 이름은 묶이지 않는다. <code>as</code>와 목록을 함께 쓰면(<code>import "./m" as c (f)</code>) 둘 다 묶인다.</li>
  <li><code>import</code>는 <strong>끌어올려진다</strong>: 파일 어디에 썼든 그 파일의 다른 문장보다 먼저, 쓴 순서대로 가져온다. 그래서 가져온 모듈의 최상위 대입은 진입 파일의 첫 문장보다 먼저 실행된다. <code>import</code>는 파일의 최상위에만 쓴다(함수나 블록 안이면 E0311).</li>
  <li>모듈 이름은 가릴 수 없다: 가져온 모듈과 같은 이름을 묶는 것은 어디서든 컴파일 에러 E0308이다. 최상위와 블록 안의 대입(구조 분해 포함), <code>fn</code> 이름, <code>fn</code>·람다의 매개변수(패턴 안의 이름 포함), <code>for</code>의 변수, <code>match</code> 절이 묶는 이름 모두다. 타입 별칭은 이름 공간이 달라 해당하지 않는다.</li>
  <li>모듈은 값이다: 함수 인자로 넘길 수 있고 <code>type_of(fs) == 'module</code>.</li>
  <li>없는 멤버(E0303)와 비공개 멤버(E0304)는 <code>json._x</code>처럼 모듈 이름에 바로 쓰면 컴파일할 때, 모듈을 값으로 넘겨 <code>m._x</code>로 읽으면 실행할 때 같은 번호로 멈춘다.</li>
</ul>

## 경계 사례와 에러 {#edges}

없는 모듈은 찾은 자리와 함께 멈춘다. 표준 모듈은 이름으로, 내 파일은 경로로 가져온다.

```fung run exit=2 error=E0301
import fs
print(type_of(fs.exists))
import nosuch
```

`import`는 끌어올려진다. 아래에서 `c`의 최상위 대입(`print`)이 진입 파일의 첫 줄보다 먼저 돈다.

```file c.fung
_x = print("init c")

fn g() = 1
```

```fung run
print("start")
import "./c"
print(c.g())
```

목록만 쓰면 모듈 이름은 묶이지 않는다. `as`를 함께 쓰면 둘 다 묶인다.

```file b.fung
fn f() = 1
```

```fung run exit=2 error=E0204
import "./b" (f)
print(f(), b.f())
```

```file b.fung
fn f() = 1
```

```fung run
import "./b" as c (f)
print(f(), c.f())
```

모듈 이름은 변수·매개변수·패턴의 이름으로 가릴 수 없고, `import`는 최상위에만 쓴다.

```fung run exit=2 error=E0308
import json
fn parse(json) = json
print(parse(1))
```

```fung run exit=2 error=E0308
import json
for json in ["a"]: print(json) ;;
```

```fung run exit=2 error=E0311
fn load():
  import json
  1
;;
print(load())
```

`_`로 시작하는 이름은 모듈 밖으로 나가지 않는다.

```fung run exit=2 error=E0304
import json
print(json._x)
```

모듈을 값으로 넘겨 읽으면 같은 검사가 실행할 때 일어난다.

```fung run exit=1 error=E0304
import json
fn secret(m) = m._x
print(secret(json))
```
