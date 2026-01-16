import asyncio

from __PLAYGROUND__.helper import begin, end


async def getRelayer():
    rrelayer_node = None
    try:
        client, _, info, rrelayer_node = await begin()

        print("Getting relayer info...")
        relayerInfo = await client.relayer.get(info["id"])
        print("Relayer info:", relayerInfo)
    except Exception as e:
        print("getRelayer failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getRelayer())
    print("get-relayer done")
