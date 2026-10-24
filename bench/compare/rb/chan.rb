# A producer fiber hands one value per resume: cooperative, like Lua.
prod = Fiber.new do
  100000.times { |i| Fiber.yield i }
  nil
end
s = 0
while (x = prod.resume)
  s += x
end
puts s
