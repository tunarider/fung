local prod = coroutine.wrap(function()
  for i = 0, 99999 do coroutine.yield(i) end
  return nil
end)
local s = 0
while true do local x = prod(); if x == nil then break end; s = s + x end
print(s)
