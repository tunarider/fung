local t = {}
for i = 0, 499999 do t[#t + 1] = tostring(i) end
print(#table.concat(t, ","))
