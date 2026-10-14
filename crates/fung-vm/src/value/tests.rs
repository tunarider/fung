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
    Value::Native(Native::leak(name, vec![]))
}

#[test]
fn value_is_16_bytes() {
    assert_eq!(std::mem::size_of::<Value>(), 16);
}

#[test]
fn print_forms_like_go() {
    let vals = [
        Value::Tuple(Rc::new(vec![Value::Int(1), s("x")])),
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
        (Value::Float(1e-7), "1e-07"),
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
        (Value::Tuple(Rc::new(vec![])), "tuple"),
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
    let one = Value::Tuple(Rc::new(vec![Value::Int(1)]));
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
    let t = |a, b| Value::Tuple(Rc::new(vec![Value::Int(a), Value::Int(b)]));
    assert!(equal(&t(1, 2), &t(1, 2)) && !equal(&t(1, 2), &t(1, 3)));
    assert_eq!(compare(&t(1, 2), &t(1, 3)).unwrap(), Ordering::Less);
    assert_eq!(
        compare(&t(1, 2), &Value::Tuple(Rc::new(vec![Value::Int(1)]))).unwrap(),
        Ordering::Greater
    );
}

#[test]
fn matches_type_cases() {
    let alts = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
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
    fails(div, md.clone(), sd.clone(), "E0501");
    fails(div, sd.clone(), Value::Int(0), "E0502");
    fails(floor_div, md.clone(), Value::Int(2), "E0501");
    fails(rem, md.clone(), Value::Int(2), "E0501");
    assert!(same(&neg(&sd).unwrap(), &Value::Dur(-SEC)));
    assert_eq!(compare(&sd, &md).unwrap(), Ordering::Less);
    assert!(equal(&sd, &Value::Dur(SEC)) && !equal(&sd, &Value::Int(1_000_000_000)));
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
