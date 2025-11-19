import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAddress():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Getting relayer address...")
        address = await relayer.address()
        print("Relayer address:", address)
    except Exception as e:
        print("getAddress failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAddress())
    print("get-address done")
