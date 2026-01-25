import asyncio

from __PLAYGROUND__.helper import begin, end


async def getNetwork():
    try:
        client, _, _, rrelayer_node = await begin()

        networks = await client.network.get(31337)
        print("networks", networks)
    except Exception as e:
        print("getNetwork failed:", e)
    finally:
        end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getNetwork())
    print("get-network done")
