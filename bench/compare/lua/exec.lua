-- os.execute goes through /bin/sh: one extra process per command.
local s = 0
for _ = 1, 500 do local _, _, code = os.execute("true"); s = s + code end
print(s)
