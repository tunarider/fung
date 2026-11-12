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
  <li>import된 파일의 최상위에는 <code>import</code>, <code>fn</code>, <code>type</code>, <code>test</code>, <code>이름 = 식</code>만 올 수 있다(그 밖의 문장은 E0306). 이 제한은 문장의 모양만 본다: 대입의 오른쪽은 어떤 식이든 되므로 <code>_x = print("init")</code>처럼 효과가 있는 초기화도 돈다. 막는 것은 효과가 아니라, 값을 남기지 않는 맨 문장이 모듈을 가져올 때 몰래 도는 일이다. 처음 import될 때 한 번 실행되고 캐시된다. 같은 파일을 심볼릭 링크나 대소문자만 다른 경로로 가져와도 같은 모듈 하나다(파일 자체로 가린다).</li>
  <li>순환 import는 컴파일 에러이고 순환 경로 전체를 보여 준다. 진입 파일을 import할 수도 없다.</li>
  <li>무엇이 묶이는가: <code>import m</code>과 <code>import "./m"</code>은 모듈 이름 <code>m</code>을, <code>as c</code>는 <code>c</code>를 묶는다. 이름 목록만 쓰면(<code>import "./m" (f)</code>) 목록의 이름만 묶이고 모듈 이름은 묶이지 않는다. <code>as</code>와 목록을 함께 쓰면(<code>import "./m" as c (f)</code>) 둘 다 묶인다.</li>
  <li><code>import</code>는 파일 <strong>맨 앞</strong>, 다른 모든 문장보다 먼저 쓴다(뒤에 오면 E0313). 쓴 순서대로 가져오므로, 가져온 모듈의 최상위 대입은 진입 파일의 첫 문장보다 먼저 실행된다. 함수나 블록 안의 <code>import</code>는 E0311이다.</li>
  <li>모듈 이름은 가릴 수 없다: 가져온 모듈과 같은 이름을 묶는 것은 어디서든 컴파일 에러 E0308이다. 최상위와 블록 안의 대입(구조 분해 포함), <code>fn</code> 이름, <code>fn</code>·람다의 매개변수(패턴 안의 이름 포함), <code>for</code>의 변수, <code>match</code> 절이 묶는 이름 모두다. 타입 별칭은 이름 공간이 달라 해당하지 않는다.</li>
  <li>import가 묶은 이름은 다른 이름처럼 다시 묶을 수 없고(E0240, <a href="scope.html">변수와 스코프</a>), 파일에서 한 번도 읽지 않으면 E0239다. 그래서 prelude 함수와 같은 이름의 멤버는 목록으로 가져오지 못하고 <code>regex.split</code>처럼 모듈 이름으로 읽는다.</li>
  <li>import된 파일의 <strong>인터페이스</strong>, 곧 공개 최상위 이름과 공개 <code>fn</code>의 매개변수 이름은 prelude 함수와 같은 이름이어도 된다(<code>http.get</code>, <code>path.join</code>, <code>http.get(url, timeout: 5s)</code>). 부르는 쪽은 그것을 모듈 이름을 붙여 읽거나 인자 이름으로만 쓰기 때문이다. 그 파일 안에서는 그 이름이 prelude 함수를 가린다. 공개 이름은 그 파일에서 읽지 않아도 에러가 아니다. 같은 파일을 진입 파일로 실행하거나 <code>fung check</code>하면 인터페이스가 아니므로 이 둘이 에러다.</li>
  <li>모듈은 값이다: 함수 인자로 넘길 수 있고 <code>type_of(fs) == 'module</code>.</li>
  <li>없는 멤버(E0303)와 비공개 멤버(E0304)는 <code>json._x</code>처럼 모듈 이름에 바로 쓰면 컴파일할 때, 모듈을 값으로 넘겨 <code>m._x</code>로 읽으면 실행할 때 같은 번호로 멈춘다.</li>
</ul>

## 경계 사례와 에러 {#edges}

없는 모듈은 찾은 자리와 함께 멈춘다. 표준 모듈은 이름으로, 내 파일은 경로로 가져온다.

```fung run exit=2 error=E0301
import fs
import nosuch
print(type_of(fs.exists))
```

`import`는 파일 맨 앞에 쓴다. 가져온 모듈의 최상위 대입(`print`)이 진입 파일의 첫 문장보다 먼저 돈다.

```file c.fung
_x = print("init c")

fn g() = 1
```

```fung run
import "./c"
print("start")
print(c.g())
```

다른 문장 뒤의 `import`는 에러다.

```fung run exit=2 error=E0313
print("start")
import json
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

가져온 이름도 prelude 함수를 가릴 수 없다. 그런 멤버는 모듈 이름으로 읽는다.

```fung run exit=2 error=E0240
import regex (split)
print(split("a,b", ","))
```

```fung run
import regex
print(regex.split("a,b", ","))
```

가져오고 읽지 않는 import는 에러다.

```fung run exit=2 error=E0239
import json
print(1)
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
