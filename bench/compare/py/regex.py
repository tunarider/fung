import re
pat = re.compile(r"latency_ms=(\d+)")
s = 0
with open("data/logs.txt") as f:
    for l in f:
        s += int(pat.search(l).group(1))
print(s)
