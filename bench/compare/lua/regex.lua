-- Lua patterns, not regular expressions: Lua has no regex library.
local s = 0
for l in io.lines("data/logs.txt") do s = s + tonumber(l:match("latency_ms=(%d+)")) end
print(s)
