const fs = require("fs");
let n = 0;
for (let i = 0; i < 25000; i++) n += fs.existsSync("/") + (fs.statSync("/") != null);
console.log(n);
