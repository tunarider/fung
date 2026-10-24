import asyncio
async def main():
    async def work(i):
        return i * i % 7
    ts = [asyncio.create_task(work(i)) for i in range(2000)]
    s = 0
    for t in ts:
        s += await t
    print(s)
asyncio.run(main())
