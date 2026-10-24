require "json"
rows = JSON.parse(File.read("data/data.json"))
ids = rows.sum { |r| r["active"] ? r["id"] : 0 }
tags = rows.sum { |r| r["tags"].length }
puts "#{ids} #{tags} #{JSON.generate(rows).length > 1000000}"
