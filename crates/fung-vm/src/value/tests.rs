use std::cmp::Ordering;
use std::rc::Rc;

use super::*;

type BinOp = fn(&Value, &Value) -> Result<Value, fung_front::diag::Diag>;

const SEC: i64 = 1_000_000_000;
const MIN: i64 = 60 * SEC;

fn s(x: &str) -> Value {
    Value::str(x)
}

/// Same scalar variant and payload (floats by bits).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Dur(x), Value::Dur(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        _ => false,
    }
}

fn ok(f: BinOp, a: Value, b: Value, want: Value) {
    let got = f(&a, &b).unwrap_or_else(|e| panic!("{a:?} op {b:?}: {}", e.code));
    assert!(same(&got, &want), "{a:?} op {b:?} = {got:?}, want {want:?}");
}

fn fails(f: BinOp, a: Value, b: Value, code: &str) {
    match f(&a, &b) {
        Err(e) => assert_eq!(e.code, code, "{a:?} op {b:?}"),
        Ok(v) => panic!("{a:?} op {b:?} = {v:?}, want {code}"),
    }
}

fn native(name: &'static str) -> Value {
    fn never(_: &mut crate::vm::Task, _: Vec<Value>) -> Result<Value, crate::vm::Stop> {
        Ok(Value::Nil)
    }
    Value::Native(Native::leak(name, vec![], never))
}

#[test]
fn value_is_16_bytes() {
    assert_eq!(std::mem::size_of::<Value>(), 16);
}

#[test]
fn print_forms_of_each_kind() {
    let vals = [
        Value::tuple(vec![Value::Int(1), s("x")]),
        Value::Float(1.5),
        Value::Dur(90 * MIN),
        Value::Sym(intern("s")),
        Value::Nil,
    ];
    let shown: Vec<String> = vals.iter().map(format_for_print).collect();
    assert_eq!(shown.join(" "), "(1, \"x\") 1.5 1h30m 's nil");
}

#[test]
fn integer_overflow_is_an_error() {
    let e = add(&Value::Int(i64::MAX), &Value::Int(1)).unwrap_err();
    assert_eq!(e.code, "E0503");
}

#[test]
fn add_sub_mul() {
    ok(add, Value::Int(1), Value::Int(2), Value::Int(3));
    ok(add, Value::Int(1), Value::Float(2.5), Value::Float(3.5));
    ok(sub, Value::Float(1.0), Value::Int(3), Value::Float(-2.0));
    ok(mul, Value::Int(6), Value::Int(7), Value::Int(42));
    fails(add, Value::Int(1), Value::Bool(true), "E0501");
    fails(mul, Value::Nil, Value::Int(1), "E0501");
}

#[test]
fn integer_overflow_cases() {
    let (max, min) = (Value::Int(i64::MAX), Value::Int(i64::MIN));
    fails(add, max.clone(), Value::Int(1), "E0503");
    fails(sub, min.clone(), Value::Int(1), "E0503");
    fails(mul, max.clone(), Value::Int(2), "E0503");
    fails(mul, min.clone(), Value::Int(-1), "E0503");
    fails(floor_div, min, Value::Int(-1), "E0503");
    fails(pow, Value::Int(2), Value::Int(63), "E0503");
    ok(pow, Value::Int(2), Value::Int(62), Value::Int(1 << 62));
    ok(add, max, Value::Int(-1), Value::Int(i64::MAX - 1));
}

#[test]
fn division() {
    ok(div, Value::Int(10), Value::Int(4), Value::Float(2.5));
    ok(div, Value::Int(10), Value::Int(2), Value::Float(5.0));
    ok(floor_div, Value::Int(7), Value::Int(2), Value::Int(3));
    ok(floor_div, Value::Int(-7), Value::Int(2), Value::Int(-4));
    ok(
        floor_div,
        Value::Float(7.5),
        Value::Int(2),
        Value::Float(3.0),
    );
    ok(rem, Value::Int(7), Value::Int(3), Value::Int(1));
    ok(rem, Value::Int(-7), Value::Int(3), Value::Int(2));
    ok(rem, Value::Float(-7.5), Value::Int(2), Value::Float(0.5));
    for f in [div as BinOp, floor_div, rem] {
        fails(f, Value::Int(1), Value::Int(0), "E0502");
        fails(f, Value::Float(1.0), Value::Float(0.0), "E0502");
        fails(f, Value::Int(1), Value::Float(0.0), "E0502");
    }
}

/// Float `//` and `%` come from the exact remainder (CPython's
/// `float_divmod`), not from the rounded quotient: the remainder stays
/// smaller than the divisor and takes its sign.
#[test]
fn float_floor_div_and_rem_use_the_exact_remainder() {
    let f = Value::Float;
    let cases = [
        // x, y, x // y, x % y
        (7.7e40, 0.1, 7.7e41, 0.04850743287925943),
        (123456789012345678.0, 3.0, 4.1152263004115224e16, 2.0),
        (1e300, 7.0, 1.4285714285714286e299, 1.0),
        (1.0, 0.1, 9.0, 0.09999999999999995),
        (-7.5, 2.0, -4.0, 0.5),
        (7.5, -2.0, -4.0, -0.5),
        (-0.5, 2.0, -1.0, 1.5),
        (6.0, -3.0, -2.0, -0.0),
        (-6.0, 3.0, -2.0, 0.0),
        (0.0, -5.0, -0.0, -0.0),
    ];
    for (x, y, q, r) in cases {
        ok(floor_div, f(x), f(y), f(q));
        ok(rem, f(x), f(y), f(r));
    }
    ok(floor_div, Value::Int(1), f(0.1), f(9.0));
    fails(floor_div, f(5.0), f(1e-320), "E0504");
}

#[test]
fn pow_cases() {
    ok(pow, Value::Int(2), Value::Int(10), Value::Int(1024));
    ok(pow, Value::Int(2), Value::Int(0), Value::Int(1));
    ok(pow, Value::Int(-2), Value::Int(3), Value::Int(-8));
    ok(pow, Value::Int(2), Value::Int(-1), Value::Float(0.5));
    ok(pow, Value::Float(2.0), Value::Int(2), Value::Float(4.0));
    ok(pow, Value::Int(4), Value::Float(0.5), Value::Float(2.0));
    fails(pow, Value::Int(2), Value::Bool(true), "E0501");
}

#[test]
fn non_finite_results_are_errors() {
    fails(mul, Value::Float(1e308), Value::Float(10.0), "E0504");
    fails(pow, Value::Float(10.0), Value::Int(400), "E0504");
    fails(
        sub,
        Value::Float(f64::INFINITY),
        Value::Float(f64::INFINITY),
        "E0504",
    );
}

#[test]
fn unary() {
    assert!(same(&neg(&Value::Int(3)).unwrap(), &Value::Int(-3)));
    assert!(same(&neg(&Value::Float(2.5)).unwrap(), &Value::Float(-2.5)));
    assert_eq!(neg(&Value::Int(i64::MIN)).unwrap_err().code, "E0503");
    assert_eq!(neg(&Value::Bool(true)).unwrap_err().code, "E0506");
    assert!(same(&not(&Value::Bool(true)).unwrap(), &Value::Bool(false)));
    assert_eq!(not(&Value::Int(1)).unwrap_err().code, "E0505");
}

#[test]
fn error_messages_name_the_operation() {
    use fung_front::diag::message;
    let e = add(&Value::Int(1), &Value::Bool(true)).unwrap_err();
    assert_eq!(message(&e.code, &e.args), "cannot add int and bool");
    let e = neg(&Value::Bool(true)).unwrap_err();
    assert_eq!(message(&e.code, &e.args), "cannot negate bool");
    let e = not(&Value::Int(1)).unwrap_err();
    assert_eq!(message(&e.code, &e.args), "expected bool, got int");
    assert!(e.pos.is_zero());
}

#[test]
fn equal_cases() {
    let cases = [
        (Value::Int(1), Value::Int(1), true),
        (Value::Int(1), Value::Float(1.0), true),
        (Value::Float(1.5), Value::Int(1), false),
        (Value::Bool(true), Value::Bool(true), true),
        (Value::Nil, Value::Nil, true),
        (Value::Int(1), Value::Bool(true), false),
        (Value::Nil, Value::Int(0), false),
        (Value::Int(0), Value::Bool(false), false),
    ];
    for (a, b, want) in cases {
        assert_eq!(equal(&a, &b), want, "{a:?} == {b:?}");
    }
    let n = native("print");
    assert!(equal(&n, &n));
    assert!(!equal(&n, &native("print")));
}

#[test]
fn compare_cases() {
    let cases = [
        (Value::Int(1), Value::Int(2), Ordering::Less),
        (Value::Int(2), Value::Int(2), Ordering::Equal),
        (Value::Float(2.5), Value::Int(2), Ordering::Greater),
        (Value::Int(1), Value::Float(1.0), Ordering::Equal),
    ];
    for (a, b, want) in cases {
        assert_eq!(compare(&a, &b).unwrap(), want, "{a:?} <=> {b:?}");
    }
    for (a, b) in [
        (Value::Bool(true), Value::Bool(false)),
        (Value::Int(1), Value::Bool(true)),
        (Value::Nil, Value::Nil),
    ] {
        assert_eq!(compare(&a, &b).unwrap_err().code, "E0501");
    }
}

#[test]
fn int_float_comparison_is_exact() {
    // 2^53 + 1 is not representable as an f64.
    let i = Value::Int(9007199254740993);
    let f = Value::Float(9007199254740992.0);
    assert!(!equal(&i, &f) && !equal(&f, &i));
    assert_eq!(compare(&i, &f).unwrap(), Ordering::Greater);
    assert_eq!(compare(&f, &i).unwrap(), Ordering::Less);
    assert_eq!(
        compare(&Value::Int(i64::MIN), &Value::Float(-9.3e18)).unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        compare(&Value::Int(1), &Value::Float(1e19)).unwrap(),
        Ordering::Less
    );
    assert!(equal(&Value::Int(3), &Value::Float(3.0)));
    assert!(!equal(&Value::Int(3), &Value::Float(3.5)));
    assert_eq!(
        compare(&Value::Int(3), &Value::Float(3.5)).unwrap(),
        Ordering::Less
    );
}

#[test]
fn format_literals() {
    let cases = [
        (Value::Int(0), "0"),
        (Value::Int(-42), "-42"),
        (Value::Int(i64::MAX), "9223372036854775807"),
        (Value::Float(5.0), "5.0"),
        (Value::Float(2.5), "2.5"),
        (Value::Float(-0.25), "-0.25"),
        (Value::Float(1e21), "1e21"),
        (Value::Float(1e-7), "1e-7"),
        (Value::Float(123456789.0), "123456789.0"),
        (Value::Float(1e6), "1000000.0"),
        (Value::Bool(true), "true"),
        (Value::Bool(false), "false"),
        (Value::Nil, "nil"),
        (native("print"), "<fn print>"),
        (native(""), "<fn>"),
    ];
    for (v, want) in cases {
        assert_eq!(format(&v), want, "{v:?}");
    }
}

/// Map keys print bare only when they read as a name or a tag (01, UAX
/// #31).
#[test]
fn format_map_keys_follow_the_name_rule() {
    let mut m = Map::new();
    for (i, k) in ["x²", "Ⅻ", "हिंदी", "ß", "Σ1", "한글", "x٣", "Ⓐ"]
        .into_iter()
        .enumerate()
    {
        m.put(Rc::new(k.to_string()), Value::Int(i as i64 + 1));
    }
    assert_eq!(
        format(&Value::Map(Rc::new(m))),
        r#"{"x²": 1, Ⅻ: 2, हिंदी: 3, ß: 4, Σ1: 5, 한글: 6, x٣: 7, "Ⓐ": 8}"#
    );
    assert_eq!(format(&s("a\u{378}b")), r#""a\u{378}b""#);
}

#[test]
fn format_shortest_round_trip() {
    let (a, b) = (std::hint::black_box(0.1), std::hint::black_box(0.2));
    assert_eq!(format(&Value::Float(a + b)), "0.30000000000000004");
}

#[test]
fn type_names() {
    let cases = [
        (Value::Int(1), "int"),
        (Value::Float(1.0), "float"),
        (Value::Bool(true), "bool"),
        (Value::Nil, "nil"),
        (native(""), "fn"),
        (s("a"), "str"),
        (Value::Sym(intern("a")), "sym"),
        (Value::Dur(1), "dur"),
        (Value::tuple(vec![]), "tuple"),
        (
            Value::Range(Rc::new(Range {
                start: 1,
                end: 2,
                inclusive: false,
            })),
            "range",
        ),
        (
            Value::Tag(Rc::new(Tag {
                name: intern("Circle"),
                args: vec![],
            })),
            "Circle",
        ),
    ];
    for (v, want) in cases {
        assert_eq!(type_name(&v), want, "{v:?}");
    }
}

#[test]
fn tuple_tag_range_forms() {
    let one = Value::tuple(vec![Value::Int(1)]);
    assert_eq!(format(&one), "(1,)");
    let ok2 = Value::Tag(Rc::new(Tag {
        name: intern("Ok"),
        args: vec![Value::Int(2)],
    }));
    assert_eq!(format(&ok2), "Ok(2)");
    let r = |inclusive| {
        Value::Range(Rc::new(Range {
            start: 1,
            end: 3,
            inclusive,
        }))
    };
    assert_eq!(format(&r(false)), "1..3");
    assert_eq!(format(&r(true)), "1..=3");
    assert!(equal(&r(false), &r(false)) && !equal(&r(false), &r(true)));
    let t = |a, b| Value::tuple(vec![Value::Int(a), Value::Int(b)]);
    assert!(equal(&t(1, 2), &t(1, 2)) && !equal(&t(1, 2), &t(1, 3)));
    assert_eq!(compare(&t(1, 2), &t(1, 3)).unwrap(), Ordering::Less);
    assert_eq!(
        compare(&t(1, 2), &Value::tuple(vec![Value::Int(1)])).unwrap(),
        Ordering::Greater
    );
}

#[test]
fn matches_type_cases() {
    let alts = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let red1 = Value::Tag(Rc::new(Tag {
        name: intern("Red"),
        args: vec![Value::Int(1)],
    }));
    let cases = [
        (Value::Int(1), alts(&["int"]), true),
        (Value::Int(1), alts(&["num"]), true),
        (Value::Float(1.0), alts(&["num"]), true),
        (s("a"), alts(&["num"]), false),
        (Value::Nil, alts(&["any"]), true),
        (Value::Nil, alts(&["str", "nil"]), true),
        (Value::Bool(true), alts(&["str", "nil"]), false),
        (native("p"), alts(&["fn"]), true),
        (Value::Int(1), alts(&["Circle"]), false),
        // A tag name takes the tagged value and the symbol of that name
        // (07, T1): a tag without values is that symbol.
        (Value::Sym(intern("Red")), alts(&["Red", "Green"]), true),
        (Value::Sym(intern("Blue")), alts(&["Red", "Green"]), false),
        (Value::Sym(intern("Red")), alts(&["sym"]), true),
        (Value::Sym(intern("red")), alts(&["Red"]), false),
        (red1.clone(), alts(&["Red"]), true),
        (red1, alts(&["sym"]), false),
        (Value::Sym(intern("Red")), alts(&["any"]), true),
        (Value::Dur(1), alts(&["int", "dur"]), true),
        (Value::Int(1), alts(&["time"]), false),
    ];
    for (v, a, want) in cases {
        assert_eq!(matches_type(&v, Some(&a)), want, "{v:?} {a:?}");
    }
    assert!(matches_type(&Value::Int(1), None));
}

#[test]
fn fn_format_and_equality() {
    let (a, b) = (native("print"), native("print"));
    assert_eq!(format(&a), "<fn print>");
    assert_eq!(format(&native("")), "<fn>");
    assert!(equal(&a, &a) && !equal(&a, &b));
}

#[test]
fn dur_format() {
    let cases = [
        (90 * MIN, "1h30m"),
        (1500 * SEC / 1000, "1s500ms"),
        (500_000, "0.5ms"),
        (0, "0s"),
        (-5 * SEC, "-5s"),
        (2 * 3600 * SEC + 3 * SEC, "2h3s"),
        (1_500_000, "1.5ms"),
    ];
    for (d, want) in cases {
        assert_eq!(format(&Value::Dur(d)), want, "{d}");
    }
}

#[test]
fn dur_arithmetic() {
    let (sd, md) = (Value::Dur(SEC), Value::Dur(MIN));
    ok(add, sd.clone(), md.clone(), Value::Dur(61 * SEC));
    ok(sub, md.clone(), sd.clone(), Value::Dur(59 * SEC));
    ok(mul, sd.clone(), Value::Int(3), Value::Dur(3 * SEC));
    ok(
        mul,
        Value::Float(1.5),
        sd.clone(),
        Value::Dur(1500 * SEC / 1000),
    );
    ok(div, md.clone(), Value::Int(4), Value::Dur(15 * SEC));
    ok(div, sd.clone(), Value::Float(0.5), Value::Dur(2 * SEC));
    fails(add, sd.clone(), Value::Int(1), "E0501");
    ok(div, md.clone(), sd.clone(), Value::Float(60.0)); // a number (07, L5)
    fails(div, sd.clone(), Value::Int(0), "E0502");
    fails(floor_div, md.clone(), Value::Int(2), "E0501");
    fails(rem, md.clone(), Value::Int(2), "E0501");
    assert!(same(&neg(&sd).unwrap(), &Value::Dur(-SEC)));
    assert_eq!(compare(&sd, &md).unwrap(), Ordering::Less);
    assert!(equal(&sd, &Value::Dur(SEC)) && !equal(&sd, &Value::Int(1_000_000_000)));
}

/// A duration divided by an int is exact, rounded to the nearest
/// nanosecond with halves away from zero (as a duration literal rounds);
/// a float divisor goes through f64.
#[test]
fn dur_div_by_int_is_exact() {
    const H: i64 = 60 * MIN;
    const MS: i64 = 1_000_000;
    for d in [2562047 * H + MS, 43800 * H + 7 * MS + 1, i64::MAX, i64::MIN] {
        ok(div, Value::Dur(d), Value::Int(1), Value::Dur(d));
    }
    let cases = [
        (7, 2, 4),
        (-7, 2, -4),
        (7, -2, -4),
        (5, 3, 2),
        (4, 3, 1),
        (-5, 3, -2),
        (i64::MAX, -1, -i64::MAX),
        (i64::MIN, 2, i64::MIN / 2),
        (i64::MAX, i64::MAX, 1),
        (i64::MAX, i64::MIN, -1),
        (1, i64::MAX, 0),
    ];
    for (d, n, want) in cases {
        ok(div, Value::Dur(d), Value::Int(n), Value::Dur(want));
    }
    fails(div, Value::Dur(i64::MIN), Value::Int(-1), "E0503");
}

/// Durations divided by durations give numbers (07, L5): `/` a float,
/// `//` an int and `%` a duration by the floor rule of ints.
#[test]
fn dur_by_dur_gives_numbers() {
    const MS: i64 = 1_000_000;
    let d = Value::Dur;
    ok(div, d(1500 * MS), d(MS), Value::Float(1500.0));
    ok(div, d(MS), d(4 * MS), Value::Float(0.25));
    ok(div, d(2 * MS), d(MS), Value::Float(2.0));
    // Exact past 2^53 ns: the quotient's whole part is an integer division.
    ok(div, d(i64::MAX), d(1), Value::Float(i64::MAX as f64));
    ok(div, d((1 << 60) + 1), d(1 << 59), Value::Float(2.0));
    ok(div, d(-3 * MS), d(2 * MS), Value::Float(-1.5));
    let floors = [
        (7, 2, 3, 1),
        (-7, 2, -4, 1),
        (7, -2, -4, -1),
        (-7, -2, 3, -1),
        (6, 3, 2, 0),
    ];
    for (x, y, q, r) in floors {
        ok(floor_div, d(x), d(y), Value::Int(q));
        ok(rem, d(x), d(y), d(r));
    }
    ok(rem, d(i64::MIN), d(-1), d(0));
    fails(floor_div, d(i64::MIN), d(-1), "E0503");
    for f in [div as BinOp, floor_div, rem] {
        fails(f, d(MS), d(0), "E0502");
    }
    // Mixed: dur // n, dur % n and n / dur stay errors.
    fails(floor_div, d(MS), Value::Int(2), "E0501");
    fails(rem, d(MS), Value::Int(2), "E0501");
    fails(div, Value::Int(2), d(MS), "E0501");
}

#[test]
fn dur_overflow_is_an_error() {
    let big = Value::Dur(1 << 62);
    fails(add, big.clone(), big.clone(), "E0503");
    fails(mul, big.clone(), Value::Int(4), "E0503");
    fails(mul, big, Value::Float(4.0), "E0503");
}

#[test]
fn str_format_quotes_and_escapes() {
    let cases = [
        ("a b", r#""a b""#),
        ("tab\there", r#""tab\there""#),
        ("nl\n", r#""nl\n""#),
        ("q\"q", r#""q\"q""#),
        ("back\\slash", r#""back\\slash""#),
        ("cost $5", r#""cost $5""#),
        ("tmpl ${x}", r#""tmpl \${x}""#),
        ("참치", r#""참치""#),
        ("bell\x07", r#""bell\u{7}""#),
    ];
    for (input, want) in cases {
        assert_eq!(format(&s(input)), want, "{input:?}");
    }
    assert_eq!(format_for_print(&s("a \"b\"")), "a \"b\"");
    assert_eq!(type_name(&s("x")), "str");
}

#[test]
fn str_concat_and_errors() {
    ok(add, s("a"), s("b"), s("ab"));
    fails(add, s("a"), Value::Int(1), "E0501");
    fails(sub, s("a"), s("b"), "E0501");
    fails(mul, s("a"), Value::Int(3), "E0501");
}

/// `+` keeps to the string limit of 100 million characters (02), counted
/// in characters, not bytes.
#[test]
fn str_concat_past_the_limit_is_an_error() {
    use fung_front::diag::message;
    let half = s(&"a".repeat(60_000_000));
    let e = add(&half, &half).unwrap_err();
    assert_eq!(
        message(&e.code, &e.args),
        "too many characters (limit 100000000)"
    );
    // 120 million bytes, but 60 million characters.
    let wide = s(&"é".repeat(30_000_000));
    let Value::Str(both) = add(&wide, &wide).unwrap() else {
        panic!("not a string");
    };
    assert_eq!(both.len(), 120_000_000);
}

#[test]
fn str_equal_and_compare() {
    assert!(equal(&s("a"), &s("a")) && !equal(&s("a"), &s("b")) && !equal(&s("1"), &Value::Int(1)));
    assert_eq!(compare(&s("abc"), &s("abd")).unwrap(), Ordering::Less);
    assert_eq!(compare(&s("가"), &s("나")).unwrap(), Ordering::Less);
    assert_eq!(compare(&s("a"), &Value::Int(1)).unwrap_err().code, "E0501");
}

#[test]
fn symbols_intern_and_format() {
    let (a, b) = (Value::Sym(intern("ok")), Value::Sym(intern("ok")));
    assert_eq!(intern("ok").name(), "ok");
    assert!(intern("ok") == intern("ok") && intern("err") != intern("ok"));
    assert_eq!(format(&a), "'ok");
    assert_eq!(type_name(&a), "sym");
    assert!(equal(&a, &b) && !equal(&a, &s("ok")) && !equal(&a, &Value::Sym(intern("err"))));
    assert_eq!(compare(&a, &b).unwrap_err().code, "E0501");
    fails(add, a, b, "E0501");
}

#[test]
fn dur_sub_is_exact() {
    // the result fits, so it is the value, whatever the right side is
    ok(
        sub,
        Value::Dur(-1),
        Value::Dur(i64::MIN),
        Value::Dur(i64::MAX),
    );
    fails(sub, Value::Dur(0), Value::Dur(i64::MIN), "E0503");
    // the same holds for ints
    ok(
        sub,
        Value::Int(-1),
        Value::Int(i64::MIN),
        Value::Int(i64::MAX),
    );
}

// ---- persistent vectors, lists, the list API and collections ----

use super::pvec::Pvec;

use super::testutil::Rng;

fn list(items: Vec<Value>) -> Value {
    Value::List(Rc::new(List::new(items)))
}

fn ints(xs: &[i64]) -> Vec<Value> {
    xs.iter().map(|&x| Value::Int(x)).collect()
}

fn int_of(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        other => panic!("not an int: {other:?}"),
    }
}

/// `l` holds exactly `want`, read through `at`, `iter` and `to_vec`.
fn same_list(what: &str, l: &List, want: &[i64]) {
    assert_eq!(l.len(), want.len(), "{what}: len");
    for (i, &x) in want.iter().enumerate() {
        assert_eq!(l.at(i).map(int_of), Some(x), "{what}: at({i})");
    }
    assert!(l.at(want.len()).is_none(), "{what}: at(len)");
    let got: Vec<i64> = l.iter().map(int_of).collect();
    assert_eq!(got, want, "{what}: iter");
    let got: Vec<i64> = l.to_vec().iter().map(int_of).collect();
    assert_eq!(got, want, "{what}: to_vec");
}

// TestPvecMatchesSlice: every version stays readable and unchanged.
#[test]
fn pvec_matches_slice() {
    let mut r = Rng::new(0x1234_5678_9abc_def1);
    let mut cur: (Pvec<i64>, Vec<i64>) = (Pvec::of(vec![]), vec![]);
    let mut history = vec![cur.clone()];
    for step in 0..40000 {
        let op = r.int_n(10);
        if op < 7 || cur.1.is_empty() {
            let x = r.int();
            cur.0.push(x);
            cur.1.push(x);
        } else {
            let i = r.int_n(cur.1.len());
            let x = r.int();
            cur.0.set(i, x);
            cur.1[i] = x;
        }
        if step % 997 == 0 {
            history.push(cur.clone());
        }
        if step % 4999 == 0 {
            // fork from an old version: the fork must not touch its parent
            let old = &history[r.int_n(history.len())];
            let mut f = old.0.clone();
            f.push(-1);
            assert_eq!(*f.at(f.len() - 1), -1);
            assert_eq!(old.0.len(), old.1.len(), "fork changed its parent");
        }
    }
    history.push(cur);
    for (v, want) in &history {
        assert_eq!(v.len(), want.len());
        for (i, x) in want.iter().enumerate() {
            assert_eq!(v.at(i), x, "at({i}) len {}", want.len());
        }
    }
}

/// `truncate` (P3) keeps the model's prefix, leaves every older version
/// as it was, and the cut vector grows on as any vector does.
#[test]
fn pvec_truncate_matches_the_model() {
    let mut r = Rng::new(0x0bad_cafe_f00d_1234);
    let mut cur: (Pvec<i64>, Vec<i64>) = (Pvec::of(vec![]), vec![]);
    let mut history = vec![cur.clone()];
    for step in 0..40000 {
        match r.int_n(10) {
            0 if !cur.1.is_empty() => {
                let n = r.int_n(cur.1.len() + 1);
                cur.0.truncate(n);
                cur.1.truncate(n);
            }
            1 if !cur.1.is_empty() => {
                // pop-and-push, the stack shape of P3
                let n = cur.1.len() - 1;
                cur.0.truncate(n);
                cur.1.truncate(n);
            }
            _ => {
                let x = r.int();
                cur.0.push(x);
                cur.1.push(x);
            }
        }
        if step % 499 == 0 {
            history.push(cur.clone());
        }
    }
    history.push(cur);
    for (v, want) in &history {
        assert_eq!(v.len(), want.len());
        for (i, x) in want.iter().enumerate() {
            assert_eq!(v.at(i), x, "at({i}) len {}", want.len());
        }
    }
    for n in [1056, 32800, 33000] {
        let v = Pvec::of((0..n as i64).collect());
        for cut in [0, 1, 31, 32, 33, 1024, 1025, n - 1, n] {
            let mut w = v.clone();
            w.truncate(cut);
            for k in 0..1100 {
                w.push(-k);
            }
            assert_eq!(w.len(), cut + 1100, "n={n} cut={cut}");
            for i in 0..cut {
                assert_eq!(*w.at(i), i as i64, "n={n} cut={cut} at {i}");
            }
            for k in 0..1100 {
                assert_eq!(*w.at(cut + k as usize), -k, "n={n} cut={cut} push {k}");
            }
            assert_eq!(*v.at(n - 1), n as i64 - 1, "the original kept its end");
        }
    }
}

// TestPvecOfBoundaries
#[test]
fn pvec_of_boundaries() {
    for n in [0, 1, 31, 32, 33, 64, 1023, 1024, 1025, 1056, 32768, 32800] {
        let v = Pvec::of((0..n as i64).map(|i| i * 3).collect());
        assert_eq!(v.len(), n);
        for i in 0..n {
            assert_eq!(*v.at(i), i as i64 * 3, "n={n} at({i})");
        }
        let mut w = v.clone();
        for k in 0..1100 {
            // push across several tail flushes and a root growth
            w.push(-k);
        }
        for k in 0..1100 {
            assert_eq!(*w.at(n + k as usize), -k, "n={n} push {k}");
        }
        for i in 0..n {
            assert_eq!(*w.at(i), i as i64 * 3, "n={n} prefix changed at {i}");
            assert_eq!(*v.at(i), i as i64 * 3, "n={n} prefix changed at {i}");
        }
        if n > 1 {
            let mut s = v.clone();
            s.set(0, 7);
            s.set(n - 1, 8);
            assert_eq!((*s.at(0), *s.at(n - 1)), (7, 8), "n={n} set");
            assert_eq!(
                (*v.at(0), *v.at(n - 1)),
                (0, (n as i64 - 1) * 3),
                "n={n} set"
            );
        }
    }
}

// TestListMatchesSlice: random lists against plain vectors; old versions
// are re-read to check that no operation changes a list it came from.
#[test]
fn list_matches_slice() {
    let mut r = Rng::new(0x0bad_cafe_f00d_5eed);
    let mut cur: (List, Vec<i64>) = (List::new(vec![]), vec![]);
    let mut history: Vec<(List, Vec<i64>)> = vec![];
    for step in 0..30000 {
        let n = cur.1.len();
        let op = r.int_n(20);
        if op < 11 || n < 2 {
            // append, most of the time
            let x = r.int_n(1000) as i64;
            cur.0.push(Value::Int(x));
            cur.1.push(x);
        } else if op < 14 {
            // drop from the front (rest / slicing a tail)
            let k = 1 + r.int_n(n.min(40));
            cur = (cur.0.slice(k, n), cur.1[k..].to_vec());
        } else if op < 16 {
            // take a front part
            let k = r.int_n(n);
            cur = (cur.0.slice(0, k), cur.1[..k].to_vec());
        } else if op < 18 {
            // concat a small or big list made from cur
            let k = r.int_n(n);
            let o = cur.0.slice(k, n);
            let mut want = cur.1.clone();
            want.extend_from_slice(&cur.1[k..]);
            cur = (cur.0.concat(&o), want);
        } else if !history.is_empty() {
            // jump back to an old version
            cur = history[r.int_n(history.len())].clone();
        }
        let n = cur.1.len();
        if n > 6000 {
            // keep sizes moving across the small/big boundary
            cur = (cur.0.slice(n - 30, n), cur.1[n - 30..].to_vec());
        }
        if step % 301 == 0 {
            history.push(cur.clone());
        }
        if step % 2003 == 0 {
            for (l, want) in &history {
                same_list("history", l, want);
            }
        }
    }
    same_list("final", &cur.0, &cur.1);
}

// TestListViewAppendKeepsOriginal
#[test]
fn list_view_append_keeps_original() {
    let mut xs = List::new(vec![]);
    for i in 0..100 {
        xs.push(Value::Int(i));
    }
    let ys = xs.slice(0, 40);
    let mut zs = ys.clone();
    zs.push(Value::Int(-1));
    assert_eq!(
        xs.at(40).map(int_of),
        Some(40),
        "front slice append changed the original"
    );
    assert_eq!(zs.at(40).map(int_of), Some(-1));
    assert_eq!((zs.len(), ys.len()), (41, 40));
    let (mut a, mut b) = (xs.clone(), xs.clone());
    a.push(Value::Int(1));
    b.push(Value::Int(2));
    assert_eq!(a.at(100).map(int_of), Some(1), "forked appends interfere");
    assert_eq!(b.at(100).map(int_of), Some(2), "forked appends interfere");
    assert_eq!(xs.len(), 100);
    let mut tz = xs.slice(60, 100);
    tz.push(Value::Int(-2));
    assert_eq!(xs.len(), 100, "appending to a tail view");
    assert_eq!(tz.at(40).map(int_of), Some(-2));
    assert_eq!(tz.at(0).map(int_of), Some(60));
}

// TestListRestIsCheap: appending and taking the tail one at a time is
// linear overall. Timed only in release builds.
#[cfg(not(debug_assertions))]
#[test]
fn list_rest_is_cheap() {
    let start = std::time::Instant::now();
    let mut xs = List::new(vec![]);
    for i in 0..100_000 {
        xs.push(Value::Int(i));
    }
    let mut i = 0;
    while !xs.is_empty() {
        assert_eq!(xs.at(0).map(int_of), Some(i), "rest {i}");
        xs = xs.slice(1, xs.len());
        i += 1;
    }
    let took = start.elapsed();
    assert!(
        took.as_secs_f64() < 2.0,
        "100,000 appends and rests took {took:?}"
    );
}

// TestListBoundaries
#[test]
fn list_boundaries() {
    for n in [0, 1, 31, 32, 33, 64, 65, 1056] {
        let items: Vec<i64> = (0..n).collect();
        let l = List::new(ints(&items));
        same_list("new", &l, &items);
        let mut appended = l.clone();
        appended.push(Value::Int(-1));
        let mut want = items.clone();
        want.push(-1);
        same_list("push", &appended, &want);
        same_list("original after push", &l, &items);
        if n > 0 {
            same_list("slice", &l.slice(1, n as usize), &items[1..]);
        }
        let built = list(ints(&items));
        let Value::List(rc) = &built else {
            unreachable!()
        };
        assert_eq!(format(&Value::List(Rc::new(l.clone()))), format(&built));
        assert_eq!(rc.len(), l.len());
    }
}

// TestListAPI
#[test]
fn list_api() {
    let a = List::new(ints(&[1, 2, 3]));
    assert_eq!(a.len(), 3);
    assert_eq!(a.at(0).map(int_of), Some(1));
    assert_eq!(a.at(2).map(int_of), Some(3));
    let mut b = a.clone();
    b.push(Value::Int(4));
    assert_eq!(
        (a.len(), b.len()),
        (3, 4),
        "push must not change the original"
    );
    assert_eq!(b.at(3).map(int_of), Some(4));
    let fmt = |l: &List| format(&Value::List(Rc::new(l.clone())));
    assert_eq!(fmt(&b.slice(1, 3)), "[2, 3]");
    assert_eq!(fmt(&b), "[1, 2, 3, 4]");
    assert_eq!(fmt(&a.concat(&List::new(ints(&[9])))), "[1, 2, 3, 9]");
    let got: Vec<i64> = b.iter().map(int_of).collect();
    assert_eq!(got, [1, 2, 3, 4]);
    let mut raw = b.to_vec();
    raw[0] = Value::Int(99);
    assert_eq!(b.at(0).map(int_of), Some(1), "to_vec must copy");
    let mut lb = ListBuilder::default();
    lb.push(Value::Int(7));
    lb.push(Value::Int(8));
    assert_eq!(fmt(&lb.finish()), "[7, 8]");
    assert_eq!(List::new(vec![]).len(), 0);
    assert_eq!(fmt(&List::new(vec![])), "[]");
    assert_eq!(SMALL_MAX, 32);
}

#[test]
fn shared_list_is_never_changed() {
    let mut a = List::new((0..100).map(Value::Int).collect());
    let b = a.clone(); // shared
    a.push(Value::Int(-1));
    assert_eq!(b.len(), 100);
    assert_eq!(a.len(), 101);
    assert!(matches!(b.at(99), Some(Value::Int(99))));
}

#[test]
fn shared_small_list_and_tail_are_never_changed() {
    // small: the flat vector is copied by clone
    let mut a = List::new(ints(&[1, 2]));
    let b = a.clone();
    a.push(Value::Int(3));
    same_list("small original", &b, &[1, 2]);
    // big with a partly filled tail shared by two lists
    let base: Vec<i64> = (0..40).collect();
    let mut c = List::new(ints(&base));
    let d = c.clone();
    c.push(Value::Int(-1));
    same_list("tail original", &d, &base);
    // big with a full tail that moves into the trie
    let base: Vec<i64> = (0..64).collect();
    let mut e = List::new(ints(&base));
    let f = e.clone();
    for i in 0..100 {
        e.push(Value::Int(-i));
    }
    same_list("trie original", &f, &base);
}

#[test]
fn dropping_a_huge_list_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut l = List::new(vec![]);
            for i in 0..1_000_000 {
                l.push(Value::Int(i));
            }
            drop(l);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn dropping_deeply_nested_lists_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            // small lists inside small lists
            let mut v = Value::Nil;
            for _ in 0..1_000_000 {
                v = list(vec![v]);
            }
            drop(v);
            // big lists (pvec leaves) holding the next level, via tuples too
            let mut v = Value::Nil;
            for _ in 0..20_000 {
                let mut items = ints(&[0; 40]);
                items.push(Value::tuple(vec![v]));
                v = list(items);
            }
            drop(v);
        })
        .unwrap()
        .join()
        .unwrap();
}

// TestCollectionFormat (list cases; maps arrive with the map task)
#[test]
fn list_format() {
    let cases = [
        (
            list(vec![Value::Int(1), s("a"), Value::Nil]),
            r#"[1, "a", nil]"#,
        ),
        (list(vec![]), "[]"),
        (
            list(vec![list(ints(&[1])), Value::tuple(vec![s("s")])]),
            r#"[[1], ("s",)]"#,
        ),
    ];
    for (v, want) in cases {
        assert_eq!(format(&v), want);
    }
    assert_eq!(type_name(&list(vec![])), "list");
    // print and ${} quote strings inside a list
    assert_eq!(
        format_for_print(&list(vec![Value::Int(1), s("s")])),
        r#"[1, "s"]"#
    );
}

// TestCollectionEquality (list cases)
#[test]
fn list_equality() {
    let cases = [
        (
            list(ints(&[1, 2])),
            list(vec![Value::Int(1), Value::Float(2.0)]),
            true,
        ),
        (list(ints(&[1])), list(ints(&[1, 2])), false),
        (list(ints(&[1])), Value::tuple(ints(&[1])), false),
        (list(vec![]), Value::Nil, false),
    ];
    for (a, b, want) in cases {
        assert_eq!(equal(&a, &b), want, "{} == {}", format(&a), format(&b));
    }
    // big lists and views compare by items
    let big: Vec<i64> = (0..100).collect();
    let l = List::new(ints(&big));
    let view = Value::List(Rc::new(l.slice(10, 90)));
    assert!(equal(&view, &list(ints(&big[10..90]))));
    assert!(!equal(&view, &list(ints(&big[10..89]))));
}

// TestCollectionOrdering (list cases)
#[test]
fn list_ordering() {
    let c = compare(&list(ints(&[1, 2])), &list(ints(&[1, 3])));
    assert_eq!(c.unwrap(), Ordering::Less, "lexicographic");
    let c = compare(&list(ints(&[1])), &list(ints(&[1, 0])));
    assert_eq!(c.unwrap(), Ordering::Less, "shorter is smaller");
    let pairs = [
        (list(vec![]), Value::tuple(vec![])),
        (list(ints(&[1])), list(vec![s("a")])),
    ];
    for (a, b) in pairs {
        let e = compare(&a, &b).unwrap_err();
        assert_eq!(e.code, "E0501", "{} <=> {}", format(&a), format(&b));
    }
}

/// Two elements that do not compare name their own types, not the
/// collections' (sort reports what actually failed).
#[test]
fn ordering_error_names_the_elements() {
    use fung_front::diag::message;
    let shown = |a: Value, b: Value| {
        let e = compare(&a, &b).unwrap_err();
        message(&e.code, &e.args)
    };
    assert_eq!(
        shown(
            list(vec![Value::Int(1), s("a")]),
            list(vec![Value::Int(1), Value::Int(2)])
        ),
        "cannot compare str and int"
    );
    assert_eq!(
        shown(
            Value::tuple(vec![list(vec![Value::Nil])]),
            Value::tuple(vec![list(vec![Value::Nil])])
        ),
        "cannot compare nil and nil"
    );
    assert_eq!(
        shown(list(vec![]), Value::tuple(vec![])),
        "cannot compare list and tuple"
    );
}

// TestListConcatAndMapPut (list cases)
#[test]
fn list_concat() {
    let v = add(&list(ints(&[1])), &list(ints(&[2]))).unwrap();
    assert_eq!(format(&v), "[1, 2]");
    fails(add, list(vec![]), Value::Int(1), "E0501");
    fails(add, Value::Int(1), list(vec![]), "E0501");
    // big + small and big + big stay correct and leave the operands alone
    let a: Vec<i64> = (0..50).collect();
    let b: Vec<i64> = (100..140).collect();
    let (la, lb) = (list(ints(&a)), list(ints(&b)));
    let v = add(&la, &lb).unwrap();
    let Value::List(l) = &v else { unreachable!() };
    let want: Vec<i64> = a.iter().chain(&b).copied().collect();
    same_list("concat", l, &want);
    let Value::List(l) = &la else { unreachable!() };
    same_list("left operand", l, &a);
}

// modules

#[test]
fn module_value() {
    let mut m = Module::new("fs", "fs");
    m.set("read", Value::Int(1));
    m.set("write", Value::Int(2));
    let m = Value::Module(Rc::new(m));
    assert_eq!(type_name(&m), "module");
    assert_eq!(format(&m), "<module fs>");
    let Value::Module(rc) = &m else {
        unreachable!()
    };
    assert_eq!(rc.names(), ["read", "write"]);
    assert_eq!(rc.name, "fs");
    assert_eq!(rc.key, "fs");
    let got = index::index(&m, &s("read")).unwrap();
    assert!(same(&got, &Value::Int(1)));
    assert_eq!(index::index(&m, &s("nope")).unwrap_err().code, "E0303");
    assert_eq!(index::index(&m, &s("_hidden")).unwrap_err().code, "E0304");
    assert_eq!(index::index(&m, &Value::Int(0)).unwrap_err().code, "E0501");
    let other = Value::Module(Rc::new(Module::new("fs", "fs")));
    assert!(
        equal(&m, &m) && !equal(&m, &other),
        "modules compare by identity"
    );
    assert_eq!(compare(&m, &other).unwrap_err().code, "E0501");
}

#[test]
fn module_set_replaces_in_place_and_messages_match_go() {
    let mut m = Module::new("init", "/p/init.fung");
    m.set("a", Value::Int(1));
    m.set("b", Value::Int(2));
    m.set("a", Value::Int(3));
    assert_eq!(m.names(), ["a", "b"]);
    assert!(same(m.get("a").unwrap(), &Value::Int(3)));
    assert!(m.get("c").is_none());
    let m = Value::Module(Rc::new(m));
    let msg = |i: Value| fung_front::diag::headline(&index::index(&m, &i).unwrap_err());
    assert_eq!(
        msg(s("nope")),
        "error[E0303]: module `init` has no member `nope`"
    );
    assert_eq!(
        msg(s("_h")),
        "error[E0304]: `_h` is private to module `init`"
    );
    assert_eq!(
        msg(Value::Int(1)),
        "error[E0501]: cannot index module and int"
    );
    assert_eq!(
        fung_front::diag::headline(&index::field(&m, "nope").unwrap_err()),
        "error[E0303]: module `init` has no member `nope`"
    );
}

#[test]
fn deep_module_chain_drops_without_recursion() {
    // Each module holds the previous one as a member.
    let t = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut v = Value::Nil;
            for _ in 0..200_000 {
                let mut m = Module::new("m", "m");
                m.set("prev", v);
                v = Value::Module(Rc::new(m));
            }
            drop(v);
        })
        .unwrap();
    t.join().unwrap();
}

/// A slice out of range names the index as written: a negative one is
/// not shown counted from the end, an inclusive end not plus one, and
/// nothing wraps.
#[test]
fn slice_errors_name_the_index_as_written() {
    use fung_front::diag::Arg;
    let l = Value::List(Rc::new(List::new(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ])));
    let shown = |a: Option<i64>, b: Option<i64>, inclusive: bool| {
        let (a, b) = (a.map(Value::Int), b.map(Value::Int));
        let e = super::index::slice(&l, a.as_ref(), b.as_ref(), inclusive).unwrap_err();
        assert_eq!(e.code, "E0507");
        e.args[1].clone()
    };
    assert_eq!(shown(Some(-5), None, false), Arg::Int(-5));
    assert_eq!(shown(Some(0), Some(5), true), Arg::Int(5));
    assert_eq!(shown(Some(0), Some(i64::MAX), true), Arg::Int(i64::MAX));
    assert_eq!(shown(Some(i64::MIN), None, false), Arg::Int(i64::MIN));
}

/// A start after the end is not out of range: it has its own message,
/// with both ends as written.
#[test]
fn reversed_slice_names_both_ends() {
    use fung_front::diag::message;
    let l = Value::List(Rc::new(List::new(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ])));
    let shown = |a: i64, b: i64, inclusive: bool| {
        let (a, b) = (Value::Int(a), Value::Int(b));
        let e = super::index::slice(&l, Some(&a), Some(&b), inclusive).unwrap_err();
        message(&e.code, &e.args)
    };
    assert_eq!(
        shown(2, 1, false),
        "slice start 2 is after its end 1 (length 3)"
    );
    assert_eq!(
        shown(2, 0, true),
        "slice start 2 is after its end 0 (length 3)"
    );
    assert_eq!(
        shown(-1, 1, false),
        "slice start -1 is after its end 1 (length 3)"
    );
    let e = super::index::slice(
        &s("hello"),
        Some(&Value::Int(3)),
        Some(&Value::Int(2)),
        false,
    )
    .unwrap_err();
    assert_eq!(e.code, "E0516");
}

/// nil is not an open end: only a missing end is (`SLICE` marks it), so
/// a nil from `find` fails here instead of slicing to the end (02).
#[test]
fn nil_slice_bound_is_a_type_error() {
    use fung_front::diag::message;
    let l = Value::List(Rc::new(List::new(vec![Value::Int(1), Value::Int(2)])));
    for x in [&l, &s("hello")] {
        let e = super::index::slice(x, Some(&Value::Int(1)), Some(&Value::Nil), false).unwrap_err();
        assert_eq!(message(&e.code, &e.args), "cannot slice with int and nil");
        let e = super::index::slice(x, Some(&Value::Nil), None, false).unwrap_err();
        assert_eq!(message(&e.code, &e.args), "cannot slice with int and nil");
    }
}

// --- deep values (L9): walked without the Rust stack ---

/// Runs `f` on a thread with a 1 MiB stack: a walk that recursed once per
/// level would overflow it long before a million levels.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

const DEEP: usize = 1_000_000;

/// `bottom` wrapped `n` times by `wrap`.
fn deep(n: usize, bottom: Value, wrap: fn(Value) -> Value) -> Value {
    let mut v = bottom;
    for _ in 0..n {
        v = wrap(v);
    }
    v
}

fn in_list(v: Value) -> Value {
    Value::List(Rc::new(List::new(vec![v])))
}

fn in_tuple(v: Value) -> Value {
    Value::tuple(vec![v])
}

fn in_map(v: Value) -> Value {
    let mut b = MapBuilder::default();
    b.set(Rc::new("k".to_string()), v);
    Value::Map(Rc::new(b.finish()))
}

fn in_tag(v: Value) -> Value {
    Value::Tag(Rc::new(Tag {
        name: intern("Ok"),
        args: vec![v],
    }))
}

#[test]
fn deep_values_format_in_linear_time_without_recursion() {
    on_small_stack(|| {
        let start = std::time::Instant::now();
        for (wrap, open, close) in [
            (in_list as fn(Value) -> Value, "[", "]"),
            (in_tuple, "(", ",)"),
            (in_map, "{k: ", "}"),
            (in_tag, "Ok(", ")"),
        ] {
            let text = format(&deep(DEEP, Value::Int(7), wrap));
            assert_eq!(text.len(), DEEP * (open.len() + close.len()) + 1);
            assert!(text.starts_with(&open.repeat(3)), "{}", &text[..20]);
            assert!(text[DEEP * open.len()..].starts_with(&format!("7{close}")));
            assert!(text.ends_with(&close.repeat(3)));
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "{:?}",
            start.elapsed()
        );
    });
}

#[test]
fn deep_values_compare_and_hash_without_recursion() {
    on_small_stack(|| {
        for wrap in [in_list as fn(Value) -> Value, in_tuple, in_map, in_tag] {
            let a = deep(DEEP, Value::Int(1), wrap);
            let b = deep(DEEP, Value::Int(1), wrap);
            let c = deep(DEEP, Value::Int(2), wrap);
            assert!(equal(&a, &b));
            assert!(!equal(&a, &c));
            assert_eq!(eq(&a, &b).map_err(|d| d.code), Ok(true));
            let hash = |v: &Value| {
                let mut h = std::hash::DefaultHasher::new();
                hash_value(v, &mut h);
                std::hash::Hasher::finish(&h)
            };
            assert_eq!(hash(&a), hash(&b));
        }
        for wrap in [in_list as fn(Value) -> Value, in_tuple] {
            let a = deep(DEEP, Value::Int(1), wrap);
            let c = deep(DEEP, Value::Int(2), wrap);
            assert_eq!(compare(&a, &c).map_err(|d| d.code), Ok(Ordering::Less));
            assert_eq!(compare(&c, &a).map_err(|d| d.code), Ok(Ordering::Greater));
            let text = deep(DEEP, s("x"), wrap);
            assert_eq!(
                compare(&a, &text).map_err(|d| d.code),
                Err("E0501".to_string())
            );
        }
        // A symbol meeting a string at the bottom is E0517 (`eq`).
        let sym = deep(DEEP, Value::Sym(intern("x")), in_list);
        let st = deep(DEEP, s("x"), in_list);
        assert_eq!(eq(&sym, &st).map_err(|d| d.code), Err("E0517".to_string()));
    });
}

/// What the walks keep (L9): `compare` stops at the first difference and
/// reports the first pair that does not compare; maps equal and hash
/// alike whatever their order; a shared collection equals itself.
#[test]
fn the_walks_keep_their_rules() {
    let list = |xs: Vec<Value>| Value::List(Rc::new(List::new(xs)));
    let int = Value::Int;
    assert_eq!(
        compare(&list(vec![int(1), s("a")]), &list(vec![int(2), int(3)])).map_err(|d| d.code),
        Ok(Ordering::Less)
    );
    let e = compare(&list(vec![int(1), s("a")]), &list(vec![int(1), int(3)])).unwrap_err();
    assert_eq!(e.code, "E0501");
    assert_eq!(
        compare(&list(vec![int(1)]), &list(vec![int(1), int(0)])).map_err(|d| d.code),
        Ok(Ordering::Less)
    );
    let map = |kv: &[(&str, i64)]| {
        let mut b = MapBuilder::default();
        for (k, v) in kv {
            b.set(Rc::new(k.to_string()), Value::Int(*v));
        }
        Value::Map(Rc::new(b.finish()))
    };
    let (m1, m2) = (map(&[("a", 1), ("b", 2)]), map(&[("b", 2), ("a", 1)]));
    assert!(equal(&m1, &m2));
    assert!(!equal(&m1, &map(&[("a", 1), ("c", 2)])));
    let hash = |v: &Value| {
        let mut h = std::hash::DefaultHasher::new();
        hash_value(v, &mut h);
        std::hash::Hasher::finish(&h)
    };
    assert_eq!(hash(&m1), hash(&m2));
    assert_eq!(
        hash(&list(vec![int(1)])),
        hash(&list(vec![Value::Float(1.0)]))
    );
    let shared = list(vec![Value::Float(0.5)]);
    assert!(equal(&shared, &shared.clone()));
    assert_eq!(
        format(&list(vec![Value::tuple(vec![int(1)]), map(&[("a b", 1)])])),
        "[(1,), {\"a b\": 1}]"
    );
}
