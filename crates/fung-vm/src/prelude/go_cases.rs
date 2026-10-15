//! Programs run through both the Go binary and the Rust VM by
//! `runs_match_the_go_binary`: (source, standard input).

/// Standard input for the programs that read it.
const INPUT: &str = "line1\r\nline2\nlast";

pub(super) const GO_CASES: &[(&str, &str)] = &[
    (
        r#"print(map([1, 2, 3], x => x * 2), map(0..3, str), map([], x => x))"#,
        "",
    ),
    (r#"print(map(1, x => x))"#, ""),
    (r#"print(map([1], 1))"#, ""),
    (r#"print(map(1, 1))"#, ""),
    (r#"print(map([1, 2], x => x / 0))"#, ""),
    (
        r#"fn f(x) = x / 0
print(map([1], f))"#,
        "",
    ),
    (r#"print(map([1, 2], (a, b) => a))"#, ""),
    (
        r#"print(fold(0..=4, 0, (a, x) => a + x), fold([], 7, (a, x) => a), fold(5..1, 0, (a, x) => a + 1))"#,
        "",
    ),
    (r#"print(fold(1, 0, 1))"#, ""),
    (r#"print(fold([1], 0, 1))"#, ""),
    (
        r#"fn g(acc, x) = x == 3 then fail("three") else acc + x
print(fold([1, 2, 3], 0, g))"#,
        "",
    ),
    (
        r#"print(fold([1, 2], 0, (a, x) => fold([x], a, (b, y) => b + y / 0)))"#,
        "",
    ),
    (
        r#"print(filter([1, 2, 3, 4], x => x > 2), filter(0..10, x => x % 3 == 0), filter([], x => 1))"#,
        "",
    ),
    (r#"print(filter([1], x => 1))"#, ""),
    (r#"print(filter("ab", x => true))"#, ""),
    (r#"print(filter("ab", 1))"#, ""),
    (r#"print(filter([1], 1))"#, ""),
    (
        r#"print(each([1, 2], x => print(x)), each(0..0, x => fail("no")))"#,
        "",
    ),
    (r#"print(each({a: 1}, x => x))"#, ""),
    (r#"print(each([1], nil))"#, ""),
    (
        r#"print(sort([3, 1, 2]), sort(["b", "a", "c"]), sort([]), sort(3..0), sort([2.5, 1, 3]), sort([[2], [1, 2], [1]]))"#,
        "",
    ),
    (r#"print(sort([1, "a"]))"#, ""),
    (r#"print(sort([1, "a", nil, 2]))"#, ""),
    (
        r#"print(sort(["x", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, nil]))"#,
        "",
    ),
    (r#"print(sort(1))"#, ""),
    (
        r#"print(sort_by(["ccc", "a", "bb"], len), sort_by([3, 1, 2], x => -x), sort_by([[1, "b"], [1, "a"], [0, "c"]], p => p[0]))"#,
        "",
    ),
    (r#"print(sort_by(1, 1))"#, ""),
    (r#"print(sort_by([1], 1))"#, ""),
    (r#"print(sort_by([1, 2], x => x == 1 then "a" else 1))"#, ""),
    (r#"print(sort_by([1, 2], x => x / 0))"#, ""),
    (
        r#"print(find([1, 2, 3], x => x > 1), find([], x => true), find(0..10, x => x * x > 10))"#,
        "",
    ),
    (r#"print(find([1], x => 1))"#, ""),
    (r#"print(find(1, 1))"#, ""),
    (r#"print(find([1], 1))"#, ""),
    (
        r#"print(any([1, 2], x => x > 1), any([], x => true), all([], x => false), all([1, 2], x => x > 0), all([1, 2], x => x > 1))"#,
        "",
    ),
    (r#"print(any(1, x => x))"#, ""),
    (r#"print(all([1], x => nil))"#, ""),
    (r#"print(any([1], "f"))"#, ""),
    (
        r#"print(flatten([[1], 2, [], [[3]]]), flatten(0..3), flatten([]))"#,
        "",
    ),
    (r#"print(flatten(1))"#, ""),
    (
        r#"print(group_by(["ab", "c", "ad", "ce"], s => s[..1]))"#,
        "",
    ),
    (r#"print(group_by([1], x => x))"#, ""),
    (r#"print(group_by([1], x => nil))"#, ""),
    (r#"print(group_by(1, 1))"#, ""),
    (r#"print(group_by([1], 1))"#, ""),
    (r#"print(pairs({b: 1, a: 2}), pairs({}))"#, ""),
    (r#"print(pairs([1]))"#, ""),
    (
        r#"m = {a: 1}
print(put(m, "b", 2), put(m, "a", 3), m)"#,
        "",
    ),
    (r#"print(put(1, "a", 2))"#, ""),
    (r#"print(put({}, 1, 2))"#, ""),
    (r#"print(put({}, nil, 2))"#, ""),
    (
        r#"print(merge({a: 1, b: 1}, {b: 2, c: 3}), merge({}, {}))"#,
        "",
    ),
    (r#"print(merge({}, 1))"#, ""),
    (r#"print(merge(1, {}))"#, ""),
    (r#"print(remove({a: 1, b: 2}, "a"), remove({}, "x"))"#, ""),
    (r#"print(remove({}, 1))"#, ""),
    (r#"print(remove([], "a"))"#, ""),
    (r#"print(chars("참치"), chars(""), chars("áb"))"#, ""),
    (r#"print(chars(1))"#, ""),
    (
        r#"print(split("a,b,,c", ","), split("참치", ""), split("", ","), split("", ""), split("abc", "abc"))"#,
        "",
    ),
    (r#"print(split(1, ","))"#, ""),
    (r#"print(split("a", 1))"#, ""),
    (
        r#"print(join(["a", 1, 2.0, nil, 'x, [1, "b"]], "-"), join([]), join(0..3), join(["a", "b"]))"#,
        "",
    ),
    (r#"print(join(1, 2))"#, ""),
    (r#"print(join(["a"], 1))"#, ""),
    (r#"print(join(["a"], sep: "+"))"#, ""),
    (
        r#"print(trim("  a \t"), trim("xxaxx", "x"), trim("abcba", "ab"), trim(" a　"), trim("aa", ""))"#,
        "",
    ),
    (r#"print(trim(1))"#, ""),
    (r#"print(trim("a", 1))"#, ""),
    (
        r#"print(upper("aéß ﬃ ǅ"), lower("AÉ İ Σ ǅ"), upper("ᾳ ᾀ"), lower("ẞ"))"#,
        "",
    ),
    (r#"print(upper(1), lower(1))"#, ""),
    (r#"print(lower(nil))"#, ""),
    (
        r#"print(replace("a-b-c", "-", "+"), replace("ab", "", "-"), replace("", "", "x"), replace("aaa", "aa", "b"))"#,
        "",
    ),
    (r#"print(replace("a", "b", 1))"#, ""),
    (r#"print(replace(1, 2, 3))"#, ""),
    (
        r#"print(starts_with("abc", "ab"), starts_with("참치", "참"), ends_with("abc", "bc"), ends_with("a", ""), starts_with("", "a"))"#,
        "",
    ),
    (r#"print(starts_with(1, "a"))"#, ""),
    (r#"print(ends_with("a", 1))"#, ""),
    (
        r#"print(repeat("ab", 3), repeat("참", 2), repeat("x", 0), repeat("x", -5), repeat("", 1000000000000))"#,
        "",
    ),
    (r#"print(repeat("ab", 50000001))"#, ""),
    (r#"print(repeat("a", 100000000) == "")"#, ""),
    (r#"print(repeat(1, 2))"#, ""),
    (r#"print(repeat("a", "2"))"#, ""),
    (r#"print(repeat("a", 2.0))"#, ""),
    (
        r#"print(int(3), int(3.9), int(-3.9), int("42"), int("-1_000"), int("+7"))"#,
        "",
    ),
    (r#"print(int("x"))"#, ""),
    (r#"print(int(" 1"))"#, ""),
    (r#"print(int(true))"#, ""),
    (r#"print(int(9.3e18))"#, ""),
    (r#"print(int(-9.223372036854775808e18))"#, ""),
    (r#"print(int("9223372036854775808"))"#, ""),
    (r#"print(int([1]))"#, ""),
    (r#"print(int("1__0"))"#, ""),
    (
        r#"print(float(2), float("2.5"), float("1e3"), float(1.5), float("1_0.5"), float("-.5"))"#,
        "",
    ),
    (r#"print(float("abc"))"#, ""),
    (r#"print(float("inf"))"#, ""),
    (r#"print(float(nil))"#, ""),
    (
        r#"print(str("a"), str(1), str(1.0), str('ok), str([1, "a"]), str({a: "b"}), str(nil), str(1s), str((1, "x")), str(Ok("x")))"#,
        "",
    ),
    (
        r#"print(sym("ok"), sym('x), sym("참치"), sym("_x"), sym("a1"))"#,
        "",
    ),
    (r#"print(sym("Ab"))"#, ""),
    (r#"print(sym("a b"))"#, ""),
    (r#"print(sym(""))"#, ""),
    (r#"print(sym(1))"#, ""),
    (
        r#"print(parse_int("12"), parse_int("x"), parse_int(" 1"), parse_int("1_000"), parse_int("99999999999999999999"))"#,
        "",
    ),
    (r#"print(parse_int(1))"#, ""),
    (
        r#"print(parse_float("1.5"), parse_float("x"), parse_float("1e400"), parse_float("nan"))"#,
        "",
    ),
    (r#"print(parse_float(nil))"#, ""),
    (
        r#"print(parse_dur("1h30m"), parse_dur(" 500ms "), parse_dur("-5s"), parse_dur("30"), parse_dur("1h x"), parse_dur("30m1h"), parse_dur(""), parse_dur("1.5s"), parse_dur("- 5s"))"#,
        "",
    ),
    (r#"print(parse_dur(5))"#, ""),
    (
        r#"print(len("참치"), len([1, 2]), len({a: 1}), len((1, 2)), len(0..10), len(5..1), len(""))"#,
        "",
    ),
    (r#"print(len(1))"#, ""),
    (r#"print(len(nil))"#, ""),
    (
        r#"print(type_of(1), type_of("a"), type_of(nil), type_of(Ok(1)), type_of(x => x), type_of(print), type_of(1s), type_of(0..1), type_of((1,)), type_of('a), type_of({}), type_of([]), type_of(1.0), type_of(true))"#,
        "",
    ),
    (r#"print()"#, ""),
    (r#"print(1, "a", [1, "a"], nil)"#, ""),
    (
        r#"eprint("err", 1)
print("out")"#,
        "",
    ),
    (r#"fail("port", 1)"#, ""),
    (r#"fail()"#, ""),
    (r#"fail([1, "a"], 'x)"#, ""),
    (r#"print(at_exit(1))"#, ""),
    (
        r#"at_exit(() => print("bye"))
print("hi")"#,
        "",
    ),
    (r#"exit(3)"#, ""),
    (r#"exit()"#, ""),
    (r#"exit("a")"#, ""),
    (r#"exit(code: 4)"#, ""),
    (r#"print(read_line(), read_lines())"#, INPUT),
    (r#"print(read_line(1))"#, INPUT),
    (r#"print(read_lines("x"))"#, INPUT),
    (r#"print(dbg(1 + 2))"#, ""),
    (
        r#"x = 5
dbg(x * 2)"#,
        "",
    ),
    (
        r#"print(dbg(1, "t"), dbg(2, "t", "w"), dbg(3, where: "w"))"#,
        "",
    ),
    (r#"print(args)"#, ""),
    (r#"print(len(1, 2))"#, ""),
    (r#"print(map(xs: [1], f: x => x + 1))"#, ""),
    (r#"print(map([1], g: 1))"#, ""),
    (r#"print(fold([1, 2], 0))"#, ""),
    (r#"print(join(["a"], "-", "x"))"#, ""),
    (r#"print(map, fold, print)"#, ""),
    (
        r#"fn h(x) = x + 1
print(map([1, 2], h), filter([1, 2], (x) => x > 1), sort_by([2, 1], h))"#,
        "",
    ),
    (
        r#"fn inner(x) = x / 0
fn outer(xs) = map(xs, inner)
print(outer([1]))"#,
        "",
    ),
    (
        r#"fn k(x) = x / 0
fn m2(xs) = fold(xs, 0, (a, x) => a + k(x))
print(m2([1]))"#,
        "",
    ),
    (
        r#"fn bad(x) = sort_by(x, y => y / 0)
print(bad([1, 2]))"#,
        "",
    ),
    (r#"print(map([1, 2], x => exit(x + 5)))"#, ""),
    (r#"print(map([[1, 2], [3, 4]], ([a, b]) => a + b))"#, ""),
    (
        r#"print(fold([[1, 2], [3, 4]], 0, (acc, [a, b]) => acc + a * b))"#,
        "",
    ),
    (
        r#"print(each([1], print), map(["a"], upper), filter(["", "a"], x => len(x) > 0))"#,
        "",
    ),
    (
        r#"print(sort_by([3, 1], str), find([1], x => x > 0), any(0..3, x => x == 2))"#,
        "",
    ),
    (
        r#"print(map(0..=2, x => x), filter(0..=2, x => true), fold(-2..=2, 0, (a, x) => a + x))"#,
        "",
    ),
    (
        r#"print(sort([1, 1.0, 0.5]), sort([true, false]), sort(['b, 'a]))"#,
        "",
    ),
    (r#"print(sort([nil, nil]))"#, ""),
    (r#"print(sort([(1, 2), (1, 1)]))"#, ""),
    (r#"print(sort([{a: 1}, {a: 0}]))"#, ""),
    (r#"print(int(1s))"#, ""),
    (
        r#"print(str(-0.0), str(1e21), str(1.5e-7), str(100000000000000000000.0))"#,
        "",
    ),
    (
        r#"print(put({a: 1}, "a", 2) == {a: 2}, merge({a: 1}, {a: 1}) == {a: 1})"#,
        "",
    ),
    (
        r#"print(trim("  "), trim(""), split("a b", " "), upper(""), join([], "x"))"#,
        "",
    ),
    (r#"print(trim("aXbXa", "aX"))"#, ""),
    (
        r#"print(sort([3, 2, 1, 0, -1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21, -22, -23, -24, -25, "x", 0.5, nil]))"#,
        "",
    ),
    (
        r#"print(sort_by([5, 3, 9, 1, 7, 2, 8, 4, 6, 0, 15, 13, 19, 11, 17, 12, 18, 14, 16, 10, 25, 23, 29, 21, 27, 22, 28, 24, 26, 20, 31, 30], x => x % 7))"#,
        "",
    ),
    (
        r#"print(sort_by(0..50, x => x % 3 == 0 then -x else x))"#,
        "",
    ),
    (
        r#"print(map([1, 2, 3], x => [x, x * 2]) |> flatten |> sort)"#,
        "",
    ),
    (
        r#"print(fold(0..100000, 0, (a, x) => a + x), len(map(0..100000, x => x)), len(filter(0..100000, x => x % 2 == 0)))"#,
        "",
    ),
    (r#"print(trim(" a　"), trim("\t a \r "))"#, ""),
    (r#"print(upper("ŉ ǰ ﬀ ı ſ"), lower("ΣΑΣ Ǆ K Å"))"#, ""),
    (r#"print(int(9223372036854775807.0))"#, ""),
    (r#"print(int(-9223372036854775809.0))"#, ""),
    (
        r#"print(float("0x1p4"), float("0x1.8p1"), float("4.9e-324"), float("1E5"), float("00.5"), float("5."))"#,
        "",
    ),
    (r#"print(float("0x10"))"#, ""),
    (r#"print(int("٣"))"#, ""),
    (r#"print(sym("Ⅰx"))"#, ""),
    (r#"print(sym("a٣"))"#, ""),
    (r#"print(read_lines(), read_line())"#, INPUT),
    (
        r#"at_exit(print)
print("x")"#,
        "",
    ),
    (r#"print(map([1, 2], print))"#, ""),
    (r#"print(fold(["a"], "", join))"#, ""),
    (r#"print(sort_by([1], fail))"#, ""),
    (r#"print(map([3], exit))"#, ""),
    (r#"print(filter([1], type_of))"#, ""),
    (
        r#"print(each(0..3, x => x == 1 then exit(9) else print(x)))"#,
        "",
    ),
    (
        r#"fn a(xs) = map(xs, x => fold([x], 0, (s, y) => s + y / 0))
fn b() = a([1])
print(b())"#,
        "",
    ),
    (
        r#"fn g(x) = x / 0
print(map([1], x => g(x)))"#,
        "",
    ),
    (r#"print(len(map(0..200000, x => x)))"#, ""),
    (
        r#"fn d(n) = n == 0 then 0 else 1 + d(n - 1)
print(map([200000], d))"#,
        "",
    ),
    (
        r#"fn r(n) = n == 0 then 0 else fold([n], 0, (a, x) => r(x - 1))
print(r(60000))"#,
        "",
    ),
    (
        r#"fn r(n) = n == 0 then 0 else fold([n], 0, (a, x) => r(x - 1))
print(r(30000))"#,
        "",
    ),
    (
        r#"print(group_by(0..6, x => x % 2 == 0 then "even" else "odd"), pairs(group_by([], x => "a")))"#,
        "",
    ),
    (
        r#"print(sort_by(["b", "B", "a"], lower), sort(["b", "B", "a", "é", "e"]))"#,
        "",
    ),
    (
        r#"print(chars("👍🏽x"), len("👍🏽x"), upper("straße"), split("a👍b", "👍"))"#,
        "",
    ),
    // --- extras/prelude.fung (booted): results and errors inside it ---
    (
        r#"print(take([1, 2, 3], 2), drop(0..5, 3), sum([1, 2.5]), min([3, 1]), reverse("ab" |> chars))"#,
        "",
    ),
    (
        r#"print(1)
take([1], "x")"#,
        "",
    ),
    (
        r#"fn f(x) = take(x, "a")
f([1])"#,
        "",
    ),
    (r#"sum([1, "a"])"#, ""),
    (r#"map([[1, "a"]], sum)"#, ""),
    (r#"pad_left("a", 3, "")"#, ""),
    (r#"pad_left("a", 200000000, " ")"#, ""),
    (r#"until(1, x => x + "a", x => false)"#, ""),
    (
        r#"fn r(n) = iterate(n, x => x / 0, 1)
r(1)"#,
        "",
    ),
    (r#"print(get([1], 0))"#, ""),
    (r#"print(lines(1))"#, ""),
    (r#"print(keys([]))"#, ""),
    (r#"print(take([1]))"#, ""),
    (
        r#"print(retry(() => Ok(1)), retry(() => 2, times: 1), until(1, x => x * 2, x => x > 50), iterate(1, x => x + 1, 3))"#,
        "",
    ),
    (
        r#"print(is_ok(Ok(1)), is_err(Err(1)), first([]), last(0..3), zip([1], [2, 3]), unique([1, 1]), contains("abc", "b"))"#,
        "",
    ),
    // --- concurrency: argument binding happens before the native runs,
    // so both give the same errors (the parameter tables are Go's) ---
    (r#"spawn()"#, ""),
    (r#"spawn(1, 2)"#, ""),
    (r#"channel(1, 2)"#, ""),
    (r#"await(t: 1, x: 2)"#, ""),
    (r#"recv()"#, ""),
    (r#"swap(1)"#, ""),
    (r#"sleep(d: 1ms, d: 2ms)"#, ""),
    // Tasks and `sleep` run for real (stage 3a); channels and atoms are
    // still placeholders.
    (r#"print(sleep(1ms), sleep(0s), sleep(-1s))"#, ""),
    (r#"spawn(1)"#, ""),
    (r#"await(1)"#, ""),
    (r#"cancel(1)"#, ""),
    (r#"await(spawn(() => 1), timeout: 1)"#, ""),
    (
        r#"t = spawn(() => 1)
print(t, [t], t == t, type_of(t), await(t), await(t), cancel(t))"#,
        "",
    ),
    (r#"print(spawn(() => 1) == spawn(() => 1))"#, ""),
    (
        r#"print(await(spawn(() => 1 / 0)), await(spawn(() => [1][5])))"#,
        "",
    ),
    (
        r#"t = spawn(() => sleep(10s))
cancel(t)
print(await(t))"#,
        "",
    ),
    (
        r#"t = spawn(() => 1)
t < t"#,
        "",
    ),
    (
        r#"print(pmap([1, 2, 3], x => x * x, limit: 2), pmap([], x => x))"#,
        "",
    ),
    (r#"sleep(1)"#, ""),
    (r#"sleep("1s")"#, ""),
    (r#"print(retry(() => Err(1), times: 2, delay: 1ms))"#, ""),
    (r#"print(retry(() => Err("x"), times: 3, delay: 0s))"#, ""),
    (r#"print(spawn, type_of(await), channel == channel)"#, ""),
    // --- process functions ---
    (r#"print(exec(["echo", "hi there"]))"#, ""),
    (r#"print(sh("echo a; echo b >&2; exit 3"))"#, ""),
    (r#"print(sh("exit 300"))"#, ""),
    (r#"print(sh("kill -9 $$"))"#, ""),
    (r#"print(exec(["cat"], stdin: "x"))"#, ""),
    (
        r#"print(exec(["sh", "-c", "cat; echo e >&2"], stdin: "in"))"#,
        "",
    ),
    (r#"print(sh("cat"), sh("true", stdin: "abc"))"#, ""),
    (
        r#"print(exec(["sh", "-c", "echo $A$B"], env: {"B": "2", "A": "1"}))"#,
        "",
    ),
    (
        r#"print(exec(["sh", "-c", "echo \"$fung_x\""], env: {"fung_x": "lo"}))"#,
        "",
    ),
    (
        r#"print(exec(["pwd"], cwd: "/")!.out, sh("pwd", cwd: "/usr/../")!.out)"#,
        "",
    ),
    (
        r#"print(sh("echo $PWD", cwd: "/tmp"), sh("echo $PWD", cwd: "/usr/../tmp/"))"#,
        "",
    ),
    (
        r#"print(sh("echo $PWD", cwd: "/tmp", env: {"X": "1"}))"#,
        "",
    ),
    (r#"print(sh("echo $PWD", cwd: ""))"#, ""),
    (r#"print(sh("echo \"$X\"", env: {"X": "a b=c"}))"#, ""),
    (r#"print(exec(["definitely-not-a-command-xyz"]))"#, ""),
    (
        r#"print(exec(["/nonexistent/x"]), exec(["/etc"]), exec([""]))"#,
        "",
    ),
    (
        r#"print(exec(["ls"], cwd: "/nonexistent"), exec(["ls"], cwd: "/etc/hosts"))"#,
        "",
    ),
    (
        r#"print(exec(["sh", "-c", "echo\u{0}"]), exec(["/bin/ls\u{0}"]), exec(["x\u{0}y"]))"#,
        "",
    ),
    (
        r#"print(exec(["sh"], env: {"A\u{0}": "x"}), exec(["sh"], env: {"A": "x\u{0}"}), exec(["ls"], cwd: "/\u{0}"))"#,
        "",
    ),
    (r#"print(exec(["ls"], cwd: "/\u{0}", env: {}))"#, ""),
    (
        r#"print(exec(["./definitely-not-here"]), exec(["sh/"]))"#,
        "",
    ),
    (
        r#"print(exec(["sh", "-c", "echo $FOO"], env: {"FOO": 1}))"#,
        "",
    ),
    (r#"print(exec(["sh"], env: 1))"#, ""),
    (r#"print(exec(["cat"], stdin: 1))"#, ""),
    (r#"print(exec(["cat"], cwd: 1, env: 1))"#, ""),
    (r#"print(exec([1], cwd: 1))"#, ""),
    (r#"print(sh(1), 2)"#, ""),
    (r#"print(exec("ls"))"#, ""),
    (r#"print(exec([1]))"#, ""),
    (r#"print(exec([]))"#, ""),
    (r#"print(exec())"#, ""),
    (r#"print(exec(["true"], shell: 1))"#, ""),
    (r#"print(env(1))"#, ""),
    (
        r#"print(env("FUNG_TEST_UNSET_VAR"), env(""), env("A=B"), type_of(env()), env("HOME") == env()["HOME"])"#,
        "",
    ),
    (
        r#"print(sh("printf 'a\\nb\\n'")!.out |> lines, sh("printf ok")!.out |> len)"#,
        "",
    ),
    (
        r#"x = sh("echo 참치")!
print(x.out, len(x.out), x.code)"#,
        "",
    ),
    (
        r#"fn run(c) = sh(c)
print(run("exit 2")?)"#,
        "",
    ),
    (r#"print(sh("exit 2")!)"#, ""),
    // Go's Unicode classes (spec/unicode.txt): map keys, sym, quoting.
    (
        r#"print({"x²": 1, "Ⅻ": 2, "हिंदी": 3, "ß": 4, "Σ1": 5, "한글": 6, "x٣": 7, "Ⓐ": 8})"#,
        "",
    ),
    (r#"print(sym("ß1"), sym("x٣"), sym("한글"), sym("ǅx"))"#, ""),
    (r#"print(sym("x²"))"#, ""),
    (r#"print(sym("ⓐ"))"#, ""),
    (r#"print(sym("हिंदी"))"#, ""),
    (
        r#"print(["a\u{378}b", "\u{E000}", "\u{A0}"], {"\u{378}": 1})"#,
        "",
    ),
];
