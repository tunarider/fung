# M4 자료구조와 패턴 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 리스트·맵·튜플·태그·범위 값, 인덱스·슬라이스·필드, `match`와 패턴(리터럴·이름·태그·튜플·리스트·맵·`_`·`|`·`when`), 구조 분해 대입, `!` `?` `??`, `|>`, `_` 부분 적용, `for`, `in`/`not in`, `type` 별칭, 가변 인자 `...rest`를 만든다. 02의 비교 표가 모든 타입에 대해 동작한다.

**Architecture:** 패턴은 코어 트리의 `p*` 노드로 표현되고, resolve가 묶이는 이름에 슬롯을 주며, 컴파일러가 **패턴 트리 상수**(`Proto.Patterns`)로 바꾸고, VM의 `MATCH n` 한 명령이 실행 중에 해석해 슬롯에 쓴다 (09: 패턴 의미가 Go 함수 하나에 모인다). `match`는 대상을 숨은 슬롯에 두고 절마다 `MATCH` → `JUMPIFFALSE` → (guard) → 본문 → `JUMP end`로 풀린다. `|>`는 desugar가 호출로, `_` 부분 적용은 숨은 임시 변수 `let` + 람다로, `for`는 `map` 호출 + 람다로 풀린다 (언어 핵심을 늘리지 않는다, 05·06 원칙). `!` `?` `??`는 전용 명령 셋이다.

**Tech Stack:** Go 1.27, 표준 라이브러리만.

**Spec:** `01-syntax.md` (EBNF의 list/map/tuple/pattern/for/match, `_`, `in`), `02-values-and-types.md` (자료구조, 순회 대상, 태그, 비교 표, `! ? ??` 표), `03-functions-and-patterns.md` (패턴, 구조 분해, 부분 적용, `for` 안 `return`), `07-operators-and-types.md` (`in`, 범위·슬라이스, 우선순위, 타입 별칭), `09-implementation-plan.md` (`MATCH`, 결과값 축약).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**.
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉·스택 추적은 사용자에게 보이지 않는다. 명령마다 소스 위치. 종료 코드 0/1/2/3.
- **에러 번호는 한 번 붙이면 바꾸지 않는다.** E0207·E0510(08의 예시 번호)은 쓰지 않는다.
- 렉서는 닫혀 있다. 불변 자료구조는 **단순 복사**로 (09).
- 스코프 모델 C (03): 함수는 만들어질 때 보이는 변수를 복사한다. 패턴 이름은 항상 새로 묶는다.

## 계획 수준의 결정

1. **prelude 추가는 `map`과 `len`뿐.** `for`가 `map`의 설탕이라 필요하고, 테스트에 `len`이 필요하다. 나머지 컬렉션 함수는 M5.
2. **`?`를 함수 밖(최상위)에서 쓰면 컴파일 에러** E0214 (힌트: `!`). 02는 "현재 함수에서 반환"이라 했고 최상위에는 함수가 없다.
3. **`match`의 대상과 `_` 부분 적용의 미리 평가된 인자는 숨은 변수**(`$m0`, `$p0` — 식별자 문자가 아니라 사용자가 쓸 수 없다)에 담는다. desugar가 만들고 resolve·컴파일러는 보통 변수로 다룬다.
4. **`for` 본문은 람다**이고 `FnInfo.ForBody`로 표시해 그 안의 `return`을 E0213으로 잡는다 (03). 패턴 인자 `for (k, v) in …`는 `$f0`를 받아 본문 첫 줄에서 구조 분해한다.
5. **태그 패턴 `Ok` (괄호 없음)는 심볼 `'Ok`에만 맞는다** (03의 함정, 그대로).
6. **`in`의 대상은 리스트(원소 `==`), 문자열(부분 문자열), 맵(키), 범위(정수 포함)**. 그 외 E0512.
7. **`_` 부분 적용은 가장 안쪽 호출에 속하고 이름 인자 자리에는 못 쓴다** (03). 호출 인자와 패턴 밖의 `_`는 E0215.
8. **타입 별칭은 최상위 문장**이고 파일 안에서만 쓴다 (04). 별칭이 별칭을 가리킬 수 있고, 자기를 가리키면 E0218, 두 번 정의하면 E0217.

## Review Focus

1. 패턴에 묶인 이름이 실패한 절에서 쓰인 슬롯을 다음 절이 재사용해도 섞이지 않는다 (Task 7 테스트: 첫 절이 두 이름을 묶다 실패, 둘째 절의 다른 이름이 그 슬롯을 씀).
2. `[first, ...rest]` 패턴에 빈 리스트 → 실패(E0601), `[...rest]`에 빈 리스트 → `rest = []` (Task 7).
3. `xs[-1]`, `xs[0..0]`(빈 슬라이스), `xs[3..]`(len 3 → 빈), `xs[4..]`(len 3 → 에러), `"참치"[1]`(룬) (Task 1·7).
4. 중첩 `match`에서 안쪽 `match`의 숨은 슬롯이 바깥 대상을 덮어쓰지 않는다 (Task 4·7).
5. `f(_, _)`의 결과를 두 번 부를 때 미리 평가된 인자가 재평가되지 않는다 (Task 7: `print` 부수효과 횟수).

## 파일 구조

| 패키지 | 변경 | 작업 |
|------|------|------|
| `internal/diag`, `internal/value` | 새 번호, `List` `Map` `Tuple` `Tag` `Range`, 포맷, 비교, 인덱스·슬라이스·`in` 도우미 | Task 1 |
| `internal/parser` | 리터럴, 후위 `[]`·`.`·`!`·`?`, 새 이항 연산자, `_`, `match`·패턴, `for`, `type`, 구조 분해 | Task 2 |
| `internal/desugar` | pipe→call, partial→let+fn, for→map, match 숨은 변수, `not in` | Task 3 |
| `internal/resolve` | 패턴 스코프·슬롯, 절, `for` 본문 `return`, `?` 위치, `_`, 대안 이름, 타입 별칭, rest | Task 4 |
| `internal/bytecode` | 새 명령, `Pattern`, `Proto.Patterns`, `Param.Rest` | Task 5 |
| `internal/compiler` | 새 코어 형태 전부 | Task 6 |
| `internal/vm`, `internal/native` | 명령 구현, 패턴 엔진, rest 바인딩, `map`·`len` | Task 7 |
| `cmd/fung`, `tests/lang`, `docs` | 골든, 노트, 상태 | Task 8 |

### M4에서 할당하는 에러 번호

| 번호 | 메시지 | 비고 |
|------|------|------|
| E0213 | `return inside a for body` | 힌트: `` `for` builds a list; use `find`, `all`/`any`, or `fold` instead `` |
| E0214 | `` `?` outside a function `` | 힌트: `` use `!` to stop the script on Err `` |
| E0215 | `` `_` can only be a call argument or a pattern `` | |
| E0216 | `alternatives of a pattern must bind the same names` | |
| E0217 | `` type `%s` is defined twice `` | |
| E0218 | `` type `%s` refers to itself `` | |
| E0507 | `index %d is out of range (length %d)` | 런타임 |
| E0508 | `` key `%s` not found `` | 런타임. 힌트: `` use get(m, key, default) or ?? for a default `` |
| E0509 | `cannot index a value of type %s` | 런타임 |
| E0511 | `cannot slice a value of type %s` | 런타임 |
| E0512 | `cannot test membership in a value of type %s` | 런타임 |
| E0601 | `no pattern matched %s` | 런타임 |
| E0602 | `value %s does not match the pattern` | 런타임 |
| E0603 | `unhandled Err: %s` | 런타임 (`!`) |
| E0604 | `` `%s` on nil `` | 런타임 |
| E0605 | `` `%s` on a value of type %s (not Ok or Err) `` | 런타임 |

---

### Task 1: diag·value — 자료구조 값, 포맷, 비교, 인덱스·슬라이스·`in`

**Files:**
- Modify: `internal/diag/catalog.go`
- Create: `internal/value/collections.go`, `internal/value/index.go`
- Modify: `internal/value/format.go`, `internal/value/compare.go`, `internal/value/arith.go`
- Test: `internal/value/collections_test.go`, `internal/value/index_test.go`

**Interfaces:**
```go
type List struct{ Items []Value }          // TypeName "list"
type Tuple struct{ Items []Value }         // "tuple"
type Map struct{ keys []string; vals map[string]Value } // "map", insertion-ordered
func NewMap() *Map
func (m *Map) Set(k string, v Value)       // in-place; only while building (compiler/VM construct fresh maps)
func (m *Map) Get(k string) (Value, bool)
func (m *Map) Keys() []string
func (m *Map) Len() int
func (m *Map) Put(k string, v Value) *Map  // copy with k set (06 put)
type Tag struct{ Name Sym; Args []Value }  // TypeName = Name.Name() (02: type_of(Circle(3)) == 'Circle)
type Range struct{ Start, End int64; Inclusive bool } // "range"
func (r Range) Len() int64
func (r Range) Contains(i int64) bool
func (r Range) Items() []Value             // 게으른 값이지만 map/for가 리스트를 만들 때 씀

func Len(v Value) (int, *diag.Error)       // str(룬), list, tuple, map, range; 그 외 E0501("take the length of") — 단항이라 E0506 사용
func Index(v, i Value) (Value, *diag.Error)      // list/str: int(음수는 뒤에서), map: str; E0507/E0508/E0509
func Slice(v Value, start, end Value) (Value, *diag.Error) // list/str; start/end는 Int 또는 Nil(열린 끝); E0507/E0511
func Contains(container, x Value) (bool, *diag.Error)     // list/str/map/range; E0512
func Concat(a, b Value) (Value, *diag.Error)              // list + list (Add가 호출)
```
- `Format`: `[1, "a"]`, `(1, 2)`·`(1,)`·`()`, `{name: "x", "a b": 1}`(키가 이름 규칙에 맞으면 따옴표 없이), `Circle(3)`, `Ok(1)`, `0..10`, `0..=10`. 빈 리스트 `[]`, 빈 맵 `{}`.
- `Equal`: 02 표. list/tuple 길이+원소, map 키 집합+값(순서 무시), tag 이름+인자, range 세 필드. list ≠ tuple.
- `Compare`: list/tuple 사전식(앞 원소부터, 같으면 짧은 쪽이 작음), 서로 다른 종류면 E0501.
- `Add`: list+list 이어 붙이기.

- [ ] **Step 1: 테스트**

```go
// internal/value/collections_test.go
package value

import "testing"

func list(vs ...Value) *List { return &List{Items: vs} }

func TestCollectionFormat(t *testing.T) {
	m := NewMap()
	m.Set("name", Str("x"))
	m.Set("a b", Int(1))
	cases := map[Value]string{
		list(Int(1), Str("a"), Nil{}):          `[1, "a", nil]`,
		list():                                  "[]",
		&Tuple{Items: []Value{Int(1), Int(2)}}:  "(1, 2)",
		&Tuple{Items: []Value{Int(1)}}:          "(1,)",
		&Tuple{}:                                "()",
		m:                                       `{name: "x", "a b": 1}`,
		NewMap():                                "{}",
		&Tag{Name: Intern("Circle"), Args: []Value{Int(3)}}:         "Circle(3)",
		&Tag{Name: Intern("Rect"), Args: []Value{Int(2), Int(5)}}:   "Rect(2, 5)",
		Range{Start: 0, End: 10}:                                    "0..10",
		Range{Start: 0, End: 10, Inclusive: true}:                   "0..=10",
		list(list(Int(1)), &Tuple{Items: []Value{Str("s")}}):        `[[1], ("s",)]`,
	}
	for v, want := range cases {
		if got := Format(v); got != want {
			t.Errorf("Format = %q, want %q", got, want)
		}
	}
	if (&Tag{Name: Intern("Circle")}).TypeName() != "Circle" || list().TypeName() != "list" || NewMap().TypeName() != "map" || (&Tuple{}).TypeName() != "tuple" || (Range{}).TypeName() != "range" {
		t.Error("TypeName")
	}
}

func TestCollectionEquality(t *testing.T) {
	m1, m2 := NewMap(), NewMap()
	m1.Set("a", Int(1))
	m1.Set("b", Int(2))
	m2.Set("b", Int(2))
	m2.Set("a", Int(1))
	cases := []struct {
		a, b Value
		want bool
	}{
		{list(Int(1), Int(2)), list(Int(1), Float(2)), true},
		{list(Int(1)), list(Int(1), Int(2)), false},
		{list(Int(1)), &Tuple{Items: []Value{Int(1)}}, false},
		{m1, m2, true},
		{&Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, &Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, true},
		{&Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, &Tag{Name: Intern("Err"), Args: []Value{Int(1)}}, false},
		{Range{0, 3, false}, Range{0, 3, false}, true},
		{Range{0, 3, false}, Range{0, 3, true}, false},
		{list(), Nil{}, false},
	}
	for _, c := range cases {
		if got := Equal(c.a, c.b); got != c.want {
			t.Errorf("Equal(%s, %s) = %v", Format(c.a), Format(c.b), got)
		}
	}
}

func TestCollectionOrdering(t *testing.T) {
	if c, err := Compare(list(Int(1), Int(2)), list(Int(1), Int(3))); err != nil || c != -1 {
		t.Errorf("lexicographic: %d %v", c, err)
	}
	if c, _ := Compare(list(Int(1)), list(Int(1), Int(0))); c != -1 {
		t.Error("shorter is smaller")
	}
	if c, _ := Compare(&Tuple{Items: []Value{Str("b")}}, &Tuple{Items: []Value{Str("a")}}); c != 1 {
		t.Error("tuples")
	}
	for _, pair := range [][2]Value{{list(), &Tuple{}}, {NewMap(), NewMap()}, {&Tag{Name: Intern("A")}, &Tag{Name: Intern("A")}}, {list(Int(1)), list(Str("a"))}} {
		if _, err := Compare(pair[0], pair[1]); err == nil || err.Code != "E0501" {
			t.Errorf("Compare(%s, %s) should fail", Format(pair[0]), Format(pair[1]))
		}
	}
}

func TestListConcatAndMapPut(t *testing.T) {
	v, err := Add(list(Int(1)), list(Int(2)))
	if err != nil || Format(v) != "[1, 2]" {
		t.Fatalf("concat: %v %v", v, err)
	}
	if _, err := Add(list(), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("list + int")
	}
	m := NewMap()
	m.Set("a", Int(1))
	m2 := m.Put("b", Int(2))
	if m.Len() != 1 || m2.Len() != 2 || Format(m2) != "{a: 1, b: 2}" {
		t.Fatalf("Put must copy: %s %s", Format(m), Format(m2))
	}
	m3 := m2.Put("a", Int(9))
	if Format(m3) != "{a: 9, b: 2}" {
		t.Fatalf("Put keeps order: %s", Format(m3))
	}
}
```

```go
// internal/value/index_test.go
package value

import "testing"

func TestLen(t *testing.T) {
	cases := map[Value]int{Str("참치"): 2, list(Int(1), Int(2)): 2, &Tuple{}: 0, Range{Start: 0, End: 10}: 10, Range{0, 10, true}: 11, Range{5, 2, false}: 0}
	for v, want := range cases {
		if got, err := Len(v); err != nil || got != want {
			t.Errorf("Len(%s) = %d %v, want %d", Format(v), got, err, want)
		}
	}
	m := NewMap()
	m.Set("k", Nil{})
	if got, _ := Len(m); got != 1 {
		t.Error("map len")
	}
	if _, err := Len(Int(1)); err == nil || err.Code != "E0506" {
		t.Error("len of int")
	}
}

func TestIndex(t *testing.T) {
	xs := list(Int(10), Int(20), Int(30))
	cases := []struct {
		v, i Value
		want string
		code string
	}{
		{xs, Int(0), "10", ""}, {xs, Int(-1), "30", ""}, {xs, Int(3), "", "E0507"}, {xs, Int(-4), "", "E0507"},
		{Str("참치"), Int(1), `"치"`, ""}, {Str("ab"), Int(2), "", "E0507"},
		{xs, Str("a"), "", "E0501"}, {&Tuple{Items: xs.Items}, Int(0), "", "E0509"}, {Int(1), Int(0), "", "E0509"},
	}
	m := NewMap()
	m.Set("name", Str("x"))
	cases = append(cases, struct {
		v, i Value
		want string
		code string
	}{m, Str("name"), `"x"`, ""}, struct {
		v, i Value
		want string
		code string
	}{m, Str("age"), "", "E0508"}, struct {
		v, i Value
		want string
		code string
	}{m, Int(0), "", "E0501"})
	for _, c := range cases {
		got, err := Index(c.v, c.i)
		if c.code != "" {
			if err == nil || err.Code != c.code {
				t.Errorf("Index(%s, %s) err = %v, want %s", Format(c.v), Format(c.i), err, c.code)
			}
			continue
		}
		if err != nil || Format(got) != c.want {
			t.Errorf("Index(%s, %s) = %v %v, want %s", Format(c.v), Format(c.i), got, err, c.want)
		}
	}
}

func TestSlice(t *testing.T) {
	xs := list(Int(1), Int(2), Int(3))
	cases := []struct {
		start, end Value
		want       string
		code       string
	}{
		{Int(1), Int(3), "[2, 3]", ""}, {Nil{}, Int(2), "[1, 2]", ""}, {Int(-2), Nil{}, "[2, 3]", ""},
		{Int(0), Int(0), "[]", ""}, {Int(3), Nil{}, "[]", ""}, {Int(4), Nil{}, "", "E0507"}, {Int(0), Int(4), "", "E0507"},
		{Int(2), Int(1), "", "E0507"}, {Nil{}, Nil{}, "[1, 2, 3]", ""},
	}
	for _, c := range cases {
		got, err := Slice(xs, c.start, c.end)
		if c.code != "" {
			if err == nil || err.Code != c.code {
				t.Errorf("Slice(%s, %s) err = %v, want %s", Format(c.start), Format(c.end), err, c.code)
			}
			continue
		}
		if err != nil || Format(got) != c.want {
			t.Errorf("Slice(%s, %s) = %v %v, want %s", Format(c.start), Format(c.end), got, err, c.want)
		}
	}
	if got, _ := Slice(Str("참치회"), Int(1), Nil{}); Format(got) != `"치회"` {
		t.Errorf("string slice by rune: %v", got)
	}
	if _, err := Slice(&Tuple{}, Nil{}, Nil{}); err == nil || err.Code != "E0511" {
		t.Error("tuple slice")
	}
}

func TestContains(t *testing.T) {
	m := NewMap()
	m.Set("k", Int(1))
	cases := []struct {
		c, x Value
		want bool
	}{
		{list(Int(1), Int(2)), Int(2), true}, {list(Int(1)), Float(1), true}, {list(), Int(1), false},
		{Str("hello"), Str("ell"), true}, {Str("hello"), Str("z"), false},
		{m, Str("k"), true}, {m, Str("z"), false},
		{Range{0, 10, false}, Int(9), true}, {Range{0, 10, false}, Int(10), false}, {Range{0, 10, true}, Int(10), true}, {Range{0, 10, false}, Float(1), false},
	}
	for _, c := range cases {
		if got, err := Contains(c.c, c.x); err != nil || got != c.want {
			t.Errorf("Contains(%s, %s) = %v %v", Format(c.c), Format(c.x), got, err)
		}
	}
	if _, err := Contains(Str("a"), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("str contains int")
	}
	if _, err := Contains(Int(1), Int(1)); err == nil || err.Code != "E0512" {
		t.Error("int container")
	}
}
```

- [ ] **Step 2: 실패 확인**, **Step 3: 구현**

카탈로그에 번호 표 추가. `collections.go`:

```go
package value

import (
	"strings"
	"unicode"
)

type List struct{ Items []Value }
type Tuple struct{ Items []Value }

// Map keeps insertion order (02) with a side index for lookups.
type Map struct {
	keys []string
	vals map[string]Value
}

func NewMap() *Map { return &Map{vals: map[string]Value{}} }

// Set adds or replaces a key in place; use it only while building a map
// nobody else can see yet (constructors in the VM and natives).
func (m *Map) Set(k string, v Value) {
	if _, ok := m.vals[k]; !ok {
		m.keys = append(m.keys, k)
	}
	m.vals[k] = v
}

func (m *Map) Get(k string) (Value, bool) { v, ok := m.vals[k]; return v, ok }
func (m *Map) Keys() []string           { return m.keys }
func (m *Map) Len() int                 { return len(m.keys) }

// Put returns a copy with k set, keeping the original's order (06 put).
func (m *Map) Put(k string, v Value) *Map {
	out := NewMap()
	for _, key := range m.keys {
		out.Set(key, m.vals[key])
	}
	out.Set(k, v)
	return out
}

// Tag is a tagged value: Circle(3), Ok(x). Its type name is the tag's name (02).
type Tag struct {
	Name Sym
	Args []Value
}

// Range is a lazy integer range (02): Start..End or Start..=End.
type Range struct {
	Start, End int64
	Inclusive  bool
}

func (r Range) Len() int64 {
	n := r.End - r.Start
	if r.Inclusive {
		n++
	}
	if n < 0 {
		return 0
	}
	return n
}

func (r Range) Contains(i int64) bool {
	if r.Inclusive {
		return i >= r.Start && i <= r.End
	}
	return i >= r.Start && i < r.End
}

// Items materializes the range (for map/for, which build lists anyway).
func (r Range) Items() []Value {
	n := r.Len()
	out := make([]Value, 0, n)
	for i := int64(0); i < n; i++ {
		out = append(out, Int(r.Start+i))
	}
	return out
}

func (*List) TypeName() string   { return "list" }
func (*Tuple) TypeName() string  { return "tuple" }
func (*Map) TypeName() string    { return "map" }
func (t *Tag) TypeName() string  { return t.Name.Name() }
func (Range) TypeName() string   { return "range" }

func formatItems(items []Value) string {
	parts := make([]string, len(items))
	for i, v := range items {
		parts[i] = Format(v)
	}
	return strings.Join(parts, ", ")
}

// isPlainKey reports whether a map key can be written without quotes.
func isPlainKey(k string) bool {
	if k == "" {
		return false
	}
	for i, r := range k {
		if i == 0 && !(r == '_' || unicode.IsLetter(r)) {
			return false
		}
		if !(r == '_' || unicode.IsLetter(r) || unicode.IsDigit(r)) {
			return false
		}
	}
	return true
}
```
`format.go`의 `Format`에:
```go
	case *List:
		return "[" + formatItems(x.Items) + "]"
	case *Tuple:
		if len(x.Items) == 1 {
			return "(" + Format(x.Items[0]) + ",)"
		}
		return "(" + formatItems(x.Items) + ")"
	case *Map:
		parts := make([]string, len(x.keys))
		for i, k := range x.keys {
			key := k
			if !isPlainKey(k) {
				key = quoteStr(k)
			}
			parts[i] = key + ": " + Format(x.vals[k])
		}
		return "{" + strings.Join(parts, ", ") + "}"
	case *Tag:
		return x.Name.Name() + "(" + formatItems(x.Args) + ")"
	case Range:
		op := ".."
		if x.Inclusive {
			op = "..="
		}
		return strconv.FormatInt(x.Start, 10) + op + strconv.FormatInt(x.End, 10)
```
`compare.go`의 `Equal`에 list/tuple/map/tag/range 케이스(`equalItems` 도우미), `Compare`에 list/tuple 사전식(`compareItems`). `arith.go`의 `Add`에 `*List` 케이스(새 슬라이스로 이어 붙임).

`index.go`:
```go
package value

import "github.com/tunarider/fung/internal/diag"

func Len(v Value) (int, *diag.Error) {
	switch x := v.(type) {
	case Str:
		return len([]rune(string(x))), nil
	case *List:
		return len(x.Items), nil
	case *Tuple:
		return len(x.Items), nil
	case *Map:
		return x.Len(), nil
	case Range:
		return int(x.Len()), nil
	}
	return 0, diag.Errorf("E0506", diag.Pos{}, "take the length of", v.TypeName())
}

// normIndex turns a possibly negative index into a position, or E0507.
func normIndex(i int64, n int) (int, *diag.Error) {
	idx := int(i)
	if idx < 0 {
		idx += n
	}
	if idx < 0 || idx >= n {
		return 0, diag.Errorf("E0507", diag.Pos{}, i, n)
	}
	return idx, nil
}

func Index(v, i Value) (Value, *diag.Error) {
	switch x := v.(type) {
	case *List:
		n, ok := i.(Int)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		idx, err := normIndex(int64(n), len(x.Items))
		if err != nil {
			return nil, err
		}
		return x.Items[idx], nil
	case Str:
		n, ok := i.(Int)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		rs := []rune(string(x))
		idx, err := normIndex(int64(n), len(rs))
		if err != nil {
			return nil, err
		}
		return Str(string(rs[idx])), nil
	case *Map:
		k, ok := i.(Str)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		val, found := x.Get(string(k))
		if !found {
			return nil, diag.Errorf("E0508", diag.Pos{}, string(k))
		}
		return val, nil
	}
	return nil, diag.Errorf("E0509", diag.Pos{}, v.TypeName())
}

// sliceBounds resolves optional start/end (Nil = open) against length n.
// Both ends may equal n; start may not exceed end (02: 범위를 벗어나면 에러).
func sliceBounds(start, end Value, n int) (int, int, *diag.Error) {
	lo, hi := 0, n
	if s, ok := start.(Int); ok {
		lo = int(s)
		if lo < 0 {
			lo += n
		}
	} else if _, isNil := start.(Nil); !isNil {
		return 0, 0, diag.Errorf("E0501", diag.Pos{}, "slice with", "list", start.TypeName())
	}
	if e, ok := end.(Int); ok {
		hi = int(e)
		if hi < 0 {
			hi += n
		}
	} else if _, isNil := end.(Nil); !isNil {
		return 0, 0, diag.Errorf("E0501", diag.Pos{}, "slice with", "list", end.TypeName())
	}
	if lo < 0 || lo > n {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, lo, n)
	}
	if hi < 0 || hi > n {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, hi, n)
	}
	if lo > hi {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, lo, n)
	}
	return lo, hi, nil
}

func Slice(v, start, end Value) (Value, *diag.Error) {
	switch x := v.(type) {
	case *List:
		lo, hi, err := sliceBounds(start, end, len(x.Items))
		if err != nil {
			return nil, err
		}
		return &List{Items: append([]Value(nil), x.Items[lo:hi]...)}, nil
	case Str:
		rs := []rune(string(x))
		lo, hi, err := sliceBounds(start, end, len(rs))
		if err != nil {
			return nil, err
		}
		return Str(string(rs[lo:hi])), nil
	}
	return nil, diag.Errorf("E0511", diag.Pos{}, v.TypeName())
}

// Contains implements `in` (07).
func Contains(container, x Value) (bool, *diag.Error) {
	switch c := container.(type) {
	case *List:
		for _, item := range c.Items {
			if Equal(item, x) {
				return true, nil
			}
		}
		return false, nil
	case Str:
		s, ok := x.(Str)
		if !ok {
			return false, diag.Errorf("E0501", diag.Pos{}, "search", container.TypeName(), x.TypeName())
		}
		return strings.Contains(string(c), string(s)), nil
	case *Map:
		k, ok := x.(Str)
		if !ok {
			return false, diag.Errorf("E0501", diag.Pos{}, "search", container.TypeName(), x.TypeName())
		}
		_, found := c.Get(string(k))
		return found, nil
	case Range:
		i, ok := x.(Int)
		if !ok {
			return false, nil
		}
		return c.Contains(int64(i)), nil
	}
	return false, diag.Errorf("E0512", diag.Pos{}, container.TypeName())
}
```
(`strings` import. `MatchesType`: 태그 값이면 `v.TypeName()`이 태그 이름이라 `s: Circle`이 그대로 동작한다.)

- [ ] **Step 4**, **Step 5: 커밋** `feat(value): add list, map, tuple, tag, and range values with indexing and membership`.

---

### Task 2: parser — 리터럴, 후위·이항 연산자, `_`, `match`와 패턴, `for`, `type`, 구조 분해

**Files:**
- Create: `internal/parser/pattern.go`, `internal/parser/collections.go`
- Modify: `internal/parser/expr.go`, `internal/parser/stmt.go`
- Test: `internal/parser/m4_test.go`

**Interfaces (겉 문법 형태):**
- 리터럴: `(list e…)`, `(map (entry <key> e)…)` (entry의 Lit = 키 문자열), `(tuple e…)` (`()`, `(e,)`, `(a, b)`; `(e)`는 괄호), `(tag Name e…)` (TAG 뒤에 `(`), 그냥 TAG → `(symbol Name)`.
- 후위(결합력 130, `[`는 같은 줄에서만): `(index x i)`, `(slice x start end)` (열린 끝은 `(nil)`), `(field x name)` (Lit 이름), `(unwrap e)` (Lit `"!"` 또는 `"?"`).
- 이항: `(pipe x rhs)` (60), `(coalesce a b)` (70), `(range a b)` (80, Lit 포함 여부 bool), `(binary in a b)`·`(binary not_in a b)` (50, 비교 그룹: 연쇄 금지).
- `_`: 호출 인자 자리(`f(a, _)`)에서 `(hole)`. 패턴에서 `(pwild)`. 그 외는 `(name "_")`로 두고 resolve가 E0215.
- `(match e (arm pat guard body)…)`: guard가 없으면 `(nil)`. body는 `(block …)`. 절 판정은 **문장 시작에서 `패턴 [when expr] =>`를 시도**해서 성공하면 새 절 (01).
- `(for pat e (block …))`.
- `(typealias name)` Lit = `[]string` 대안 (문장).
- 구조 분해: 문장 시작에서 `패턴 =`를 시도해 `(assign <pattern> e)`. 단순 이름은 기존대로 `(assign (name x) e)`.
- 패턴 노드: `(pwild)`, `(pbind x)`(Lit 이름), `(plit v)`(Lit: int64·float64·string·bool·time.Duration), `(pnil)`, `(psym name)`, `(ptag Name pat…)`(Lit 이름), `(ptuple pat…)`, `(plist pat… [(prest name)|(prest)])`, `(pmap (pentry key pat)…)`(pentry Lit 키), `(palt pat…)`.

문법 (01 EBNF 그대로). 패턴 파싱은 별도 함수 `pattern()`; 되돌리기는 `p.i` 저장·복원.

- [ ] **Step 1: 테스트**

```go
// internal/parser/m4_test.go
package parser

import "testing"

func TestCollectionLiterals(t *testing.T) {
	cases := map[string]string{
		"[1, 2, 3]":               "(program (list 1 2 3))",
		"[]":                      "(program (list))",
		"[\n  1,\n  2,\n]":        "(program (list 1 2))",
		"{name: \"a\", tags: [1]}": `(program (map (entry name "a") (entry tags (list 1))))`,
		"{\"a b\": 1}":            "(program (map (entry a b 1)))",
		"{}":                      "(program (map))",
		"()":                      "(program (tuple))",
		"(1,)":                    "(program (tuple 1))",
		"(1, 2)":                  "(program (tuple 1 2))",
		"(1)":                     "(program 1)",
		"Circle(3)":               "(program (tag Circle 3))",
		"Rect(2, 5)":              "(program (tag Rect 2 5))",
		"Red":                     "(program 'Red)",
		"Ok(x) + 1":               "(program (binary + (tag Ok x) 1))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestPostfixAndNewInfix(t *testing.T) {
	cases := map[string]string{
		"xs[0]":                   "(program (index xs 0))",
		"xs[-1]":                  "(program (index xs (unary - 1)))",
		"xs[1..3]":                "(program (slice xs 1 3))",
		"xs[..3]":                 "(program (slice xs (nil) 3))",
		"xs[-4..]":                "(program (slice xs (unary - 4) (nil)))",
		"xs[1..=3]":               "(program (index xs (range 1 3)))", // inclusive range value as index (07: 범위 값)
		"m.name.first":            "(program (field (field m name) first))",
		"f(1)!":                   "(program (unwrap (call f 1)))",
		"f(1)?":                   "(program (unwrap (call f 1)))",
		"a ?? b ?? c":             "(program (coalesce (coalesce a b) c))",
		"xs |> map(f) |> sum":     "(program (pipe (pipe xs (call map f)) sum))",
		"0..10":                   "(program (range 0 10))",
		"0..=10":                  "(program (range 0 10))",
		"x in xs":                 "(program (binary in x xs))",
		"x not in xs":             "(program (binary not_in x xs))",
		"files |> len > 3":        "(program (binary > (pipe files len) 3))",
		"a + b |> f":              "(program (pipe (binary + a b) f))",
		"fs.read(p) ?? \"{}\" |> json.parse": `(program (pipe (coalesce (call (field fs read) p) "{}") (field json parse)))`,
		"xs\n  |> map(f)":         "(program (pipe xs (call map f)))",
		"m\n  .name":              "(program (field m name))",
		"f(_)":                    "(program (call f (hole)))",
		"path.join(dir, _)":       "(program (call (field path join) dir (hole)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	// Lit of range/unwrap is visible through dedicated checks since String() hides it.
	toks, _, _ := lexerLex("0..=10")
	n, _ := Parse(toks)
	if n.Kids[0].Lit != true {
		t.Error("..= should set the inclusive flag")
	}
	toks, _, _ = lexerLex("f()?")
	n, _ = Parse(toks)
	if n.Kids[0].Lit != "?" {
		t.Error("unwrap lit")
	}
	for _, src := range []string{"x in xs in ys", "a < b in c", "f(a: _)", "f(_ + 1)"} {
		if got := parseErr(t, src); got != "E0111" && got != "E0110" {
			t.Errorf("%q: %s", src, got)
		}
	}
}

func TestPatternsAndMatch(t *testing.T) {
	cases := map[string]string{
		"match v:\n  0 => a\n  _ => b\n;;":                    "(program (match v (arm (plit 0) (nil) (block a)) (arm (pwild) (nil) (block b))))",
		"match v: 'ok => 1; Ok(x) => x; Err(e) => 0 ;;":        "(program (match v (arm (psym ok) (nil) (block 1)) (arm (ptag Ok (pbind x)) (nil) (block x)) (arm (ptag Err (pbind e)) (nil) (block 0))))",
		"match p: (0, 0) => a; (x, 0) => b ;;":                 "(program (match p (arm (ptuple (plit 0) (plit 0)) (nil) (block a)) (arm (ptuple (pbind x) (plit 0)) (nil) (block b))))",
		"match xs: [] => 0; [first, ...rest] => 1; [a, ...] => 2 ;;": "(program (match xs (arm (plist) (nil) (block 0)) (arm (plist (pbind first) (prest rest)) (nil) (block 1)) (arm (plist (pbind a) (prest)) (nil) (block 2))))",
		"match m: {name: n, port: 80} => n ;;":                 "(program (match m (arm (pmap (pentry name (pbind n)) (pentry port (plit 80))) (nil) (block n))))",
		"match x: n when n > 100 => big; 'a | 'b => ab ;;":     "(program (match x (arm (pbind n) (binary > n 100) (block big)) (arm (palt (psym a) (psym b)) (nil) (block ab))))",
		"match v: -1 => neg; \"s\" => s; 1.5 => f; nil => z; true => t; 1h => d ;;": `(program (match v (arm (plit -1) (nil) (block neg)) (arm (plit "s") (nil) (block s)) (arm (plit 1.5) (nil) (block f)) (arm (pnil) (nil) (block z)) (arm (plit true) (nil) (block t)) (arm (plit 1h) (nil) (block d))))`,
		"match r:\n  Ok(text) =>\n    lines = f(text)\n    len(lines)\n  Err(e) => print(e); 0\n;;": "(program (match r (arm (ptag Ok (pbind text)) (nil) (block (assign lines (call f text)) (call len lines))) (arm (ptag Err (pbind e)) (nil) (block (call print e) 0))))",
		"match v: Red => 1 ;;":                                 "(program (match v (arm (psym Red) (nil) (block 1))))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"match v: (x) => 1 ;;":      "E0110", // (p) is not a pattern
		"match v: [...a, b] => 1 ;;": "E0110", // rest must be last
		"match v: 1 + 2 => 1 ;;":    "E0110",
		"match v: ;;":               "E0110", // no arms
		"match v: x => ;;":          "E0110", // empty arm body
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestForTypeAndDestructuring(t *testing.T) {
	cases := map[string]string{
		"for x in xs: print(x) ;;":            "(program (for (pbind x) xs (block (call print x))))",
		"for (k, v) in pairs(m): k ;;":        "(program (for (ptuple (pbind k) (pbind v)) (call pairs m) (block k)))",
		"for i in 0..10: i ;;":                "(program (for (pbind i) (range 0 10) (block i)))",
		"type shape = Circle | Rect":           "(program (typealias shape))",
		"(q, r) = divmod(7, 2)":                "(program (assign (ptuple (pbind q) (pbind r)) (call divmod 7 2)))",
		"[first, ...rest] = files":             "(program (assign (plist (pbind first) (prest rest)) files))",
		"{name: n, port: p} = config":          "(program (assign (pmap (pentry name (pbind n)) (pentry port (pbind p))) config))",
		"Ok(text) = fs.read(\"a\")":            `(program (assign (ptag Ok (pbind text)) (call (field fs read) "a")))`,
		"x = 1":                                "(program (assign x 1))",
		"(a, b)":                               "(program (tuple a b))", // no `=`: an expression
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "for x xs: 1 ;;"); got != "E0112" {
		t.Errorf("for without in: %s", got)
	}
	if got := parseErr(t, "type Shape = Circle"); got != "E0110" {
		t.Errorf("alias must be lowercase: %s", got)
	}
}
```
`lexerLex`는 `lexer.Lex("t.fung", src)`를 감싼 테스트 도우미다 (import 추가). `(map (entry a b 1))`처럼 `entry`의 Lit이 공백을 포함한 키면 출력이 모호하지만 테스트용 표시일 뿐이다.

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현** (핵심 부분)

`expr.go`: `infix` 표에 추가 —
```go
	lexer.Pipe:     {bpPipe, bpPipe + 1, "|>", false},
	lexer.Coalesce: {bpCoalesce, bpCoalesce + 1, "??", false},
	lexer.DotDot:   {bpRange, bpRange + 1, "..", false},
	lexer.DotDotEq: {bpRange, bpRange + 1, "..=", false},
	lexer.KwIn:     {bpCompare, bpCompare + 1, "in", true},
```
`expr()` 루프: `t.Kind == lexer.KwNot && p.toks[p.i+1].Kind == lexer.KwIn`이면 두 토큰을 먹고 `not_in` 비교 연산자로 처리(연쇄 검사 포함). 노드 생성은 연산자별로: `|>` → `tree.New("pipe", …)`, `??` → `coalesce`, `..`/`..=` → `tree.WithLit("range", pos, inclusive, a, b)`, 나머지 → `binary`.
후위 루프(`bpPostfix >= minBP`이고 같은 줄): `[` → `p.indexOrSlice(lhs)`; `.` 뒤 NAME → `(field lhs name)` (`.`는 줄 이어짐 토큰이라 `peekInfix`가 다음 줄의 `.`도 본다); `!`/`?` → `(unwrap lhs)` Lit. 호출 `(`는 기존대로.

```go
// indexOrSlice parses `[i]`, `[a..b]`, `[..b]`, `[a..]` after an expression.
func (p *parser) indexOrSlice(x *tree.Node) (*tree.Node, *diag.Error) {
	open := p.next() // [
	p.nest++
	defer func() { p.nest-- }()
	var start, end *tree.Node
	var err *diag.Error
	if p.at(lexer.DotDot) {
		p.next()
		if p.at(lexer.RBracket) {
			p.next()
			return tree.New("slice", open.Pos, x, tree.New("nil", open.Pos), tree.New("nil", open.Pos)), nil
		}
		end, err = p.expr(bpRange + 1)
		if err != nil {
			return nil, err
		}
		if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
			return nil, err
		}
		return tree.New("slice", open.Pos, x, tree.New("nil", open.Pos), end), nil
	}
	start, err = p.expr(bpRange + 1) // stop before `..`
	if err != nil {
		return nil, err
	}
	if p.at(lexer.DotDot) {
		p.next()
		if p.at(lexer.RBracket) {
			p.next()
			return tree.New("slice", open.Pos, x, start, tree.New("nil", open.Pos)), nil
		}
		end, err = p.expr(bpRange + 1)
		if err != nil {
			return nil, err
		}
		if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
			return nil, err
		}
		return tree.New("slice", open.Pos, x, start, end), nil
	}
	if p.at(lexer.DotDotEq) {
		// xs[1..=3]: an inclusive range value as the index.
		p.next()
		end, err = p.expr(bpRange + 1)
		if err != nil {
			return nil, err
		}
		if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
			return nil, err
		}
		return tree.New("index", open.Pos, x, tree.WithLit("range", open.Pos, true, start, end)), nil
	}
	if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
		return nil, err
	}
	return tree.New("index", open.Pos, x, start), nil
}
```
`prefix()`: `LBracket` → `p.listLiteral()`; `LBrace` → `p.mapLiteral()`; `LParen` → 람다 검사 → `p.parenOrTuple()`; `Tag` → 다음이 같은 줄 `(`면 `(tag Name args)`(인자는 `call()`의 인자 파싱 재사용: `callArgs()`로 분리), 아니면 `(symbol Name)`; `KwMatch` → `p.matchExpr()`; `KwFor` → `p.forExpr()`. `call()`의 인자 루프: `Name "_"`이고 다음이 `,`/`)`면 `(hole)`; 이름 인자 값이 `_`면 E0110.

`stmt.go`의 `statement()`: `KwType` + Name → `typeAlias()` (이름은 소문자 시작이어야: Tag 토큰이면 E0110); 문장 시작 토큰이 `(` `[` `{` 또는 TAG면 `tryDestructure()`: `save := p.i; pat, err := p.pattern(); if err == nil && p.at(Assign) { … (assign pat rhs) } else { p.i = save; fall through }`.

`pattern.go`:
```go
// pattern = pat_term { "|" pat_term }
func (p *parser) pattern() (*tree.Node, *diag.Error) {
	first, err := p.patTerm()
	if err != nil {
		return nil, err
	}
	if !p.at(lexer.Bar) {
		return first, nil
	}
	alt := tree.New("palt", first.Pos, first)
	for p.at(lexer.Bar) {
		p.next()
		term, err := p.patTerm()
		if err != nil {
			return nil, err
		}
		alt.Kids = append(alt.Kids, term)
	}
	return alt, nil
}

func (p *parser) patTerm() (*tree.Node, *diag.Error) {
	t := p.peek()
	switch t.Kind {
	case lexer.Name:
		p.next()
		if t.Text == "_" {
			return tree.New("pwild", t.Pos), nil
		}
		return tree.Leaf("pbind", t.Pos, t.Text), nil
	case lexer.Int:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Int), nil
	case lexer.Float:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Float), nil
	case lexer.Minus:
		p.next()
		n := p.peek()
		switch n.Kind {
		case lexer.Int:
			p.next()
			return tree.Leaf("plit", t.Pos, -n.Int), nil
		case lexer.Float:
			p.next()
			return tree.Leaf("plit", t.Pos, -n.Float), nil
		}
		return nil, p.unexpected()
	case lexer.String:
		p.next()
		for _, part := range t.Parts {
			if part.IsCode {
				return nil, diag.Errorf("E0110", t.Pos, "interpolation in a pattern")
			}
		}
		var text strings.Builder
		for _, part := range t.Parts {
			text.WriteString(part.Text)
		}
		return tree.Leaf("plit", t.Pos, text.String()), nil
	case lexer.RawString:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Text), nil
	case lexer.Symbol:
		p.next()
		return tree.Leaf("psym", t.Pos, t.Text), nil
	case lexer.Duration:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Dur), nil
	case lexer.KwTrue, lexer.KwFalse:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Kind == lexer.KwTrue), nil
	case lexer.KwNil:
		p.next()
		return tree.New("pnil", t.Pos), nil
	case lexer.Tag:
		p.next()
		if !p.at(lexer.LParen) || p.toks[p.i].Pos.Line != t.Pos.Line {
			return tree.Leaf("psym", t.Pos, t.Text), nil // bare tag: the symbol (03의 함정)
		}
		p.next()
		n := tree.WithLit("ptag", t.Pos, t.Text)
		return p.patList(n, lexer.RParen)
	case lexer.LParen:
		p.next()
		if p.at(lexer.RParen) {
			p.next()
			return tree.New("ptuple", t.Pos), nil
		}
		first, err := p.pattern()
		if err != nil {
			return nil, err
		}
		if !p.at(lexer.Comma) {
			return nil, diag.Errorf("E0110", p.peek().Pos, "`,` (a parenthesized pattern is not a pattern; use (p,) for a tuple)")
		}
		p.next()
		n := tree.New("ptuple", t.Pos, first)
		return p.patList(n, lexer.RParen)
	case lexer.LBracket:
		p.next()
		n := tree.New("plist", t.Pos)
		for !p.at(lexer.RBracket) {
			if p.at(lexer.Ellipsis) {
				p.next()
				rest := tree.New("prest", t.Pos)
				if p.at(lexer.Name) {
					rest.Lit = p.next().Text
				}
				n.Kids = append(n.Kids, rest)
				if p.at(lexer.Comma) {
					p.next()
				}
				if !p.at(lexer.RBracket) {
					return nil, p.unexpected()
				}
				break
			}
			k, err := p.pattern()
			if err != nil {
				return nil, err
			}
			n.Kids = append(n.Kids, k)
			if p.at(lexer.Comma) {
				p.next()
			} else if !p.at(lexer.RBracket) {
				return nil, p.unexpected()
			}
		}
		p.next()
		return n, nil
	case lexer.LBrace:
		p.next()
		n := tree.New("pmap", t.Pos)
		for !p.at(lexer.RBrace) {
			key, err := p.expectDesc(lexer.Name, "a map key")
			if err != nil {
				return nil, err
			}
			if _, err := p.expectDesc(lexer.Colon, "`:`"); err != nil {
				return nil, err
			}
			v, err := p.pattern()
			if err != nil {
				return nil, err
			}
			n.Kids = append(n.Kids, tree.WithLit("pentry", key.Pos, key.Text, v))
			if p.at(lexer.Comma) {
				p.next()
			} else if !p.at(lexer.RBrace) {
				return nil, p.unexpected()
			}
		}
		p.next()
		return n, nil
	}
	return nil, p.unexpected()
}

// patList parses comma-separated patterns into n until the closing token.
func (p *parser) patList(n *tree.Node, closer lexer.Kind) (*tree.Node, *diag.Error) {
	for !p.at(closer) {
		k, err := p.pattern()
		if err != nil {
			return nil, err
		}
		n.Kids = append(n.Kids, k)
		if p.at(lexer.Comma) {
			p.next()
		} else if !p.at(closer) {
			return nil, p.unexpected()
		}
	}
	p.next()
	return n, nil
}

// armHeadAhead reports whether `pattern [when expr] =>` parses at p.i,
// restoring the position either way (01: 절 판정은 시도로).
func (p *parser) armHeadAhead() bool {
	save, saveNest := p.i, p.nest
	defer func() { p.i, p.nest = save, saveNest }()
	if _, err := p.pattern(); err != nil {
		return false
	}
	if p.at(lexer.KwWhen) {
		p.next()
		if _, err := p.expr(0); err != nil {
			return false
		}
	}
	return p.at(lexer.FatArrow)
}

// matchExpr parses `match e: arms ;;`.
func (p *parser) matchExpr() (*tree.Node, *diag.Error) {
	t := p.next() // match
	subject, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the match subject"); err != nil {
		return nil, err
	}
	saved := p.nest
	p.nest = 0
	defer func() { p.nest = saved }()
	n := tree.New("match", t.Pos, subject)
	p.skipSeps()
	for !p.at(lexer.DoubleSemi) {
		if p.at(lexer.EOF) || !p.armHeadAhead() {
			return nil, p.unexpected()
		}
		pat, err := p.pattern()
		if err != nil {
			return nil, err
		}
		guard := tree.New("nil", pat.Pos)
		if p.at(lexer.KwWhen) {
			p.next()
			guard, err = p.expr(0)
			if err != nil {
				return nil, err
			}
		}
		arrow := p.next() // =>
		body := tree.New("block", arrow.Pos)
		p.skipSeps()
		for !p.at(lexer.DoubleSemi) && !p.at(lexer.EOF) && !p.armHeadAhead() {
			stmt, err := p.statement()
			if err != nil {
				return nil, err
			}
			body.Kids = append(body.Kids, stmt)
			switch p.peek().Kind {
			case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.EOF:
			default:
				return nil, p.unexpected()
			}
			p.skipSeps()
		}
		if len(body.Kids) == 0 {
			return nil, diag.Errorf("E0110", p.peek().Pos, "an empty match arm")
		}
		n.Kids = append(n.Kids, tree.New("arm", pat.Pos, pat, guard, body))
	}
	p.next() // ;;
	if len(n.Kids) == 1 {
		return nil, diag.Errorf("E0110", t.Pos, "a match without arms")
	}
	return n, nil
}

// forExpr parses `for pat in e: body ;;`.
func (p *parser) forExpr() (*tree.Node, *diag.Error) {
	t := p.next() // for
	pat, err := p.pattern()
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.KwIn, "`in` after the for pattern"); err != nil {
		return nil, err
	}
	iter, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the for header"); err != nil {
		return nil, err
	}
	body, err := p.body(t.Pos)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the for"); err != nil {
		return nil, err
	}
	return tree.New("for", t.Pos, pat, iter, body), nil
}
```
`match` 안에서 `expr(0)`로 subject를 읽을 때 `:`가 블록 시작이므로 `subject`는 `:` 앞에서 끝난다 (`:`는 연산자가 아님). `for pat in e`: `pattern()` 뒤 `in`은 `KwIn`이라 패턴이 거기서 멈춘다 ✓. **주의**: `for x in xs`에서 패턴 `x`는 Name이고 다음 토큰이 `in`이다; `armHeadAhead`는 `match` 안에서만 쓴다.

`collections.go`: `listLiteral()`(`[`…`]`, nest++), `mapLiteral()`(키는 Name 또는 보간 없는 String; `entry` 노드), `parenOrTuple()`, `tagCall()`.

- [ ] **Step 4**, **Step 5: 커밋** `feat(parser): parse collections, patterns, match, for, type aliases, and M4 operators`.

---

### Task 3: desugar — pipe, partial, for, match, in

**Files:**
- Modify: `internal/desugar/desugar.go`
- Test: `internal/desugar/desugar_test.go`

**Interfaces (코어 형태):**
- `(pipe x (call f a…))` → `(call f x a…)`; `(pipe x f)` → `(call f x)`.
- `(call f … (hole) …)` → `(do (let (name $p0) f) (let (name $p1) a) … (fn <h0,h1…> (do (call (name $p0) (name $p1) (name h0) …))))`: 구멍이 아닌 인자(와 함수)는 숨은 변수에 **한 번** 평가되고, 구멍은 람다 인자가 된다. 이름 인자는 그대로 람다 안의 호출에 남는다(값은 미리 평가해 숨은 변수에). 카운터는 desugar 한 번에 하나.
- `(for pat e body)` → `(call (prelude map) e (fn <$f> (do (let pat (name $f)) body…)))`; pat이 `(pbind x)`면 람다 인자 이름이 `x`이고 `let`이 없다. 람다의 `FnInfo.ForBody = true`.
- `(match e arms…)` → `(do (let (name $m0) e) (match (name $m0) (arm pat guard' body')…))`.
- `(binary in a b)` → `(op in a b)`; `(binary not_in a b)` → `(op not (op in a b))`.
- `(assign pat e)` → `(let pat e)`. 나머지(`list` `map` `entry` `tuple` `tag` `range` `index` `slice` `field` `unwrap` `coalesce` `typealias` `p*`)는 자식만 desugar.

- [ ] **Step 1: 테스트**

```go
func TestDesugarM4(t *testing.T) {
	cases := map[string]string{
		"xs |> map(f) |> sum":          "(do (call sum (call map xs f)))",
		"x |> f":                       "(do (call f x))",
		"x in xs":                      "(do (op in x xs))",
		"x not in xs":                  "(do (op not (op in x xs)))",
		"(q, r) = f()":                 "(do (let (ptuple (pbind q) (pbind r)) (call f)))",
		"for x in xs: g(x) ;;":         "(do (call (prelude map) xs (fn (x) (do (call g x)))))",
		"for (k, v) in m: k ;;":        "(do (call (prelude map) m (fn ($f0) (do (let (ptuple (pbind k) (pbind v)) $f0) k))))",
		"match v: 0 => a; _ => b ;;":   "(do (do (let $m0 v) (match $m0 (arm (plit 0) (nil) (do a)) (arm (pwild) (nil) (do b)))))",
		"f(a, _)":                      "(do (do (let $p0 f) (let $p1 a) (fn (h0) (do (call $p0 $p1 h0)))))",
		"g(_, k(1), _)":                "(do (do (let $p0 g) (let $p1 (call k 1)) (fn (h0, h1) (do (call $p0 h0 $p1 h1)))))",
		"f(x, b: _)":                   "", // parser error, not reached
		"xs[0] + m.k":                  "(do (op + (index xs 0) (field m k)))",
		"r!":                           "(do (unwrap r))",
	}
	for src, want := range cases {
		if want == "" {
			continue
		}
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestForBodyLambdaIsMarked(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "for x in xs: x ;;")
	s, _ := parser.Parse(toks)
	c, _ := Desugar(s)
	fn := c.Kids[0].Kids[2]
	if info, ok := fn.Lit.(*tree.FnInfo); !ok || !info.ForBody {
		t.Fatalf("for body lambda = %s", fn)
	}
}
```
(`tree.FnInfo`에 `ForBody bool` 필드 추가 — Task 2에서 미리 넣는다.) 테스트에 `tree` import.

- [ ] **Step 2**, **Step 3: 구현** — `Desugar`를 `*desugarer{counter int}` 메서드로 바꾸고(숨은 이름 번호), `hidden(prefix) string` → `fmt.Sprintf("$%s%d", prefix, n)`. `pipe`: rhs가 `call`이면 `x`를 kids[1]에 끼워 넣고, 아니면 `(call rhs x)`. `call`에 `hole`이 있으면 `partial()`로. `for`→`forLoop()`. `match`→`matchExpr()`.

- [ ] **Step 4**, **Step 5: 커밋** `feat(desugar): lower pipes, partial application, for, match, and membership`.

---

### Task 4: resolve — 패턴 스코프, 절, `for` 본문, `?`, `_`, 타입 별칭, rest

**Files:**
- Modify: `internal/resolve/resolve.go`, `internal/resolve/fn.go`
- Create: `internal/resolve/pattern.go`, `internal/resolve/alias.go`
- Test: `internal/resolve/m4_test.go`

**Interfaces (코어 출력):**
- 패턴의 `(pbind x)` → `(pbind (local n))` 또는 `(pbind (global n))` (자식 하나). `(prest x)` → `(prest (local n)|(global n))`. 나머지 패턴 노드 그대로.
- `(let pat e)`: 오른쪽 먼저, 그다음 패턴 이름을 **대입 규칙**으로 묶는다 (같은 스코프 재대입 OK, 바깥이면 E0205).
- `(match subj (arm pat guard body)…)`: 절마다 새 블록 스코프; 패턴 이름은 **새로 묶는다**(가리기 허용, 04); guard와 body는 그 스코프에서.
- `(fn …)`의 `ForBody`가 참이면 그 함수(와 중첩 블록, 중첩 fn 제외) 안의 `return` → E0213.
- `(unwrap e)` Lit `"?"`가 함수 밖 → E0214.
- `(hole)`과 `(name "_")` → E0215. `palt` 대안의 묶는 이름 집합이 다르면 E0216.
- `(typealias name)`: 루트 블록에서만(아니면 E0110으로 파서가 막는다 — 파서가 `statement()`에서 `type`을 어디서나 받으므로 resolve가 루트가 아니면 E0219… 번호 절약을 위해 **파서가** 최상위 깊이(`p.nest == 0`이고 블록 안이 아님)를 모르므로 resolve에서 E0217 메시지 대신 전용 번호 없이 `E0110`을 낼 수는 없다 → **E0219 `type aliases must be at the top level`** 추가.) 루트에서는 끌어올려 먼저 등록(E0217 중복), 타입 표기에서 별칭을 재귀 확장(E0218 순환), 미정의면 E0212.
- `Param.Rest`: 슬롯은 보통 인자와 같이 `i`. E0805 제거.

- [ ] **Step 1: 테스트**

```go
// internal/resolve/m4_test.go
package resolve

import (
	"strings"
	"testing"
)

func TestDestructuringBindsLikeAssignment(t *testing.T) {
	r, errs := res(t, "(q, r) = f()\nprint(q, r)\n[a, ...rest] = xs\n(q, r) = g()")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (ptuple (pbind (global 0)) (pbind (global 1))) (call (prelude f) ... "
	_ = want
	got := r.Core.String()
	if !strings.Contains(got, "(let (ptuple (pbind (global 0)) (pbind (global 1)))") || !strings.Contains(got, "(prest (global 3))") {
		t.Fatalf("got %s", got)
	}
	if r.Globals[0] != "q" || r.Globals[3] != "rest" {
		t.Fatalf("globals = %v", r.Globals)
	}
	_, errs = res(t, "total = 0\nif true: (total, n) = (1, 2) ;;")
	if len(errs) != 1 || errs[0].Code != "E0205" {
		t.Fatalf("destructuring an outer name: %v", errs)
	}
}

func TestMatchArmsShadowAndScope(t *testing.T) {
	r, errs := res(t, "x = 1\nmatch 2:\n  x when x > 1 => x\n  _ => x\n;;\nprint(x)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	got := r.Core.String()
	// The arm's x is a fresh slot; outside the match, x is still the global.
	if !strings.Contains(got, "(arm (pbind (local 1)) (op > (local 1) 1) (do (local 1)))") || !strings.Contains(got, "(call (prelude print) (global 0))") {
		t.Fatalf("got %s", got)
	}
	if r.NumLocals != 2 { // $m0 and the arm's x
		t.Fatalf("NumLocals = %d", r.NumLocals)
	}
	_, errs = res(t, "match 1: 'a | 'b => 1 ;;\nmatch 1: Ok(x) | Err(y) => 1 ;;")
	if len(errs) != 1 || errs[0].Code != "E0216" {
		t.Fatalf("alt names: %v", errs)
	}
	_, errs = res(t, "match 1: x => 1 ;;\nprint(x)")
	if len(errs) != 1 || errs[0].Code != "E0206" {
		t.Fatalf("arm name leaks: %v", errs)
	}
}

func TestNestedMatchUsesSeparateHiddenSlots(t *testing.T) {
	r, errs := res(t, "match 1:\n  a => match 2:\n    b => a + b\n  ;;\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(let (local 0) 1)") || !strings.Contains(got, "(let (local 2) 2)") {
		t.Fatalf("got %s", got)
	}
}

func TestForBodyReturnAndQuestionOutsideFn(t *testing.T) {
	cases := map[string]string{
		"for x in xs: return x ;;":                "E0213",
		"for x in xs: if x: return 1 ;; ;;":       "E0213",
		"for x in xs: (() => return x)() ;;":      "",      // a nested fn may return
		"fn f(xs) = for x in xs: return x ;;":     "E0213",
		"f()?":                                    "E0214",
		"fn g() = f()?":                           "",
		"print(_)":                                "",      // hole: fine
		"_ + 1":                                   "E0215",
		"x = _":                                   "E0215",
	}
	for src, want := range cases {
		_, errs := res(t, src)
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
}

func TestTypeAliases(t *testing.T) {
	cases := map[string]string{
		"type shape = Circle | Rect\nfn area(s: shape) = s":       "",
		"fn area(s: shape) = s\ntype shape = Circle":               "", // hoisted
		"type a = b\ntype b = int\nfn f(x: a) = x":                 "",
		"type a = a\nfn f(x: a) = x":                               "E0218",
		"type a = b\ntype b = a\nfn f(x: a) = x":                   "E0218",
		"type a = int\ntype a = str":                               "E0217",
		"fn f(): type a = int ;;":                                  "E0219",
		"type a = zzz":                                             "E0212",
	}
	for src, want := range cases {
		_, errs := res(t, src)
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
	r, _ := res(t, "type shape = Circle | Rect\nfn area(s: shape | nil) = s")
	info := fnInfo(t, r.Core, 0, 1)
	if strings.Join(info.Params[0].Type, "|") != "Circle|Rect|nil" {
		t.Fatalf("expanded type = %v", info.Params[0].Type)
	}
}

func TestRestParamIsAllowedNow(t *testing.T) {
	r, errs := res(t, "fn f(a, ...xs) = xs")
	if len(errs) != 0 || !fnInfo(t, r.Core, 0, 1).Params[1].Rest {
		t.Fatalf("errs = %v", errs)
	}
}
```
(`TestDestructuringBindsLikeAssignment`의 `want`/`_ = want` 두 줄은 지운다. 그리고 M3의 `fn_test.go`에서 `"fn f(...xs) = xs": "E0805"` 항목을 지운다.)

- [ ] **Step 2**, **Step 3: 구현** — 요점:
  - `fnScope`에 `forBody bool`. `function()`에서 `decl.ForBody`를 복사. `walk`의 `"return"`: `r.fn.parent == nil` → E0211; `r.fn.forBody` → E0213.
  - `"unwrap"`: `n.Lit == "?" && r.fn.parent == nil` → E0214 (그 뒤 자식 walk).
  - `"hole"` → E0215; `"name"`에서 `_` → E0215.
  - `"let"`: 대상이 `name`이면 기존; 아니면 `r.walk(rhs)` 뒤 `r.bindPattern(pat, r.bindNew)`.
  - `"match"`: subj walk; 절마다 `pushBlock(body)`→ `bindPattern(pat, r.bindFresh)` → guard, body(`r.block`가 아니라 같은 스코프에서 kids walk — body는 `(do …)`라 `block()`이 스코프를 하나 더 연다; 그래도 된다) → `popBlock`.
  - `bindFresh(name)`: 현재 블록에 새 슬롯(또는 전역)을 **항상** 만든다 (이미 있으면 재사용). `bindPattern(pat, bind func)`: `pbind`/`prest`에 Lit 이름이 있으면 ref로 교체; `palt`: 각 대안의 이름 집합을 모아 비교(E0216), 같으면 첫 대안에서 묶고 나머지 대안의 같은 이름은 같은 ref.
  - `alias.go`: 루트 pre-scan으로 `aliases map[string][]string` (E0217), `expand(alts, visiting)` (E0218), `checkTypes`가 확장을 적용해 `info.Params[i].Type`/`Return`을 **확장된 목록으로 교체**. `typealias` 노드는 코어에서 **제거**(출력하지 않음). 루트가 아닌 곳의 `typealias` → E0219.
  - `rest`: E0805 제거.

- [ ] **Step 4**, **Step 5: 커밋** `feat(resolve): resolve patterns, match arms, for bodies, type aliases, and rest params`.

---

### Task 5: bytecode — 새 명령, `Pattern`, `Proto.Patterns`, `Param.Rest`

**Files:**
- Create: `internal/bytecode/pattern.go`
- Modify: `internal/bytecode/op.go`, `proto.go`, `disasm.go`
- Test: `internal/bytecode/m4_test.go`

**Interfaces:**
```go
// 새 명령
OpList        // a1..an -- list            (arg = n)
OpMap         // k1 v1 .. kn vn -- map     (arg = n; keys are Str)
OpTuple       // a1..an -- tuple           (arg = n)
OpTag         // name a1..an -- tag        (arg = n; name is a Sym)
OpRange       // start end -- range        (arg = 1 if inclusive)
OpIndex       // x i -- x[i]
OpSlice       // x [start] [end] -- slice  (arg bit0 = has start, bit1 = has end)
OpField       // x -- x.name               (arg = const index of the Str name)
OpIn          // x c -- bool
OpUnwrapAbort // r -- v   (! : Ok(v) -> v, Err -> E0603, nil -> E0604, other -> E0605)
OpUnwrapTest  // r -- v or (Err, jump arg)  (? : Ok(v) -> v and fall through; Err -> leave Err, ip = arg)
OpCoalesce    // r -- v or (jump arg)       (??: Ok(v) -> v, nil/Err -> pop and ip = arg, other -> itself)
OpMatch       // v -- bool  (arg = Patterns index; binds slots/globals on success)
OpMatchOrFail // v --       (like MATCH but E0602 on failure)
OpNoMatch     // v --       (E0601 with the value)

type PatKind int
const ( PatWild PatKind = iota; PatBind; PatLit; PatTag; PatTuple; PatList; PatMap; PatAlt )
type Pattern struct {
	Kind   PatKind
	Lit    value.Value // PatLit: the literal (Int, Float, Str, Bool, Nil, Sym, Dur)
	Name   string      // PatTag: tag name; PatMap entry key (in Keys)
	Global bool        // PatBind: where to write
	Index  int
	Kids   []*Pattern  // PatTag args, PatTuple/PatList items, PatAlt alternatives, PatMap values
	Keys   []string    // PatMap: keys parallel to Kids
	Rest   *Pattern    // PatList: nil = exact length; PatWild = ignore rest; PatBind = bind rest
}
func (p *Pattern) String() string   // `_`, `x`, `1`, `Ok(x)`, `(a, b)`, `[a, ...rest]`, `{k: p}`, `a | b` (pbind prints as `$n`/`@n`)
// Proto: Patterns []*Pattern; Param.Rest bool
```
- 역어셈블러: `MATCH 0    ; Ok(x)`, `TAG 1    ; 1 args`, `SLICE 3    ; start, end`, `FIELD 2    ; name`.

- [ ] **Step 1: 테스트**
```go
func TestPatternString(t *testing.T) {
	p := &Pattern{Kind: PatAlt, Kids: []*Pattern{
		{Kind: PatTag, Name: "Ok", Kids: []*Pattern{{Kind: PatBind, Index: 1}}},
		{Kind: PatList, Kids: []*Pattern{{Kind: PatLit, Lit: value.Int(1)}}, Rest: &Pattern{Kind: PatWild}},
		{Kind: PatMap, Keys: []string{"k"}, Kids: []*Pattern{{Kind: PatBind, Global: true, Index: 2}}},
		{Kind: PatTuple, Kids: []*Pattern{{Kind: PatWild}, {Kind: PatLit, Lit: value.Str("s")}}},
	}}
	if got := p.String(); got != `Ok($1) | [1, ...] | {k: @2} | (_, "s")` {
		t.Fatalf("got %s", got)
	}
}

func TestM4OpsDisassemble(t *testing.T) {
	p := &Proto{Name: "<main>", ReturnType: -1, Patterns: []*Pattern{{Kind: PatBind, Index: 0}}}
	pos := diag.Pos{Line: 1, Col: 1}
	p.Emit(OpList, 2, pos)
	p.Emit(OpTag, 1, pos)
	p.Emit(OpSlice, 3, pos)
	p.Emit(OpField, p.Const(value.Str("name")), pos)
	p.Emit(OpMatch, 0, pos)
	p.Emit(OpRange, 1, pos)
	p.Emit(OpIn, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:1   LIST         2\n" +
		"0001  1:1   TAG          1    ; 1 args\n" +
		"0002  1:1   SLICE        3    ; start, end\n" +
		"0003  1:1   FIELD        0    ; name\n" +
		"0004  1:1   MATCH        0    ; $0\n" +
		"0005  1:1   RANGE        1    ; inclusive\n" +
		"0006  1:1   IN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
```
- [ ] **Step 2**, **Step 3: 구현** — 상수·이름·`HasArg`(IN·INDEX·UNWRAPABORT·NOMATCH은 false), `Pattern`과 `String`, Proto 필드, 역어셈블러 주석.
- [ ] **Step 4**, **Step 5: 커밋** `feat(bytecode): add collection, unwrap, and pattern instructions`.

---

### Task 6: compiler — 새 코어 형태

**Files:**
- Create: `internal/compiler/pattern.go`, `internal/compiler/m4.go`
- Modify: `internal/compiler/compiler.go`, `internal/compiler/fn.go`
- Test: `internal/compiler/m4_test.go`

**Interfaces (코어 → 명령):**
- `(list k…)` → k…; `LIST n`. `(map (entry key v)…)` → 각 `CONST key; v`; `MAP n`. `(tuple k…)` → `TUPLE n`. `(tag Name k…)` → `CONST 'Name; k…; TAG n`. `(range a b)` Lit → a; b; `RANGE incl`. `(index x i)` → x; i; `INDEX`. `(slice x s e)` → x; (s가 `(nil)`이 아니면 s); (e …); `SLICE flags`. `(field x name)` → x; `FIELD const`. `(op in a b)` → a; b; `IN`.
- `(unwrap e)` `"!"` → e; `UNWRAPABORT`. `"?"` → e; `UNWRAPTEST L1`; `JUMP L2`; `L1:` [`CHECKRETURN`] `RETURN`; `L2:`.
- `(coalesce a b)` → a; `COALESCE L1`; `JUMP L2`; `L1:` b; `L2:`.
- `(let pat e)` (pat이 `name`이 아닌 경우) → e; `MATCHORFAIL p`.
- `(match subj arms…)` → 절마다: subj; `MATCH p`; `JUMPIFFALSE next`; [guard; `JUMPIFFALSE next`]; body(tail); `JUMP end`; `next:`. 마지막: subj; `NOMATCH`. `end:`. 절 본문은 꼬리 위치.
- 패턴 변환 `buildPattern(n) *bytecode.Pattern`: `pbind (local n)` → Bind, `(global n)` → Bind Global; `plit` Lit → value(Int/Float/Str/Bool/Dur); `pnil` → Lit `Nil{}`; `psym` → Lit `Intern(name)`; `ptag`; `ptuple`; `plist` (+`prest`); `pmap`; `palt`; `pwild`.
- `Param.Rest` 복사.

- [ ] **Step 1: 테스트**
```go
func TestCompileCollectionsAndOps(t *testing.T) {
	cases := map[string]string{
		"[1, 2]":            "CONST(0) CONST(1) LIST(2) RETURN",
		"{a: 1}":            "CONST(0) CONST(1) MAP(1) RETURN",
		"(1,)":              "CONST(0) TUPLE(1) RETURN",
		"Ok(1)":             "CONST(0) CONST(1) TAG(1) RETURN",
		"0..=3":             "CONST(0) CONST(1) RANGE(1) RETURN",
		"xs = []\nxs[0]":    "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) INDEX RETURN",
		"xs = []\nxs[1..]":  "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(1) RETURN",
		"xs = []\nxs[..2]":  "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(2) RETURN",
		"m = {}\nm.k":       "MAP(0) STOREGLOBAL(0) LOADGLOBAL(0) FIELD(0) RETURN",
		"1 in [1]":          "CONST(0) CONST(0) LIST(1) IN RETURN",
		"Ok(1)!":            "CONST(0) CONST(1) TAG(1) UNWRAPABORT RETURN",
		"nil ?? 1":          "CONST(0) COALESCE(4) JUMP(5) CONST(1) RETURN",
	}
	for src, want := range cases {
		if got := ops(compile(t, src)); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestCompileQuestionInsideFunction(t *testing.T) {
	p := compile(t, "fn f(r) = r? + 1")
	// r; UNWRAPTEST L1; JUMP L2; L1: RETURN; L2: CONST 1; ADD; RETURN
	if got, want := ops(p.Protos[0]), "LOADLOCAL(0) UNWRAPTEST(3) JUMP(4) RETURN CONST(0) ADD RETURN"; got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	p = compile(t, "fn f(r) -> int = r?")
	if got := ops(p.Protos[0]); !strings.Contains(got, "UNWRAPTEST(3) JUMP(5) CHECKRETURN(0) RETURN") {
		t.Fatalf("annotated: %s", got)
	}
}

func TestCompileMatchAndDestructuring(t *testing.T) {
	p := compile(t, "match 1:\n  0 => \"zero\"\n  n when n > 0 => \"pos\"\n  _ => \"other\"\n;;")
	want := "CONST(0) STORELOCAL(0) " + // $m0 = 1
		"LOADLOCAL(0) MATCH(0) JUMPIFFALSE(7) CONST(1) JUMP(21) " + // arm 0
		"LOADLOCAL(0) MATCH(1) JUMPIFFALSE(16) LOADLOCAL(1) CONST(2) GT JUMPIFFALSE(16) CONST(3) JUMP(21) " + // arm 1 with guard
		"LOADLOCAL(0) MATCH(2) JUMPIFFALSE(20) CONST(4) JUMP(21) " + // arm 2
		"LOADLOCAL(0) NOMATCH RETURN"
	if got := ops(p); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if len(p.Patterns) != 3 || p.Patterns[1].String() != "$1" || p.Patterns[0].String() != "0" {
		t.Fatalf("patterns = %v", p.Patterns)
	}
	p = compile(t, "(a, b) = (1, 2)")
	if got, want := ops(p), "CONST(0) CONST(1) TUPLE(2) MATCHORFAIL(0) CONST(2) RETURN"; got != want || p.Patterns[0].String() != "(@0, @1)" {
		t.Fatalf("got %s %v", got, p.Patterns)
	}
}
```
(`match`의 `(do …)` 래퍼가 블록이라 마지막 `let`이 아닌 `match` 식의 값이 블록 값이 된다. 컴파일 결과의 명령 번호는 구현 뒤 실제 출력으로 확인해 테스트를 맞춘다 — 점프 목적지 숫자는 어긋날 수 있으므로 **구현 후 dump로 확인하고 고정**한다.)

- [ ] **Step 2**, **Step 3: 구현**, **Step 4**, **Step 5: 커밋** `feat(compiler): compile collections, patterns, match, unwrap, and new operators`.

---

### Task 7: vm·native — 명령 구현, 패턴 엔진, rest, `map`·`len`

**Files:**
- Create: `internal/vm/match.go`, `internal/vm/m4.go`
- Modify: `internal/vm/vm.go`, `internal/vm/call.go`, `internal/native/prelude.go`
- Test: `internal/vm/m4_test.go`, `internal/vm/match_test.go`

**Interfaces:**
- `matchPattern(t *Task, f *frame, v value.Value, p *bytecode.Pattern) bool`: 성공 시 슬롯/전역에 쓴다. Wild: 참. Bind: 쓰고 참. Lit: `value.Equal`. Tag: `*value.Tag`이고 이름 같고 인자 수 같고 각각 매칭. Tuple: `*value.Tuple` 길이·원소. List: `*value.List`; Rest가 nil이면 길이 같아야, 아니면 `len >= len(Kids)`이고 나머지를 `*List`로 Rest에. Map: `*value.Map`이고 각 키가 있고 매칭 (다른 키 무시). Alt: 차례로 시도.
- `bindArgs`: `Rest` 인자가 있으면 남는 위치 인자를 `*List`로 그 슬롯에; 없으면 빈 리스트. 이름 인자가 rest를 가리키면 E0406.
- `Err` 메시지 규칙 `errMessage(v)`: `*Map`에 `message` 키가 `Str`이면 그 값, 아니면 `Format(v)` (02).
- natives: `map(xs, f)`: 리스트·범위 → `ctx.Call(f, [x])` 결과 리스트; 그 외 E0506 "map over". `len(x)` → `value.Len`.

- [ ] **Step 1: 테스트**

```go
// internal/vm/m4_test.go
package vm

import "testing"

func TestCollectionsRun(t *testing.T) {
	cases := map[string]string{
		"print([1, \"a\", [2]], (1,), (), {name: \"x\", \"a b\": 1}, Circle(3), 0..3, 0..=3)": "[1, \"a\", [2]] (1,) () {name: \"x\", \"a b\": 1} Circle(3) 0..3 0..=3\n",
		"xs = [10, 20, 30]\nprint(xs[0], xs[-1], xs[1..3], xs[..1], xs[-2..], xs[0..0], xs[3..])": "10 30 [20, 30] [10] [20, 30] [] []\n",
		"print(\"참치회\"[1], \"참치회\"[1..], len(\"참치\"), len([1, 2]), len(0..10))":            "치 치회 2 2 10\n",
		"m = {name: \"x\", port: 80}\nprint(m.name, m[\"port\"], len(m))":                    "x 80 2\n",
		"print(1 in [1, 2], 3 in [1, 2], \"ell\" in \"hello\", \"k\" in {k: 1}, 5 in 0..10, 10 in 0..10, 10 in 0..=10, 2 not in [1])": "true false true true true false true true\n",
		"print([1] + [2], [1, 2] == [1, 2], [1] == (1,), {a: 1, b: 2} == {b: 2, a: 1}, [1, 2] < [1, 3], Ok(1) == Ok(1))": "[1, 2] true false true true true\n",
		"print(type_of(Circle(3)), type_of(Red), type_of([]), type_of(0..1))":                "'Circle 'sym 'list 'range\n",
		"fn area(s: Circle | Rect) = 1\nprint(area(Circle(3)))":                             "1\n",
		"xs = [1, 2, 3]\nprint(xs |> len, xs |> map(x => x * 2), xs |> map(x => x + 1) |> len)": "3 [2, 4, 6] 3\n",
		"print(for x in [1, 2]: x * 10 ;;, for i in 0..3: i ;;)":                            "[10, 20] [0, 1, 2]\n",
		"print(for (k, v) in [(1, 2), (3, 4)]: k + v ;;)":                                   "[3, 7]\n",
		"fn sum(...xs) = len(xs)\nprint(sum(), sum(1, 2, 3))":                               "0 3\n",
		"fn f(a, ...rest) = rest\nprint(f(1), f(1, 2, 3))":                                  "[] [2, 3]\n",
		"add = (a, b) => a + b\ninc = add(1, _)\nprint(inc(5), inc(10))":                    "6 11\n",
		"fn join3(a, b, c) = a + b + c\nmid = join3(\"<\", _, \">\")\nprint(mid(\"x\"))":    "<x>\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestPartialEvaluatesFixedArgsOnce(t *testing.T) {
	_, out, _ := run(t, "fn f(a, b) = a + b\ng = f(print(\"once\") then 0 else 1, _)\ng(1)\ng(2)")
	if out != "once\n" {
		t.Fatalf("out = %q", out)
	}
}
```
`print("once") then 0 else 1`은 `print`가 nil을 돌려주므로 E0505다. 대신 `fn side() = print("once")` / `g = f(side(), _)`로 쓴다 (nil + 1은 E0501… → `fn side(): print("once"); 0 ;;`).

```go
func TestResultOperators(t *testing.T) {
	cases := map[string]string{
		"print(Ok(1)!, Ok(2) ?? 0, nil ?? 3, Err(\"e\") ?? 4, 5 ?? 6)":             "1 2 3 4 5\n",
		"fn f(r) = r? + 1\nprint(f(Ok(1)), f(Err(\"bad\")))":                        "2 Err(\"bad\")\n",
		"fn f(r) -> Ok | Err = Ok(r? * 2)\nprint(f(Ok(2)), f(Err('x)))":           "Ok(4) Err('x)\n",
		"print(nil ?? Err(1) ?? 2)":                                                "2\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"Err({kind: 'x, message: \"boom\"})!": "E0603",
		"nil!":                                "E0604",
		"5!":                                  "E0605",
		"fn f(r) = r?\nf(nil)":                "E0604",
		"fn f(r) = r?\nf(1)":                  "E0605",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
	p := compileWith(t, "Err({kind: 'x, message: \"boom\"})!", native.Prelude())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Error() != "error[E0603]: unhandled Err: boom" {
		t.Fatalf("err = %v", err)
	}
}

func TestRuntimeErrorsM4(t *testing.T) {
	cases := map[string]string{
		"[1][1]":                 "E0507",
		"[1][-2]":                "E0507",
		"[1, 2][0..3]":           "E0507",
		"{a: 1}.b":               "E0508",
		"{a: 1}[\"b\"]":          "E0508",
		"(1, 2)[0]":              "E0509",
		"5[0]":                   "E0509",
		"(1,)[0..1]":             "E0511",
		"1 in 1":                 "E0512",
		"1 in \"a\"":             "E0501",
		"[1][\"a\"]":             "E0501",
		"\"a\"..\"b\"":           "E0501",
		"match 1: 0 => 1 ;;":     "E0601",
		"(a, b) = (1, 2, 3)":     "E0602",
		"Ok(x) = Err(1)":         "E0602",
		"map(1, x => x)":         "E0506",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}
```

```go
// internal/vm/match_test.go
package vm

import "testing"

func TestMatchRuns(t *testing.T) {
	cases := map[string]string{
		"fn f(v) = match v:\n  0 => \"zero\"\n  n when n < 0 => \"neg\"\n  n => \"pos ${n}\"\n;;\nprint(f(0), f(-1), f(5))": "zero neg pos 5\n",
		"fn f(r) = match r: Ok(x) => x; Err(e) => \"err:${e}\" ;;\nprint(f(Ok(1)), f(Err('bad)))":                     "1 err:'bad\n",
		"fn f(p) = match p: (0, 0) => \"origin\"; (x, 0) => \"x${x}\"; (0, y) => \"y${y}\"; _ => \"other\" ;;\nprint(f((0, 0)), f((2, 0)), f((0, 3)), f((1, 1)))": "origin x2 y3 other\n",
		"fn len2(xs) = match xs: [] => 0; [_, ...rest] => 1 + len2(rest) ;;\nprint(len2([]), len2([1, 2, 3]))":        "0 3\n",
		"fn f(xs) = match xs: [a, b] => a + b; [a, ...] => a; _ => -1 ;;\nprint(f([1, 2]), f([5, 6, 7]), f([]))":     "3 5 -1\n",
		"fn f(m) = match m: {name: n, port: 80} => \"${n}:80\"; {name: n} => n; _ => \"?\" ;;\nprint(f({name: \"a\", port: 80}), f({name: \"b\", x: 1}), f({}))": "a:80 b ?\n",
		"fn f(c) = match c: 'r | 'g | 'b => \"rgb\"; Red => \"red\"; _ => \"no\" ;;\nprint(f('g), f(Red), f('x))":    "rgb red no\n",
		"fn f(v) = match v: 1 | 1.0 => \"one\"; \"s\" => \"str\"; nil => \"nil\"; true => \"t\"; 1h => \"h\"; _ => \"?\" ;;\nprint(f(1.0), f(\"s\"), f(nil), f(true), f(1h), f(2))": "one str nil t h ?\n",
		"fn f(v) = match v: Ok(Ok(x)) => x; Ok(_) => 0; _ => -1 ;;\nprint(f(Ok(Ok(7))), f(Ok(3)), f(Err(1)))":     "7 0 -1\n",
		"x = 10\nfn f(v) = match v: y when y == x => \"same\"; _ => \"diff\" ;;\nprint(f(10), f(3))":                "", // see below
		"r = match 3: n when n > 2 => \"big\"; _ => \"small\" ;;\nprint(r)":                                         "big\n",
		"fn f(v) = match v: Ok => 1; Ok(_) => 2 ;;\nprint(f('Ok), f(Ok(0)))":                                      "1 2\n", // bare tag pattern is the symbol
	}
	delete(cases, "x = 10\nfn f(v) = match v: y when y == x => \"same\"; _ => \"diff\" ;;\nprint(f(10), f(3))")
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestFailedArmDoesNotLeakBindings(t *testing.T) {
	// Arm 0 binds a and b then fails on the guard; arm 1 binds c into the
	// slot a used. c must see its own value.
	_, out, code := run(t, "fn f(v) = match v: (a, b) when a > 100 => a; (c, _) => c ;;\nprint(f((1, 2)))")
	if code != "" || out != "1\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
	_, out, _ = run(t, "fn f(xs) = match xs: [first, ...rest] => rest; [...rest] => rest ;;\nprint(f([]), f([1]))")
	if out != "[] []\n" {
		t.Fatalf("rest on empty: %q", out)
	}
	_, out, _ = run(t, "fn f(xs) = match xs: [a] => match a: (p, q) => p + q; _ => a ;;; _ => 0 ;;\nprint(f([(1, 2)]), f([5]))")
	if out != "3 5\n" {
		t.Fatalf("nested match: %q", out)
	}
}

func TestDestructuringAssignment(t *testing.T) {
	cases := map[string]string{
		"(q, r) = (7, 2)\nprint(q, r)":                         "7 2\n",
		"[first, ...rest] = [1, 2, 3]\nprint(first, rest)":      "1 [2, 3]\n",
		"{name: n, port: p} = {name: \"a\", port: 80, x: 1}\nprint(n, p)": "a 80\n",
		"Ok(text) = Ok(\"hi\")\nprint(text)":                    "hi\n",
		"fn f():\n  (a, b) = (1, 2)\n  (a, b) = (b, a)\n  a - b\n;;\nprint(f())": "1\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}
```
`TestMatchRuns`의 `x = 10` 케이스와 `delete` 줄은 쓰지 않는다 (모델 C에서 최상위 fn은 x를 못 본다). 대신 함수 안 버전을 넣는다: `"fn f(v, x) = match v: y when y == x => \"same\"; _ => \"diff\" ;;\nprint(f(10, 10), f(3, 10))": "same diff\n"`.

- [ ] **Step 2**, **Step 3: 구현** — `match.go`(패턴 엔진), `m4.go`(명령 switch 추가: 컬렉션 생성, INDEX/SLICE/FIELD/IN, UNWRAP*/COALESCE, MATCH*/NOMATCH), `call.go`의 `bindArgs`에 rest, `prelude.go`에 `map`·`len`.
- [ ] **Step 4**, **Step 5: 커밋** `feat(vm): run collections, pattern matching, result operators, rest args, map, and len`.

---

### Task 8: 골든 테스트와 문서

**Files:**
- Create: `tests/lang/*.fung` (아래), `docs/notes/M4-data-and-patterns.md`
- Modify: `docs/design/09-implementation-plan.md`, `docs/design/03-functions-and-patterns.md` (구조 분해는 대입 규칙 명시), `CLAUDE.md`

골든 (값·흐름 9 + 에러 12):

```
# lists.fung
xs = [10, 20, 30]
print(xs, xs[0], xs[-1], xs[1..3], xs[..1], xs[-2..], len(xs))
# expect: [10, 20, 30] 10 30 [20, 30] [10] [20, 30] 3
print([1] + [2, 3], [] == [], [1, 2] < [1, 3], 2 in xs, 20 in xs)
# expect: [1, 2, 3] true true false true
print(xs |> map(x => x // 10), for x in xs: x + 1 ;;)
# expect: [1, 2, 3] [11, 21, 31]
```
```
# maps.fung
m = {name: "fung", port: 8080, "a b": 1}
print(m, m.name, m["port"], len(m), "name" in m, "x" in m)
# expect: {name: "fung", port: 8080, "a b": 1} fung 8080 3 true false
print({a: 1, b: 2} == {b: 2, a: 1})
# expect: true
```
```
# tuples_tags.fung
fn divmod(a, b) = (a // b, a % b)
(q, r) = divmod(7, 2)
print(q, r, (1,), (), (1, 2) == (1, 2))
# expect: 3 1 (1,) () true
print(Circle(3), Rect(2, 5), Red, Ok(1) == Ok(1), type_of(Circle(3)), type_of(Red))
# expect: Circle(3) Rect(2, 5) 'Red true 'Circle 'sym
```
```
# ranges.fung
print(0..3, 0..=3, len(0..10), 5 in 0..10, 10 in 0..10, 10 in 0..=10)
# expect: 0..3 0..=3 10 true false true
print(for i in 1..=3: i * i ;;, map(0..2, x => x))
# expect: [1, 4, 9] [0, 1]
```
```
# match.fung
fn describe(v) = match v:
  0 => "zero"
  n when n < 0 => "negative"
  1 | 2 => "small"
  "s" => "string"
  'ok => "ok symbol"
  Ok(x) => "ok ${x}"
  (x, 0) => "x-axis ${x}"
  [] => "empty"
  [first, ...] => "list from ${first}"
  {name: n} => "named ${n}"
  _ => "other"
;;
print(describe(0), describe(-5), describe(2), describe("s"), describe('ok))
# expect: zero negative small string ok symbol
print(describe(Ok(9)), describe((4, 0)), describe([]), describe([7, 8]), describe({name: "z"}), describe(3.5))
# expect: ok 9 x-axis 4 empty list from 7 named z other
fn len2(xs) = match xs:
  []           => 0
  [_, ...rest] => 1 + len2(rest)
;;
print(len2([1, 2, 3]))
# expect: 3
```
```
# destructure.fung
[first, ...rest] = [1, 2, 3]
{name: n} = {name: "a", x: 1}
Ok(text) = Ok("hi")
print(first, rest, n, text)
# expect: 1 [2, 3] a hi
```
```
# results.fung
fn parse(s) = s == "1" then Ok(1) else Err({kind: 'parse, message: "not a number: ${s}"})
fn twice(s) = Ok(parse(s)? * 2)
print(parse("1")!, parse("x") ?? 0, twice("1"), twice("x"), nil ?? "default")
# expect: 1 0 Ok(2) Err({kind: 'parse, message: "not a number: x"}) default
```
```
# pipes_partial.fung
import_free = [3, 1, 2]
fn double(x) = x * 2
print(import_free |> map(double) |> len, 5 |> double)
# expect: 3 10
fn join3(a, b, c) = a + b + c
wrap = join3("<", _, ">")
print(wrap("x"), map(["a", "b"], join3("[", _, "]")))
# expect: <x> ["[a]", "[b]"]
```
```
# rest_and_alias.fung
fn count(...xs) = len(xs)
fn head(x, ...rest) = (x, rest)
print(count(), count(1, 2), head(1), head(1, 2, 3))
# expect: 0 2 (1, []) (1, [2, 3])
type shape = Circle | Rect
fn area(s: shape) = match s: Circle(r) => r * r * 3; Rect(w, h) => w * h ;;
print(area(Circle(2)), area(Rect(2, 5)))
# expect: 12 10
```
에러 골든:

| 파일 | 내용 | expect |
|------|------|------|
| `err_index_range.fung` | `print([1][1])` | E0507 |
| `err_key.fung` | `print({a: 1}.b)` | E0508 |
| `err_index_tuple.fung` | `print((1, 2)[0])` | E0509 |
| `err_slice_tuple.fung` | `print((1,)[0..1])` | E0511 |
| `err_in.fung` | `print(1 in 1)` | E0512 |
| `err_no_match.fung` | `match 1: 0 => 1 ;;` | E0601 |
| `err_destructure.fung` | `(a, b) = (1, 2, 3)` | E0602 |
| `err_unhandled.fung` | `print(1)` + `# expect: 1` / `Err("boom")!` | E0603 |
| `err_bang_nil.fung` | `nil!` | E0604 |
| `err_bang_value.fung` | `5!` | E0605 |
| `err_return_in_for.fung` | `for x in [1]: return x ;;` | E0213 |
| `err_question_top.fung` | `Ok(1)?` | E0214 |
| `err_hole.fung` | `x = _ + 1` | E0215 |
| `err_alt_names.fung` | `match 1: Ok(x) \| Err(y) => 1 ;;` | E0216 |
| `err_alias_dup.fung` | `type a = int` / `type a = str` | E0217 |
| `err_alias_cycle.fung` | `type a = a` / `fn f(x: a) = x` | E0218 |
| `err_alias_in_fn.fung` | `fn f(): type a = int ;;` | E0219 |
| `err_arg_type_tag.fung` | `fn f(s: Circle) = s` / `f(1)` | E0403 |

- [ ] **Step 1: 골든 파일 작성 → 실패 확인 → 전부 통과 확인** (`go test ./...`, 골든 55 + 27 = 82).
- [ ] **Step 2: 노트** — 절 구성: (1) 한눈에: `match`가 `MATCH`·점프로 풀리는 바이트코드; (2) 값 표현(불변·단순 복사, 맵의 순서+색인, 태그의 타입 이름, 게으른 범위); (3) 패턴 엔진: 패턴 트리 상수와 `MATCH` 한 명령, 실패한 절의 슬롯 재사용이 안전한 이유, `|` 대안의 이름 검사, 괄호 없는 태그의 함정; (4) 구조 분해는 대입 규칙·`match` 절은 가리기 규칙인 이유; (5) 숨은 변수(`$m0` `$p0` `$f0`)와 desugar가 언어 핵심을 늘리지 않는 방식(pipe→call, partial→let+fn, for→map); (6) `!` `?` `??` 명령 셋과 `?`가 `RETURN` 경로를 공유하는 컴파일; (7) 인덱스·슬라이스의 엄격함(범위 밖 에러, `take`/`drop`은 M5), 룬 단위; (8) `in`의 대상 네 가지; (9) 타입 별칭 확장(끌어올림·순환); (10) 가변 인자 바인딩; (11) 테스트와 다음 M5.
- [ ] **Step 3: 문서** — 09의 M4 행 ` — 완료`; 03의 "구조 분해 대입" 절에 "묶는 규칙은 대입과 같다(같은 스코프 재대입 OK, 바깥 이름이면 E0205); `match` 절과 `for` 변수는 가리기" 추가; CLAUDE.md 상태(M4 완료, 다음 M5).
- [ ] **Step 4: 커밋** `feat(tests): add M4 golden tests` / `docs: add M4 learning notes and update status`.
