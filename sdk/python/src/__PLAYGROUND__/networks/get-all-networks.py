import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAllNetworks():
    try:
        client, _, _, rrelayer_node = await begin()

        networks = await client.network.getAll()
        print("networks", networks)
    except Exception as e:
        print("getAllNetworks failed:", e)
    finally:
        end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAllNetworks())
    print("get-all-networks done")
