import json
with open("data/data.json") as f:
    rows = json.load(f)
ids = sum(r["id"] for r in rows if r["active"])
tags = sum(len(r["tags"]) for r in rows)
print(ids, tags, "true" if len(json.dumps(rows)) > 1000000 else "false")
