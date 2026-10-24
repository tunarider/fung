const xs = Array.from({ length: 500000 }, (_, i) => i * 7 % 1000);
const ys = xs.map(x => x + 1).filter(x => x % 2 === 0);
const zs = [...xs].sort((a, b) => b - a);
console.log(ys.reduce((a, b) => a + b, 0) + zs[0] + ys.length);
