(async () => {
  const work = async i => i * i % 7;
  const ts = Array.from({ length: 2000 }, (_, i) => work(i));
  let s = 0;
  for (const t of ts) s += await t;
  console.log(s);
})();
