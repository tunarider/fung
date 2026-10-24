import asyncio
async def main():
    q = asyncio.Queue(maxsize=1)
    done = object()
    async def prod():
        for i in range(100000):
            await q.put(i)
        await q.put(done)
    t = asyncio.create_task(prod())
    s = 0
    while (x := await q.get()) is not done:
        s += x
    await t
    print(s)
asyncio.run(main())
