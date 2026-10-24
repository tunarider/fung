import os
n = 0
for _ in range(25000):
    n += os.path.exists("/") + (os.stat("/") is not None)
print(n)
