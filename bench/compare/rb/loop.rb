s = 0
i = 0
while i < 3000000
  s += i * i % 7
  i += 1
end
puts s
