local cs = {}
for i = 0, 1999 do cs[#cs + 1] = coroutine.create(function() return i * i % 7 end) end
local s = 0
for _, c in ipairs(cs) do local _, v = coroutine.resume(c); s = s + v end
print(s)
