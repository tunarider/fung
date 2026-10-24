const { spawnSync } = require("child_process");
let s = 0;
for (let i = 0; i < 500; i++) s += spawnSync("true").status;
console.log(s);
