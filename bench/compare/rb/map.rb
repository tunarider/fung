n = 200000
m = {}
n.times { |i| m["k#{i}"] = i }
s = 0
n.times { |i| s += m["k#{i}"] }
puts s
