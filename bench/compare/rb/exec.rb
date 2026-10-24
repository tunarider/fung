s = 0
500.times { system("true"); s += $?.exitstatus }
puts s
