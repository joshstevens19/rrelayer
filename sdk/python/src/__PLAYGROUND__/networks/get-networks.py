import asyncio

from __PLAYGROUND__.helper import begin, end


async def getNetwork():
    client, _, rrelayer_node = await begin()

    networks = await client.network.get(31337)
    print("networks", networks)

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getNetwork())
    print("get-network done")
