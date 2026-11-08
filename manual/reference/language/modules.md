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
  <li>파일 하나가 모듈 하나다. 모듈 이름은 파일명이고, 이름 규칙에 맞지 않으면(<code>my-lib.fung</code>) <code>as</code>가 필요하다.</li>
  <li><code>_</code>로 시작하는 최상위 이름은 비공개. 나머지는 공개다. 가져온 이름은 다시 내보내지 않는다.</li>
  <li>import된 파일의 최상위에는 <code>import</code>, <code>fn</code>, <code>type</code>, <code>test</code>, <code>이름 = 식</code>만 올 수 있다. 처음 import될 때 한 번 실행되고 캐시된다.</li>
  <li>순환 import는 컴파일 에러이고 순환 경로 전체를 보여 준다. 진입 파일을 import할 수도 없다.</li>
  <li>모듈은 값이다: 함수 인자로 넘길 수 있고 <code>type_of(fs) == 'module</code>.</li>
</ul>

## 경계 사례와 에러 {#edges}

없는 모듈은 찾은 자리와 함께 멈춘다. 표준 모듈은 이름으로, 내 파일은 경로로 가져온다.

```fung run exit=2 error=E0301
import fs
print(type_of(fs.exists))
import nosuch
```

`_`로 시작하는 이름은 모듈 밖으로 나가지 않는다.

```fung run exit=2 error=E0304
import json
print(json._x)
```
