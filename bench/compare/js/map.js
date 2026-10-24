const n = 200000;
const m = new Map();
for (let i = 0; i < n; i++) m.set(`k${i}`, i);
let s = 0;
for (let i = 0; i < n; i++) s += m.get(`k${i}`);
console.log(s);
