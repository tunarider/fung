s = 0
File.foreach("data/logs.txt") { |l| s += l[/latency_ms=(\d+)/, 1].to_i }
puts s
