const fs = require("fs");
const pat = /latency_ms=(\d+)/;
let s = 0;
for (const l of fs.readFileSync("data/logs.txt", "utf8").split("\n")) {
  if (l === "") continue;
  s += parseInt(pat.exec(l)[1], 10);
}
console.log(s);
