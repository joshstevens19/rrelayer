import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAllRelayers():
    client, _, rrelayer_node = await begin()

    print("Getting all relayers...")
    relayers = await client.relayer.getAll()
    print("All relayers:", relayers)

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAllRelayers())
    print("get-all-relayers done")
