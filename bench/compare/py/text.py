counts = {}
err = 0
with open("data/logs.txt") as f:
    for l in f:
        p = l.rstrip("\n").split(" ")
        lv = p[1]
        ms = int(p[3].split("=")[1])
        counts[lv] = counts.get(lv, 0) + 1
        if lv == "ERROR":
            err += ms
for k in sorted(counts):
    print(k, counts[k])
print(err)
