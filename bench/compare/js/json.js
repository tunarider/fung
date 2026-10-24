const fs = require("fs");
const rows = JSON.parse(fs.readFileSync("data/data.json", "utf8"));
let ids = 0, tags = 0;
for (const r of rows) { if (r.active) ids += r.id; tags += r.tags.length; }
console.log(ids, tags, JSON.stringify(rows).length > 1000000);
