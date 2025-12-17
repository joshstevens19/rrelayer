import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAllRelayers():
    rrelayer_node = None

    try:
        client, _, _, rrelayer_node = await begin()

        print("Getting all relayers...")
        relayers = await client.relayer.getAll()
        print("All relayers:", relayers)
    except Exception as e:
        print("getAllRelayers failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAllRelayers())
    print("get-all-relayers done")
