// An unbuffered channel: send waits until a receiver takes the value.
class Chan {
  constructor() { this.senders = []; this.receivers = []; this.closed = false; }
  send(v) {
    return new Promise(res => {
      const r = this.receivers.shift();
      if (r) { r({ v, done: false }); res(); } else this.senders.push({ v, res });
    });
  }
  close() { this.closed = true; for (const r of this.receivers.splice(0)) r({ done: true }); }
  recv() {
    return new Promise(res => {
      const s = this.senders.shift();
      if (s) { s.res(); res({ v: s.v, done: false }); }
      else if (this.closed) res({ done: true });
      else this.receivers.push(res);
    });
  }
}
(async () => {
  const c = new Chan();
  (async () => { for (let i = 0; i < 100000; i++) await c.send(i); c.close(); })();
  let s = 0;
  for (;;) { const { v, done } = await c.recv(); if (done) break; s += v; }
  console.log(s);
})();
