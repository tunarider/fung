package vm

import (
	"runtime"
	"strings"
	"testing"
)

func TestCollectionFunctions(t *testing.T) {
	cases := map[string]string{
		"print(is_empty([]), is_empty(\"a\"), is_empty({}), is_empty(0..0))":                                                          "true false true true\n",
		"print(filter([1, 2, 3, 4], x => x % 2 == 0), filter(0..5, x => x > 2))":                                                      "[2, 4] [3, 4]\n",
		"print(fold([1, 2, 3], 0, (acc, x) => acc + x), fold([], 1, (a, x) => a))":                                                    "6 1\n",
		"print(each([1, 2], x => print(x)))":                                                                                          "1\n2\nnil\n",
		"print(sum([1, 2.5]), sum([]), min([3, 1, 2]), max([\"b\", \"a\"]), min([]))":                                                 "3.5 0 1 b nil\n",
		"print(sort([3, 1, 2]), sort([\"b\", \"a\"]), sort_by([\"bb\", \"a\"], len))":                                                 "[1, 2, 3] [\"a\", \"b\"] [\"a\", \"bb\"]\n",
		"xs = [1, 2]\nprint(reverse(xs), xs, range(3), range(1, 3))":                                                                  "[2, 1] [1, 2] 0..3 1..3\n",
		"print(zip([1, 2, 3], [\"a\", \"b\"]), first([]), first([1]), last([1, 2]))":                                                  "[(1, \"a\"), (2, \"b\")] nil 1 2\n",
		"print(take([1, 2, 3], 2), take([1], 5), drop([1, 2, 3], 1), drop([1], 5), take([1], -1))":                                    "[1, 2] [1] [2, 3] [] []\n",
		"print(find([1, 2, 3], x => x > 1), find([], x => true), any([1, 2], x => x > 1), all([], x => false), all([1], x => x > 1))": "2 nil true true false\n",
		"print(contains([1, 2], 2), contains(\"abc\", \"b\"), contains({a: 1}, \"a\"), contains(0..3, 3))":                            "true true true false\n",
		"print(unique([1, 1.0, 2, 1]), flatten([[1], [2, [3]], 4]))":                                                                  "[1, 2] [1, 2, [3], 4]\n",
		"print(group_by([\"ab\", \"c\", \"de\"], s => s[..1]))":                                                                       "{a: [\"ab\"], c: [\"c\"], d: [\"de\"]}\n",
		"m = {b: 1, a: 2}\nprint(keys(m), values(m), pairs(m))":                                                                       "[\"b\", \"a\"] [1, 2] [(\"b\", 1), (\"a\", 2)]\n",
		"m = {a: 1}\nprint(get(m, \"a\"), get(m, \"z\"), get(m, \"z\", 0), put(m, \"b\", 2), m)":                                      "1 nil 0 {a: 1, b: 2} {a: 1}\n",
		"print(merge({a: 1, b: 1}, {b: 2, c: 3}), remove({a: 1, b: 2}, \"a\"), remove({}, \"x\"))":                                    "{a: 1, b: 2, c: 3} {b: 2} {}\n",
		"print(chars(\"참치\"), map(0..2, x => x * x))":                                                                                 "[\"참\", \"치\"] [0, 1]\n",
		"print([3, 1, 2] |> sort |> reverse |> first)":                                                                                "3\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"filter(1, x => x)":            "E0506",
		"filter([1], x => 1)":          "E0505",
		"sort([1, \"a\"])":             "E0501",
		"min([1, \"a\"])":              "E0501",
		"fold(\"ab\", 0, (a, x) => a)": "E0506",
		"group_by([1], x => x)":        "E0514",
		"keys([])":                     "E0506",
		"put(1, \"a\", 2)":             "E0506",
		"get({}, 1)":                   "E0501",
		"get([1], 0)":                  "E0803",
		"get([10, 20], 20)":            "E0803",
		"min([nil, 1])":                "E0501",
		"max([nil, 1])":                "E0501",
		"chars(1)":                     "E0506",
		"sum([1, \"a\"])":              "E0501",
		"each([1], 1)":                 "E0403",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

// TestFungPreludeFunctions covers the prelude functions written in fung
// (extras/prelude.fung): the cases their Go versions had, plus ranges.
func TestFungPreludeFunctions(t *testing.T) {
	cases := map[string]string{
		"xs = [1, 2, 3]\nprint(take(xs, 2), take(xs, 9), take(xs, -1), drop(xs, 1), drop(xs, 9), drop(xs, -1), xs)":                               "[1, 2] [1, 2, 3] [] [2, 3] [] [1, 2, 3] [1, 2, 3]\n",
		"print(unique([1, 1.0, \"a\", \"a\"]), flatten([[1], 2, []]))":                                                                            "[1, \"a\"] [1, 2]\n",
		"print(pad_left(\"7\", 3, \"0\"), pad_right(\"참\", 3, \" \"), pad_left(\"long\", 2, \" \"), pad_left(\"ab\", 5, \"xy\"))":                 "007 참   long xyxab\n",
		"print(lines(\"\"), lines(\"a\"), lines(\"a\\n\"), lines(\"a\\n\\nb\"), lines(\"a\\r\\nb\\r\\n\"), lines(\"\\n\"), lines(\"참치\\n회\\n\"))": "[] [\"a\"] [\"a\"] [\"a\", \"\", \"b\"] [\"a\", \"b\"] [\"\"] [\"참치\", \"회\"]\n",
		"print(is_ok(Ok(1)), is_ok(Err(1)), is_ok(1), is_err(Err(1)), is_err(Err(1, 2)))":                                                         "true false false true true\n",
		"print(take(0..5, 2), drop(0..5, 4), first(1..3), last(1..3), reverse(0..3), zip(0..2, [\"a\"]))":                                         "[0, 1] [4] 1 2 [2, 1, 0] [(0, \"a\")]\n",
		"print(min([]), max([]), sum(0..4), min([2.5, 1]), get({a: 1}, \"a\", default: 0), range(2), range(1, 2))":                                "nil nil 6 1 1 0..2 1..2\n",
		"ch = channel(3)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(first(ch))":                                                                  "1\n",
		"ch = channel(3)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(last(ch))":                                                                   "2\n",
		"print(len(pad_left(\"a\", 34_000_000, \"abc\")), pad_left(\"x\", 5, \"ab\"))":                                                            "34000000 ababx\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"pad_left(\"a\", 3, \"\")":          "E0803",
		"pad_left(\"a\", 200000000, \" \")": "E0806",
		"lines(1)":                          "E0403",
		"take(1, 2)":                        "E0506",
		"first(\"abc\")":                    "E0506",
		"get({}, 1)":                        "E0501",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestProcessFunctions(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("needs a POSIX shell")
	}
	cases := map[string]string{
		"print(exec([\"echo\", \"hi there\"]))":                                                            "Ok({out: \"hi there\\n\", err: \"\", code: 0})\n",
		"print(sh(\"echo a; echo b >&2; exit 3\"))":                                                        "Err({kind: 'exit, code: 3, out: \"a\\n\", err: \"b\\n\", message: \"`/bin/sh` exited with code 3\"})\n",
		"print(exec([\"cat\"], stdin: \"x\")!.out)":                                                        "x\n",
		"print(exec([\"sh\", \"-c\", \"echo $FOO\"], env: {\"FOO\": \"bar\"})!.out)":                       "bar\n\n",
		"print(exec([\"pwd\"], cwd: \"/\")!.out)":                                                          "/\n\n",
		"match exec([\"definitely-not-a-command-xyz\"]): Err({kind: k}) => print(k); _ => print(\"?\") ;;": "'not_found\n",
		"print(env(\"FUNG_TEST_UNSET_VAR\"), type_of(env()))":                                              "nil 'map\n",
		"print(args)": "[]\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"exec([])":               "E0403",
		"exec([1])":              "E0403",
		"exec(\"ls\")":           "E0403",
		"exec([\"ls\"], cwd: 1)": "E0403",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestFungStdlib(t *testing.T) {
	cases := map[string]string{
		"import fs\nprint(fs.with_temp_dir(d => fs.exists(d)))": "true\n",
		"import fs\nimport path\nfs.with_temp_dir(fn(d):\n  p = path.join(d, \"x.txt\")\n  fs.write(p, \"a\\nb\\n\")!\n  r = fs.with_open(p, fn(h):\n    first = read_line(h)?\n    first\n  ;;)\n  print(r)\n  nil\n;;)": "a\n",
		"import fs\nprint(fs.with_open(\"/nonexistent/x\", h => 1))":                                                                                                               "Err({kind: 'not_found",
		"import fs\nkept = fs.with_temp_dir(fn(d):\n  p = d + \"/f\"\n  fs.write(p, \"\")!\n  fs.with_open(p, h => h)\n;;)\nprint(read_line(kept))":                                "Err({kind: 'closed",
		"import fs\nimport path\nfs.with_temp_dir(fn(d):\n  p = path.join(d, \"w.txt\")\n  fs.with_open(p, fn(h): fs.write_line(h, \"x\")! ;;, 'write)\n  print(fs.read(p)!)\n;;)": "x\n\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" {
			t.Errorf("%q: code = %s", src, code)
			continue
		}
		if !strings.HasPrefix(out, want) {
			t.Errorf("%q: out=%q, want prefix %q", src, out, want)
		}
	}
}

func TestFsReadLines(t *testing.T) {
	_, out, code := run(t, "import fs\nimport path\nfs.with_temp_dir(fn(d):\n  p = path.join(d, \"l.txt\")\n  fs.write(p, \"a\\n\\nb\\n\")!\n  print(fs.read_lines(p), fs.read_lines(path.join(d, \"nope\")) |> is_err)\n  nil\n;;)")
	if code != "" || out != "Ok([\"a\", \"\", \"b\"]) true\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}

// TestTakeDropCheckTheirCount: a bad count is reported as take/drop's own
// argument error (06: fung extras check arguments with fail), not as a
// comparison inside the helper, and a float count is not silently used.
func TestTakeDropCheckTheirCount(t *testing.T) {
	cases := map[string]string{
		`take([1], "x")`:  "take expects an int count, got 'str",
		`drop([1], "x")`:  "drop expects an int count, got 'str",
		`take([1], 1.5)`:  "take expects an int count, got 'float",
		`drop(0..3, nil)`: "drop expects an int count, got 'nil",
	}
	for src, want := range cases {
		rt := &Runtime{}
		b := buildWith(t, src, bootTestPrelude(t, rt))
		registerModules(rt, b)
		_, err := rt.Run(b.Proto)
		if err == nil || err.Code != "E0803" || !strings.Contains(err.Error(), want) {
			t.Errorf("%s: got %v, want E0803 %q", src, err, want)
		}
	}
}
