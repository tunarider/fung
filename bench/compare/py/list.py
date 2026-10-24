xs = [i * 7 % 1000 for i in range(500000)]
ys = [x for x in (x + 1 for x in xs) if x % 2 == 0]
zs = sorted(xs, key=lambda x: -x)
print(sum(ys) + zs[0] + len(ys))
