const fs = require("fs");
const counts = new Map();
let err = 0;
const text = fs.readFileSync("data/logs.txt", "utf8");
for (const l of text.split("\n")) {
  if (l === "") continue;
  const p = l.split(" ");
  const lv = p[1];
  const ms = parseInt(p[3].split("=")[1], 10);
  counts.set(lv, (counts.get(lv) || 0) + 1);
  if (lv === "ERROR") err += ms;
}
for (const k of [...counts.keys()].sort()) console.log(k, counts.get(k));
console.log(err);
