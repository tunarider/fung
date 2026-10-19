#!/bin/sh
# Times every bench/*.fung with each fung binary given: one warm-up run,
# then the median wall-clock time of 9 runs (bench/README.md). Output of
# the runs is discarded; a binary that fails a file shows `fails`.
#
#   scripts/bench.sh /tmp/fung-base target/release/fung
set -eu
if [ "$#" -eq 0 ]; then
    echo "usage: scripts/bench.sh <fung binary>..." >&2
    exit 2
fi
cd "$(dirname "$0")/.."
exec python3 -I -c '
import statistics, subprocess, sys, time, glob
bins = sys.argv[1:]
files = sorted(glob.glob("bench/*.fung"))
print("| file | " + " | ".join(bins) + " |")
print("|---|" + "---|" * len(bins))
for f in files:
    row = []
    for b in bins:
        def once():
            t = time.perf_counter()
            r = subprocess.run([b, f], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return (time.perf_counter() - t) if r.returncode == 0 else None
        if once() is None:
            row.append("fails")
            continue
        row.append("%.3f" % statistics.median(once() for _ in range(9)))
    print("| " + f.split("/")[1] + " | " + " | ".join(row) + " |")
' "$@"
