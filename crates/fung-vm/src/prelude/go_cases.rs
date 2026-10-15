// Generated from the Go binary (`/tmp/fung-go -e <src>`): src, stdin, stdout, stderr, exit code.
pub(super) const GO_CASES: &[(&str, &str, &str, &str, i32)] = &[
    (
        r#"print(map([1, 2, 3], x => x * 2), map(0..3, str), map([], x => x))"#,
        r#""#,
        r#"[2, 4, 6] ["0", "1", "2"] []
"#,
        r#""#,
        0,
    ),
    (
        r#"print(map(1, x => x))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot map over int
  --> -e:1:10
  |
1 | print(map(1, x => x))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(map([1], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:10
  |
1 | print(map([1], 1))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(map(1, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot map over int
  --> -e:1:10
  |
1 | print(map(1, 1))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(map([1, 2], x => x / 0))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:26
  |
1 | print(map([1, 2], x => x / 0))
  |                          ^
  |
 call path (innermost first):
   -e:1       <lambda>
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"fn f(x) = x / 0
print(map([1], f))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:13
  |
1 | fn f(x) = x / 0
  |             ^
  |
 call path (innermost first):
   -e:1       f
   -e:2       <entry>
"#,
        1,
    ),
    (
        r#"print(map([1, 2], (a, b) => a))"#,
        r#""#,
        r#""#,
        r#"error[E0407]: missing argument `b`
  --> -e:1:10
  |
1 | print(map([1, 2], (a, b) => a))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(fold(0..=4, 0, (a, x) => a + x), fold([], 7, (a, x) => a), fold(5..1, 0, (a, x) => a + 1))"#,
        r#""#,
        r#"10 7 0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(fold(1, 0, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot fold int
  --> -e:1:11
  |
1 | print(fold(1, 0, 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(fold([1], 0, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:11
  |
1 | print(fold([1], 0, 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"fn g(acc, x) = x == 3 then fail("three") else acc + x
print(fold([1, 2, 3], 0, g))"#,
        r#""#,
        r#""#,
        r#"error[E0803]: three
  --> -e:1:32
  |
1 | fn g(acc, x) = x == 3 then fail("three") else acc + x
  |                                ^
  |
 call path (innermost first):
   -e:1       g
   -e:2       <entry>
"#,
        1,
    ),
    (
        r#"print(fold([1, 2], 0, (a, x) => fold([x], a, (b, y) => b + y / 0)))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:62
  |
1 | print(fold([1, 2], 0, (a, x) => fold([x], a, (b, y) => b + y / 0)))
  |                                                              ^
  |
 call path (innermost first):
   -e:1       <lambda>
   -e:1       <lambda>
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(filter([1, 2, 3, 4], x => x > 2), filter(0..10, x => x % 3 == 0), filter([], x => 1))"#,
        r#""#,
        r#"[3, 4] [0, 3, 6, 9] []
"#,
        r#""#,
        0,
    ),
    (
        r#"print(filter([1], x => 1))"#,
        r#""#,
        r#""#,
        r#"error[E0505]: expected bool, got int
  --> -e:1:13
  |
1 | print(filter([1], x => 1))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(filter("ab", x => true))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot filter str
  --> -e:1:13
  |
1 | print(filter("ab", x => true))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(filter("ab", 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot filter str
  --> -e:1:13
  |
1 | print(filter("ab", 1))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(filter([1], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:13
  |
1 | print(filter([1], 1))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(each([1, 2], x => print(x)), each(0..0, x => fail("no")))"#,
        r#""#,
        r#"1
2
nil nil
"#,
        r#""#,
        0,
    ),
    (
        r#"print(each({a: 1}, x => x))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot iterate over map
  --> -e:1:11
  |
1 | print(each({a: 1}, x => x))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(each([1], nil))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got nil
  --> -e:1:11
  |
1 | print(each([1], nil))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort([3, 1, 2]), sort(["b", "a", "c"]), sort([]), sort(3..0), sort([2.5, 1, 3]), sort([[2], [1, 2], [1]]))"#,
        r#""#,
        r#"[1, 2, 3] ["a", "b", "c"] [] [] [1, 2.5, 3] [[1], [1, 2], [2]]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort([1, "a"]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare str and int
  --> -e:1:11
  |
1 | print(sort([1, "a"]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort([1, "a", nil, 2]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare str and int
  --> -e:1:11
  |
1 | print(sort([1, "a", nil, 2]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort(["x", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, nil]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare int and str
  --> -e:1:11
  |
1 | print(sort(["x", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, nil]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort(1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot sort int
  --> -e:1:11
  |
1 | print(sort(1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by(["ccc", "a", "bb"], len), sort_by([3, 1, 2], x => -x), sort_by([[1, "b"], [1, "a"], [0, "c"]], p => p[0]))"#,
        r#""#,
        r#"["a", "bb", "ccc"] [3, 2, 1] [[0, "c"], [1, "b"], [1, "a"]]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort_by(1, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot sort int
  --> -e:1:14
  |
1 | print(sort_by(1, 1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by([1], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:14
  |
1 | print(sort_by([1], 1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by([1, 2], x => x == 1 then "a" else 1))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare int and str
  --> -e:1:14
  |
1 | print(sort_by([1, 2], x => x == 1 then "a" else 1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by([1, 2], x => x / 0))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:30
  |
1 | print(sort_by([1, 2], x => x / 0))
  |                              ^
  |
 call path (innermost first):
   -e:1       <lambda>
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(find([1, 2, 3], x => x > 1), find([], x => true), find(0..10, x => x * x > 10))"#,
        r#""#,
        r#"2 nil 4
"#,
        r#""#,
        0,
    ),
    (
        r#"print(find([1], x => 1))"#,
        r#""#,
        r#""#,
        r#"error[E0505]: expected bool, got int
  --> -e:1:11
  |
1 | print(find([1], x => 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(find(1, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot search int
  --> -e:1:11
  |
1 | print(find(1, 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(find([1], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:11
  |
1 | print(find([1], 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(any([1, 2], x => x > 1), any([], x => true), all([], x => false), all([1, 2], x => x > 0), all([1, 2], x => x > 1))"#,
        r#""#,
        r#"true false true true false
"#,
        r#""#,
        0,
    ),
    (
        r#"print(any(1, x => x))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot test int
  --> -e:1:10
  |
1 | print(any(1, x => x))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(all([1], x => nil))"#,
        r#""#,
        r#""#,
        r#"error[E0505]: expected bool, got nil
  --> -e:1:10
  |
1 | print(all([1], x => nil))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(any([1], "f"))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got str
  --> -e:1:10
  |
1 | print(any([1], "f"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(flatten([[1], 2, [], [[3]]]), flatten(0..3), flatten([]))"#,
        r#""#,
        r#"[1, 2, [3]] [0, 1, 2] []
"#,
        r#""#,
        0,
    ),
    (
        r#"print(flatten(1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot flatten int
  --> -e:1:14
  |
1 | print(flatten(1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(group_by(["ab", "c", "ad", "ce"], s => s[..1]))"#,
        r#""#,
        r#"{a: ["ab", "ad"], c: ["c", "ce"]}
"#,
        r#""#,
        0,
    ),
    (
        r#"print(group_by([1], x => x))"#,
        r#""#,
        r#""#,
        r#"error[E0514]: cannot use a value of type int as a map key
  --> -e:1:15
  |
1 | print(group_by([1], x => x))
  |               ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(group_by([1], x => nil))"#,
        r#""#,
        r#""#,
        r#"error[E0514]: cannot use a value of type nil as a map key
  --> -e:1:15
  |
1 | print(group_by([1], x => nil))
  |               ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(group_by(1, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot group int
  --> -e:1:15
  |
1 | print(group_by(1, 1))
  |               ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(group_by([1], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:15
  |
1 | print(group_by([1], 1))
  |               ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(pairs({b: 1, a: 2}), pairs({}))"#,
        r#""#,
        r#"[("b", 1), ("a", 2)] []
"#,
        r#""#,
        0,
    ),
    (
        r#"print(pairs([1]))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot list the pairs of list
  --> -e:1:12
  |
1 | print(pairs([1]))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"m = {a: 1}
print(put(m, "b", 2), put(m, "a", 3), m)"#,
        r#""#,
        r#"{a: 1, b: 2} {a: 3} {a: 1}
"#,
        r#""#,
        0,
    ),
    (
        r#"print(put(1, "a", 2))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot put into int
  --> -e:1:10
  |
1 | print(put(1, "a", 2))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(put({}, 1, 2))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot index map and int
  --> -e:1:10
  |
1 | print(put({}, 1, 2))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(put({}, nil, 2))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot index map and nil
  --> -e:1:10
  |
1 | print(put({}, nil, 2))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(merge({a: 1, b: 1}, {b: 2, c: 3}), merge({}, {}))"#,
        r#""#,
        r#"{a: 1, b: 2, c: 3} {}
"#,
        r#""#,
        0,
    ),
    (
        r#"print(merge({}, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot merge int
  --> -e:1:12
  |
1 | print(merge({}, 1))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(merge(1, {}))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot merge int
  --> -e:1:12
  |
1 | print(merge(1, {}))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(remove({a: 1, b: 2}, "a"), remove({}, "x"))"#,
        r#""#,
        r#"{b: 2} {}
"#,
        r#""#,
        0,
    ),
    (
        r#"print(remove({}, 1))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot index map and int
  --> -e:1:13
  |
1 | print(remove({}, 1))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(remove([], "a"))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot remove from list
  --> -e:1:13
  |
1 | print(remove([], "a"))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(chars("참치"), chars(""), chars("áb"))"#,
        r#""#,
        r#"["참", "치"] [] ["a", "́", "b"]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(chars(1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot list the characters of int
  --> -e:1:12
  |
1 | print(chars(1))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(split("a,b,,c", ","), split("참치", ""), split("", ","), split("", ""), split("abc", "abc"))"#,
        r#""#,
        r#"["a", "b", "", "c"] ["참", "치"] [""] [] ["", ""]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(split(1, ","))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:12
  |
1 | print(split(1, ","))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(split("a", 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `sep` must be str, got int
  --> -e:1:12
  |
1 | print(split("a", 1))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(join(["a", 1, 2.0, nil, 'x, [1, "b"]], "-"), join([]), join(0..3), join(["a", "b"]))"#,
        r#""#,
        r#"a-1-2.0-nil-'x-[1, "b"]  012 ab
"#,
        r#""#,
        0,
    ),
    (
        r#"print(join(1, 2))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot join int
  --> -e:1:11
  |
1 | print(join(1, 2))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(join(["a"], 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `sep` must be str, got int
  --> -e:1:11
  |
1 | print(join(["a"], 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(join(["a"], sep: "+"))"#,
        r#""#,
        r#"a
"#,
        r#""#,
        0,
    ),
    (
        r#"print(trim("  a \t"), trim("xxaxx", "x"), trim("abcba", "ab"), trim(" a　"), trim("aa", ""))"#,
        r#""#,
        r#"a a c a aa
"#,
        r#""#,
        0,
    ),
    (
        r#"print(trim(1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:11
  |
1 | print(trim(1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(trim("a", 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `cut` must be str, got int
  --> -e:1:11
  |
1 | print(trim("a", 1))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(upper("aéß ﬃ ǅ"), lower("AÉ İ Σ ǅ"), upper("ᾳ ᾀ"), lower("ẞ"))"#,
        r#""#,
        r#"AÉß ﬃ Ǆ aé i σ ǆ ᾼ ᾈ ß
"#,
        r#""#,
        0,
    ),
    (
        r#"print(upper(1), lower(1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:12
  |
1 | print(upper(1), lower(1))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(lower(nil))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got nil
  --> -e:1:12
  |
1 | print(lower(nil))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(replace("a-b-c", "-", "+"), replace("ab", "", "-"), replace("", "", "x"), replace("aaa", "aa", "b"))"#,
        r#""#,
        r#"a+b+c -a-b- x ba
"#,
        r#""#,
        0,
    ),
    (
        r#"print(replace("a", "b", 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `new` must be str, got int
  --> -e:1:14
  |
1 | print(replace("a", "b", 1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(replace(1, 2, 3))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:14
  |
1 | print(replace(1, 2, 3))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(starts_with("abc", "ab"), starts_with("참치", "참"), ends_with("abc", "bc"), ends_with("a", ""), starts_with("", "a"))"#,
        r#""#,
        r#"true true true true false
"#,
        r#""#,
        0,
    ),
    (
        r#"print(starts_with(1, "a"))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:18
  |
1 | print(starts_with(1, "a"))
  |                  ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(ends_with("a", 1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `suffix` must be str, got int
  --> -e:1:16
  |
1 | print(ends_with("a", 1))
  |                ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(repeat("ab", 3), repeat("참", 2), repeat("x", 0), repeat("x", -5), repeat("", 1000000000000))"#,
        r#""#,
        r#"ababab 참참   
"#,
        r#""#,
        0,
    ),
    (
        r#"print(repeat("ab", 50000001))"#,
        r#""#,
        r#""#,
        r#"error[E0806]: too many characters (limit 100000000)
  --> -e:1:13
  |
1 | print(repeat("ab", 50000001))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(repeat("a", 100000000) == "")"#,
        r#""#,
        r#"false
"#,
        r#""#,
        0,
    ),
    (
        r#"print(repeat(1, 2))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:13
  |
1 | print(repeat(1, 2))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(repeat("a", "2"))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `n` must be int, got str
  --> -e:1:13
  |
1 | print(repeat("a", "2"))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(repeat("a", 2.0))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `n` must be int, got float
  --> -e:1:13
  |
1 | print(repeat("a", 2.0))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(3), int(3.9), int(-3.9), int("42"), int("-1_000"), int("+7"))"#,
        r#""#,
        r#"3 3 -3 42 -1000 7
"#,
        r#""#,
        0,
    ),
    (
        r#"print(int("x"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "x" to int
  --> -e:1:10
  |
1 | print(int("x"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(" 1"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert " 1" to int
  --> -e:1:10
  |
1 | print(int(" 1"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(true))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert true to int
  --> -e:1:10
  |
1 | print(int(true))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(9.3e18))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert 9300000000000000000.0 to int
  --> -e:1:10
  |
1 | print(int(9.3e18))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(-9.223372036854775808e18))"#,
        r#""#,
        r#"-9223372036854775808
"#,
        r#""#,
        0,
    ),
    (
        r#"print(int("9223372036854775808"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "9223372036854775808" to int
  --> -e:1:10
  |
1 | print(int("9223372036854775808"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int([1]))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert [1] to int
  --> -e:1:10
  |
1 | print(int([1]))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int("1__0"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "1__0" to int
  --> -e:1:10
  |
1 | print(int("1__0"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(float(2), float("2.5"), float("1e3"), float(1.5), float("1_0.5"), float("-.5"))"#,
        r#""#,
        r#"2.0 2.5 1000.0 1.5 10.5 -0.5
"#,
        r#""#,
        0,
    ),
    (
        r#"print(float("abc"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "abc" to float
  --> -e:1:12
  |
1 | print(float("abc"))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(float("inf"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "inf" to float
  --> -e:1:12
  |
1 | print(float("inf"))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(float(nil))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert nil to float
  --> -e:1:12
  |
1 | print(float(nil))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(str("a"), str(1), str(1.0), str('ok), str([1, "a"]), str({a: "b"}), str(nil), str(1s), str((1, "x")), str(Ok("x")))"#,
        r#""#,
        r#"a 1 1.0 'ok [1, "a"] {a: "b"} nil 1s (1, "x") Ok("x")
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sym("ok"), sym('x), sym("참치"), sym("_x"), sym("a1"))"#,
        r#""#,
        r#"'ok 'x '참치 '_x 'a1
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sym("Ab"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "Ab" to sym
  --> -e:1:10
  |
1 | print(sym("Ab"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sym("a b"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "a b" to sym
  --> -e:1:10
  |
1 | print(sym("a b"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sym(""))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "" to sym
  --> -e:1:10
  |
1 | print(sym(""))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sym(1))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert 1 to sym
  --> -e:1:10
  |
1 | print(sym(1))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(parse_int("12"), parse_int("x"), parse_int(" 1"), parse_int("1_000"), parse_int("99999999999999999999"))"#,
        r#""#,
        r#"Ok(12) Err({kind: 'parse, message: "not an integer: x"}) Err({kind: 'parse, message: "not an integer:  1"}) Ok(1000) Err({kind: 'parse, message: "not an integer: 99999999999999999999"})
"#,
        r#""#,
        0,
    ),
    (
        r#"print(parse_int(1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:16
  |
1 | print(parse_int(1))
  |                ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(parse_float("1.5"), parse_float("x"), parse_float("1e400"), parse_float("nan"))"#,
        r#""#,
        r#"Ok(1.5) Err({kind: 'parse, message: "not a number: x"}) Err({kind: 'parse, message: "not a number: 1e400"}) Err({kind: 'parse, message: "not a number: nan"})
"#,
        r#""#,
        0,
    ),
    (
        r#"print(parse_float(nil))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got nil
  --> -e:1:18
  |
1 | print(parse_float(nil))
  |                  ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(parse_dur("1h30m"), parse_dur(" 500ms "), parse_dur("-5s"), parse_dur("30"), parse_dur("1h x"), parse_dur("30m1h"), parse_dur(""), parse_dur("1.5s"), parse_dur("- 5s"))"#,
        r#""#,
        r#"Ok(1h30m) Ok(500ms) Ok(-5s) Err({kind: 'parse, message: "not a duration: 30"}) Err({kind: 'parse, message: "not a duration: 1h x"}) Err({kind: 'parse, message: "not a duration: 30m1h"}) Err({kind: 'parse, message: "not a duration: "}) Ok(1s500ms) Ok(-5s)
"#,
        r#""#,
        0,
    ),
    (
        r#"print(parse_dur(5))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `s` must be str, got int
  --> -e:1:16
  |
1 | print(parse_dur(5))
  |                ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(len("참치"), len([1, 2]), len({a: 1}), len((1, 2)), len(0..10), len(5..1), len(""))"#,
        r#""#,
        r#"2 2 1 2 10 0 0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(len(1))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot take the length of int
  --> -e:1:10
  |
1 | print(len(1))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(len(nil))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot take the length of nil
  --> -e:1:10
  |
1 | print(len(nil))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(type_of(1), type_of("a"), type_of(nil), type_of(Ok(1)), type_of(x => x), type_of(print), type_of(1s), type_of(0..1), type_of((1,)), type_of('a), type_of({}), type_of([]), type_of(1.0), type_of(true))"#,
        r#""#,
        r#"'int 'str 'nil 'Ok 'fn 'fn 'dur 'range 'tuple 'sym 'map 'list 'float 'bool
"#,
        r#""#,
        0,
    ),
    (
        r#"print()"#,
        r#""#,
        r#"
"#,
        r#""#,
        0,
    ),
    (
        r#"print(1, "a", [1, "a"], nil)"#,
        r#""#,
        r#"1 a [1, "a"] nil
"#,
        r#""#,
        0,
    ),
    (
        r#"eprint("err", 1)
print("out")"#,
        r#""#,
        r#"out
"#,
        r#"err 1
"#,
        0,
    ),
    (
        r#"fail("port", 1)"#,
        r#""#,
        r#""#,
        r#"error[E0803]: port 1
  --> -e:1:5
  |
1 | fail("port", 1)
  |     ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"fail()"#,
        r#""#,
        r#""#,
        r#"error[E0803]: fail
  --> -e:1:5
  |
1 | fail()
  |     ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"fail([1, "a"], 'x)"#,
        r#""#,
        r#""#,
        r#"error[E0803]: [1, "a"] 'x
  --> -e:1:5
  |
1 | fail([1, "a"], 'x)
  |     ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(at_exit(1))"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `f` must be fn, got int
  --> -e:1:14
  |
1 | print(at_exit(1))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"at_exit(() => print("bye"))
print("hi")"#,
        r#""#,
        r#"hi
bye
"#,
        r#""#,
        0,
    ),
    (r#"exit(3)"#, r#""#, r#""#, r#""#, 3),
    (r#"exit()"#, r#""#, r#""#, r#""#, 0),
    (
        r#"exit("a")"#,
        r#""#,
        r#""#,
        r#"error[E0403]: argument `code` must be int, got str
  --> -e:1:5
  |
1 | exit("a")
  |     ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (r#"exit(code: 4)"#, r#""#, r#""#, r#""#, 4),
    (
        r#"print(read_line(), read_lines())"#,
        r#"line1
line2
last"#,
        r#"Ok("line1") Ok(["line2", "last"])
"#,
        r#""#,
        0,
    ),
    (
        r#"print(read_line(1))"#,
        r#"line1
line2
last"#,
        r#""#,
        r#"error[E0403]: argument `f` must be file | nil, got int
  --> -e:1:16
  |
1 | print(read_line(1))
  |                ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(read_lines("x"))"#,
        r#"line1
line2
last"#,
        r#""#,
        r#"error[E0403]: argument `f` must be file | nil, got str
  --> -e:1:17
  |
1 | print(read_lines("x"))
  |                 ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(dbg(1 + 2))"#,
        r#""#,
        r#"3
"#,
        r#"[-e:1] 1 + 2 = 3
"#,
        0,
    ),
    (
        r#"x = 5
dbg(x * 2)"#,
        r#""#,
        r#""#,
        r#"[-e:2] x * 2 = 10
"#,
        0,
    ),
    (
        r#"print(dbg(1, "t"), dbg(2, "t", "w"), dbg(3, where: "w"))"#,
        r#""#,
        r#"1 2 3
"#,
        r#"t = 1
[w] t = 2
dbg = 3
"#,
        0,
    ),
    (
        r#"print(args)"#,
        r#""#,
        r#"[]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(len(1, 2))"#,
        r#""#,
        r#""#,
        r#"error[E0402]: len takes 1 argument(s), got 2
  --> -e:1:10
  |
1 | print(len(1, 2))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(map(xs: [1], f: x => x + 1))"#,
        r#""#,
        r#"[2]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(map([1], g: 1))"#,
        r#""#,
        r#""#,
        r#"error[E0406]: map has no argument named `g`
  --> -e:1:10
  |
1 | print(map([1], g: 1))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(fold([1, 2], 0))"#,
        r#""#,
        r#""#,
        r#"error[E0407]: missing argument `f`
  --> -e:1:11
  |
1 | print(fold([1, 2], 0))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(join(["a"], "-", "x"))"#,
        r#""#,
        r#""#,
        r#"error[E0402]: join takes 2 argument(s), got 3
  --> -e:1:11
  |
1 | print(join(["a"], "-", "x"))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(map, fold, print)"#,
        r#""#,
        r#"<fn map> <fn fold> <fn print>
"#,
        r#""#,
        0,
    ),
    (
        r#"fn h(x) = x + 1
print(map([1, 2], h), filter([1, 2], (x) => x > 1), sort_by([2, 1], h))"#,
        r#""#,
        r#"[2, 3] [2] [1, 2]
"#,
        r#""#,
        0,
    ),
    (
        r#"fn inner(x) = x / 0
fn outer(xs) = map(xs, inner)
print(outer([1]))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:17
  |
1 | fn inner(x) = x / 0
  |                 ^
  |
 call path (innermost first):
   -e:1       inner
   -e:2       outer
   -e:3       <entry>
"#,
        1,
    ),
    (
        r#"fn k(x) = x / 0
fn m2(xs) = fold(xs, 0, (a, x) => a + k(x))
print(m2([1]))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:13
  |
1 | fn k(x) = x / 0
  |             ^
  |
 call path (innermost first):
   -e:1       k
   -e:2       <lambda>
   -e:2       m2
   -e:3       <entry>
"#,
        1,
    ),
    (
        r#"fn bad(x) = sort_by(x, y => y / 0)
print(bad([1, 2]))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:31
  |
1 | fn bad(x) = sort_by(x, y => y / 0)
  |                               ^
  |
 call path (innermost first):
   -e:1       <lambda>
   -e:1       bad
   -e:2       <entry>
"#,
        1,
    ),
    (
        r#"print(map([1, 2], x => exit(x + 5)))"#,
        r#""#,
        r#""#,
        r#""#,
        6,
    ),
    (
        r#"print(map([[1, 2], [3, 4]], ([a, b]) => a + b))"#,
        r#""#,
        r#"[3, 7]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(fold([[1, 2], [3, 4]], 0, (acc, [a, b]) => acc + a * b))"#,
        r#""#,
        r#"14
"#,
        r#""#,
        0,
    ),
    (
        r#"print(each([1], print), map(["a"], upper), filter(["", "a"], x => len(x) > 0))"#,
        r#""#,
        r#"1
nil ["A"] ["a"]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort_by([3, 1], str), find([1], x => x > 0), any(0..3, x => x == 2))"#,
        r#""#,
        r#"[1, 3] 1 true
"#,
        r#""#,
        0,
    ),
    (
        r#"print(map(0..=2, x => x), filter(0..=2, x => true), fold(-2..=2, 0, (a, x) => a + x))"#,
        r#""#,
        r#"[0, 1, 2] [0, 1, 2] 0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort([1, 1.0, 0.5]), sort([true, false]), sort(['b, 'a]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare bool and bool
  --> -e:1:32
  |
1 | print(sort([1, 1.0, 0.5]), sort([true, false]), sort(['b, 'a]))
  |                                ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort([nil, nil]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare nil and nil
  --> -e:1:11
  |
1 | print(sort([nil, nil]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort([(1, 2), (1, 1)]))"#,
        r#""#,
        r#"[(1, 1), (1, 2)]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort([{a: 1}, {a: 0}]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare map and map
  --> -e:1:11
  |
1 | print(sort([{a: 1}, {a: 0}]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(1s))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert 1s to int
  --> -e:1:10
  |
1 | print(int(1s))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(str(-0.0), str(1e21), str(1.5e-7), str(100000000000000000000.0))"#,
        r#""#,
        r#"-0.0 1e21 1.5e-07 100000000000000000000.0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(put({a: 1}, "a", 2) == {a: 2}, merge({a: 1}, {a: 1}) == {a: 1})"#,
        r#""#,
        r#"true true
"#,
        r#""#,
        0,
    ),
    (
        r#"print(trim("  "), trim(""), split("a b", " "), upper(""), join([], "x"))"#,
        r#""#,
        r#"  ["a", "b"]  
"#,
        r#""#,
        0,
    ),
    (
        r#"print(trim("aXbXa", "aX"))"#,
        r#""#,
        r#"b
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort([3, 2, 1, 0, -1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21, -22, -23, -24, -25, "x", 0.5, nil]))"#,
        r#""#,
        r#""#,
        r#"error[E0501]: cannot compare str and int
  --> -e:1:11
  |
1 | print(sort([3, 2, 1, 0, -1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21, -22, -23, -24, -25, "x", 0.5, nil]))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by([5, 3, 9, 1, 7, 2, 8, 4, 6, 0, 15, 13, 19, 11, 17, 12, 18, 14, 16, 10, 25, 23, 29, 21, 27, 22, 28, 24, 26, 20, 31, 30], x => x % 7))"#,
        r#""#,
        r#"[7, 0, 14, 21, 28, 1, 8, 15, 29, 22, 9, 2, 16, 23, 30, 3, 17, 10, 24, 31, 4, 11, 18, 25, 5, 19, 12, 26, 6, 13, 27, 20]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort_by(0..50, x => x % 3 == 0 then -x else x))"#,
        r#""#,
        r#"[48, 45, 42, 39, 36, 33, 30, 27, 24, 21, 18, 15, 12, 9, 6, 3, 0, 1, 2, 4, 5, 7, 8, 10, 11, 13, 14, 16, 17, 19, 20, 22, 23, 25, 26, 28, 29, 31, 32, 34, 35, 37, 38, 40, 41, 43, 44, 46, 47, 49]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(map([1, 2, 3], x => [x, x * 2]) |> flatten |> sort)"#,
        r#""#,
        r#"[1, 2, 2, 3, 4, 6]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(fold(0..100000, 0, (a, x) => a + x), len(map(0..100000, x => x)), len(filter(0..100000, x => x % 2 == 0)))"#,
        r#""#,
        r#"4999950000 100000 50000
"#,
        r#""#,
        0,
    ),
    (
        r#"print(trim(" a　"), trim("\t a \r "))"#,
        r#""#,
        r#"a a
"#,
        r#""#,
        0,
    ),
    (
        r#"print(upper("ŉ ǰ ﬀ ı ſ"), lower("ΣΑΣ Ǆ K Å"))"#,
        r#""#,
        r#"ŉ ǰ ﬀ I S σασ ǆ k å
"#,
        r#""#,
        0,
    ),
    (
        r#"print(int(9223372036854775807.0))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert 9223372036854776000.0 to int
  --> -e:1:10
  |
1 | print(int(9223372036854775807.0))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int(-9223372036854775809.0))"#,
        r#""#,
        r#"-9223372036854775808
"#,
        r#""#,
        0,
    ),
    (
        r#"print(float("0x1p4"), float("0x1.8p1"), float("4.9e-324"), float("1E5"), float("00.5"), float("5."))"#,
        r#""#,
        r#"16.0 3.0 5e-324 100000.0 0.5 5.0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(float("0x10"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "0x10" to float
  --> -e:1:12
  |
1 | print(float("0x10"))
  |            ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(int("٣"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "٣" to int
  --> -e:1:10
  |
1 | print(int("٣"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sym("Ⅰx"))"#,
        r#""#,
        r#""#,
        r#"error[E0513]: cannot convert "Ⅰx" to sym
  --> -e:1:10
  |
1 | print(sym("Ⅰx"))
  |          ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sym("a٣"))"#,
        r#""#,
        r#"'a٣
"#,
        r#""#,
        0,
    ),
    (
        r#"print(read_lines(), read_line())"#,
        r#"line1
line2
last"#,
        r#"Ok(["line1", "line2", "last"]) Ok(nil)
"#,
        r#""#,
        0,
    ),
    (
        r#"at_exit(print)
print("x")"#,
        r#""#,
        r#"x

"#,
        r#""#,
        0,
    ),
    (
        r#"print(map([1, 2], print))"#,
        r#""#,
        r#"1
2
[nil, nil]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(fold(["a"], "", join))"#,
        r#""#,
        r#""#,
        r#"error[E0506]: cannot join str
  --> -e:1:11
  |
1 | print(fold(["a"], "", join))
  |           ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(sort_by([1], fail))"#,
        r#""#,
        r#""#,
        r#"error[E0803]: 1
  --> -e:1:14
  |
1 | print(sort_by([1], fail))
  |              ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (r#"print(map([3], exit))"#, r#""#, r#""#, r#""#, 3),
    (
        r#"print(filter([1], type_of))"#,
        r#""#,
        r#""#,
        r#"error[E0505]: expected bool, got sym
  --> -e:1:13
  |
1 | print(filter([1], type_of))
  |             ^
  |
 call path (innermost first):
   -e:1       <entry>
"#,
        1,
    ),
    (
        r#"print(each(0..3, x => x == 1 then exit(9) else print(x)))"#,
        r#""#,
        r#"0
"#,
        r#""#,
        9,
    ),
    (
        r#"fn a(xs) = map(xs, x => fold([x], 0, (s, y) => s + y / 0))
fn b() = a([1])
print(b())"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:54
  |
1 | fn a(xs) = map(xs, x => fold([x], 0, (s, y) => s + y / 0))
  |                                                      ^
  |
 call path (innermost first):
   -e:1       <lambda>
   -e:1       <lambda>
   -e:1       a (1 tail calls elided)
   -e:3       <entry>
"#,
        1,
    ),
    (
        r#"fn g(x) = x / 0
print(map([1], x => g(x)))"#,
        r#""#,
        r#""#,
        r#"error[E0502]: division by zero
  --> -e:1:13
  |
1 | fn g(x) = x / 0
  |             ^
  |
 call path (innermost first):
   -e:1       g (1 tail calls elided)
   -e:2       <entry>
"#,
        1,
    ),
    (
        r#"print(len(map(0..200000, x => x)))"#,
        r#""#,
        r#"200000
"#,
        r#""#,
        0,
    ),
    (
        r#"fn d(n) = n == 0 then 0 else 1 + d(n - 1)
print(map([200000], d))"#,
        r#""#,
        r#""#,
        r#"error[E0804]: stack overflow: call depth exceeded 100000
  --> -e:1:35
  |
1 | fn d(n) = n == 0 then 0 else 1 + d(n - 1)
  |                                   ^
  |
 call path (innermost first):
   -e:1       d
   -e:1       d
   -e:1       d
   -e:1       d
   -e:1       d
   -e:1       d
   -e:1       d
   -e:1       d
   … 99990 more
   -e:1       d
   -e:2       <entry>
 help: make the recursion a tail call or use `fold`
"#,
        1,
    ),
    (
        r#"fn r(n) = n == 0 then 0 else fold([n], 0, (a, x) => r(x - 1))
print(r(60000))"#,
        r#""#,
        r#"0
"#,
        r#""#,
        0,
    ),
    (
        r#"fn r(n) = n == 0 then 0 else fold([n], 0, (a, x) => r(x - 1))
print(r(30000))"#,
        r#""#,
        r#"0
"#,
        r#""#,
        0,
    ),
    (
        r#"print(group_by(0..6, x => x % 2 == 0 then "even" else "odd"), pairs(group_by([], x => "a")))"#,
        r#""#,
        r#"{even: [0, 2, 4], odd: [1, 3, 5]} []
"#,
        r#""#,
        0,
    ),
    (
        r#"print(sort_by(["b", "B", "a"], lower), sort(["b", "B", "a", "é", "e"]))"#,
        r#""#,
        r#"["a", "b", "B"] ["B", "a", "b", "e", "é"]
"#,
        r#""#,
        0,
    ),
    (
        r#"print(chars("👍🏽x"), len("👍🏽x"), upper("straße"), split("a👍b", "👍"))"#,
        r#""#,
        r#"["👍", "🏽", "x"] 3 STRAßE ["a", "b"]
"#,
        r#""#,
        0,
    ),
];
