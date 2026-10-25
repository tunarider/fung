# Times the same programs in fung, Python, Lua, Ruby and Node: one
# warm-up run, then the median wall-clock time of 7 runs and the peak
# RSS of a run (macOS only, see RSS_OK; docs/notes/bench-languages.md). Every language's output must
# match the first one's, or the cell shows `differs`; a language without
# the program (or not installed) shows `-`.
#
#   python3 -I bench/compare/run.py target/release/fung
#   RUBY="ruby --yjit" python3 -I bench/compare/run.py target/release/fung
#
# PYTHON, LUA, RUBY and NODE replace a language's command (words split on
# spaces).
#
# fung is measured every time. The other languages' results are kept in
# records/<machine>.json and reused while nothing that decides them has
# changed: the method (METHOD), the program, its input data, the command
# and the interpreter's version. `--remeasure` measures them again.
import hashlib, json, math, os, platform, shutil, statistics, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
ARGS = [a for a in sys.argv[1:] if a != "--remeasure"]
REMEASURE = "--remeasure" in sys.argv[1:]
FUNG = os.path.abspath(ARGS[0]) if ARGS else sys.exit("usage: run.py [--remeasure] <fung binary>")
# Bump when the way a program is timed changes (warm-up, runs, what is
# taken), so that no result measured the old way is reused.
METHOD = 1
RUNS = 7
VERSION_FLAG = {"python": "--version", "lua": "-v", "ruby": "--version", "node": "--version"}


def command(env, default):
    return os.environ[env].split() if os.environ.get(env) else default


LANGS = [("fung", [FUNG], "fung"), ("python", command("PYTHON", ["python3", "-I"]), "py"),
         ("lua", command("LUA", ["lua"]), "lua"), ("ruby", command("RUBY", ["ruby"]), "rb"),
         ("node", command("NODE", ["node"]), "js")]
BENCHES = ["startup", "fib", "loop", "str", "map", "list", "text", "regex", "json",
           "chan", "spawn", "fs", "exec"]
GROUPS = [("core", ["fib", "loop", "str", "map", "list"]), ("text", ["text", "regex", "json"]),
          ("conc", ["chan", "spawn"]), ("sys", ["fs", "exec"])]


def make_data(d):
    """Deterministic input of the text, regex and json programs."""
    os.makedirs(d, exist_ok=True)
    levels = ["INFO", "INFO", "INFO", "DEBUG", "WARN", "INFO", "ERROR", "INFO", "DEBUG", "WARN"]
    with open(f"{d}/logs.txt", "w") as f:
        for i in range(200000):
            lv = levels[i * 7 % 10]
            f.write(f"2026-10-21T12:{i // 60 % 60:02d}:{i % 60:02d}Z {lv} service=api{i % 5} "
                    f"latency_ms={i * 37 % 1000} msg=request-{i}\n")
    rows = [{"id": i, "name": f"user-{i}", "active": i % 3 != 0, "score": (i * 13 % 1000) / 10,
             "tags": ["a", "b", "c"][: i % 4]} for i in range(100000)]
    with open(f"{d}/data.json", "w") as f:
        json.dump(rows, f)


# On Linux a child's peak RSS counts the memory it had before exec, and a
# child Python forks or spawns shares Python's memory until then, so every
# program would show at least this Python's size: memory is shown on
# macOS only.
RSS_OK = sys.platform == "darwin"


def once(cmd):
    t = time.perf_counter()
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    out = p.stdout.read()
    _, status, ru = os.wait4(p.pid, 0)
    dt = time.perf_counter() - t
    rss = ru.ru_maxrss / (1 if sys.platform == "darwin" else 1 / 1024)  # bytes
    return dt, rss / 1048576, " ".join(out.decode().split()), os.waitstatus_to_exitcode(status)



def measure(cmd):
    """One warm-up run, then RUNS: (median time, peak RSS, output, exit code)."""
    _, _, out, rc = once(cmd)
    if rc != 0:
        return None, None, out, rc
    runs = [once(cmd) for _ in range(RUNS)]
    return statistics.median(r[0] for r in runs), max(r[1] for r in runs), out, rc


def machine():
    """A name for this machine's records: system, architecture, CPU."""
    cpu = ""
    try:
        if sys.platform == "darwin":
            cpu = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True,
                                 text=True).stdout.strip()
        else:
            with open("/proc/cpuinfo") as f:
                for line in f:
                    if line.lower().startswith(("model name", "cpu part")):
                        cpu = line.split(":", 1)[1].strip()
                        break
    except OSError:
        pass
    name = f"{sys.platform}-{platform.machine()}-{cpu}".lower()
    return "".join(c if c.isalnum() else "-" for c in name).strip("-")


def digest(*paths):
    h = hashlib.sha256()
    for p in paths:
        with open(p, "rb") as f:
            h.update(f.read())
    return h.hexdigest()[:16]


def version(lang, cmd):
    r = subprocess.run(cmd + [VERSION_FLAG[lang]], capture_output=True, text=True)
    return (r.stdout + r.stderr).strip().splitlines()[0] if (r.stdout + r.stderr).strip() else "?"


def main():
    os.chdir(HERE)
    if not os.path.exists("data/data.json"):
        make_data("data")
    path = f"records/{machine()}.json"
    try:
        with open(path) as f:
            records = json.load(f)
    except (OSError, ValueError):
        records = {}
    data = digest("data/logs.txt", "data/data.json")
    versions = {}
    reused = measured = 0
    res = {}
    for b in BENCHES:
        ref = None
        for lang, cmd, ext in LANGS:
            f = f"{ext}/{b}.{ext}"
            if not os.path.exists(f) or shutil.which(cmd[0]) is None:
                continue
            if lang == "fung":
                t, rss, out, rc = measure(cmd + [f])
            else:
                if lang not in versions:
                    versions[lang] = version(lang, cmd)
                key = {"method": METHOD, "program": digest(f), "data": data, "command": cmd,
                       "version": versions[lang]}
                rec = records.get(lang, {}).get(b)
                if rec and rec["key"] == key and not REMEASURE:
                    t, rss, out, rc = rec["time"], rec["rss"], rec["out"], rec["rc"]
                    reused += 1
                else:
                    t, rss, out, rc = measure(cmd + [f])
                    records.setdefault(lang, {})[b] = {"key": key, "time": t, "rss": rss, "out": out,
                                                       "rc": rc, "measured": time.strftime("%Y-%m-%d")}
                    measured += 1
            ref = out if ref is None else ref
            if rc != 0 or out != ref:
                res.setdefault(b, {})[lang] = None
                continue
            res.setdefault(b, {})[lang] = (t, rss)
    os.makedirs("records", exist_ok=True)
    with open(path, "w") as f:
        json.dump(records, f, indent=1, sort_keys=True)
        f.write("\n")
    names = [l for l, _, _ in LANGS]
    print("| program | " + " | ".join(names) + " |")
    print("|---|" + "---|" * len(names))
    for b in BENCHES:
        cells = []
        for l in names:
            v = res.get(b, {}).get(l, "-")
            if v == "-" or v is None:
                cells.append("-" if v == "-" else "differs")
            elif RSS_OK:
                cells.append("%.3f s, %.0f MB" % v)
            else:
                cells.append("%.3f s" % v[0])
        print(f"| {b} | " + " | ".join(cells) + " |")
    print("\nfung time / other time, geometric mean per group (above 1: fung is slower)\n")
    print("| group | " + " | ".join(names[1:]) + " |")
    print("|---|" + "---|" * (len(names) - 1))
    for g, bs in GROUPS:
        cells = []
        for l in names[1:]:
            rs = [res[b]["fung"][0] / res[b][l][0] for b in bs if res.get(b, {}).get(l) and res[b].get("fung")]
            cells.append("%.2f (%d)" % (math.exp(sum(map(math.log, rs)) / len(rs)), len(rs)) if rs else "-")
        print(f"| {g} | " + " | ".join(cells) + " |")
    print(f"\nother languages: {measured} measured, {reused} reused from bench/compare/{path}")
    for lang, v in versions.items():
        print(f"  {lang}: {v}")


main()
