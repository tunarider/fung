local s = 0
for i = 0, 2999999 do s = s + i * i % 7 end
print(s)
