import asyncio

from __PLAYGROUND__.helper import begin, end


async def pauseUnpause():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Pausing relayer...")
        await relayer.pause()
        print("Relayer paused")

        print("Unpausing relayer...")
        await relayer.unpause()
        print("Relayer unpaused")
    except Exception as e:
        print("pauseUnpause failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(pauseUnpause())
    print("pause-unpause done")
