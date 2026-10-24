local counts, err = {}, 0
for l in io.lines("data/logs.txt") do
  local p = {}
  for w in l:gmatch("[^ ]+") do p[#p + 1] = w end
  local lv = p[2]
  local ms = tonumber(p[4]:match("=(.*)"))
  counts[lv] = (counts[lv] or 0) + 1
  if lv == "ERROR" then err = err + ms end
end
local ks = {}
for k in pairs(counts) do ks[#ks + 1] = k end
table.sort(ks)
for _, k in ipairs(ks) do print(k, counts[k]) end
print(err)
