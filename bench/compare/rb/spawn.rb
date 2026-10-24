fs = (0...2000).map { |i| Fiber.new { i * i % 7 } }
puts fs.sum(&:resume)
