n = 0
25000.times { n += (File.exist?("/") ? 1 : 0) + (File.stat("/") ? 1 : 0) }
puts n
