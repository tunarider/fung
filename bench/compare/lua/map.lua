local n = 200000
local m = {}
for i = 0, n - 1 do m["k" .. i] = i end
local s = 0
for i = 0, n - 1 do s = s + m["k" .. i] end
print(s)
