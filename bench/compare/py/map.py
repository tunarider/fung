n = 200000
m = {}
for i in range(n):
    m[f"k{i}"] = i
s = 0
for i in range(n):
    s += m[f"k{i}"]
print(s)
