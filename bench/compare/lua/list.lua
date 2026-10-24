local xs = {}
for i = 0, 499999 do xs[#xs + 1] = i * 7 % 1000 end
local ys = {}
for _, x in ipairs(xs) do local y = x + 1; if y % 2 == 0 then ys[#ys + 1] = y end end
local zs = {}
for i, x in ipairs(xs) do zs[i] = x end
table.sort(zs, function(a, b) return a > b end)
local s = 0
for _, y in ipairs(ys) do s = s + y end
print(s + zs[1] + #ys)
