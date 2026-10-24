let s = 0;
for (let i = 0; i < 3000000; i++) s += i * i % 7;
console.log(s);
