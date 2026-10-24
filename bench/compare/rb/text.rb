counts = Hash.new(0)
err = 0
File.foreach("data/logs.txt") do |l|
  p = l.chomp.split(" ")
  lv = p[1]
  ms = p[3].split("=")[1].to_i
  counts[lv] += 1
  err += ms if lv == "ERROR"
end
counts.keys.sort.each { |k| puts "#{k} #{counts[k]}" }
puts err
