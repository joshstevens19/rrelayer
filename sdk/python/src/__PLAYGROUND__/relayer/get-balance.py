import asyncio

from __PLAYGROUND__.helper import begin, end


async def pauseUnpause():
    _, relayer, rrelayer_node = await begin()

    print("Pausing relayer...")
    await relayer.pause()
    print("Relayer balance:", balance, "ETH")

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(pauseUnpause())
    print("pause-unpause done")
