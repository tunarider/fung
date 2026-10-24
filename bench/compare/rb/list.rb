xs = (0...500000).map { |i| i * 7 % 1000 }
ys = xs.map { |x| x + 1 }.select(&:even?)
zs = xs.sort_by { |x| -x }
puts ys.sum + zs[0] + ys.length
