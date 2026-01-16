import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAllowlist():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Getting relayer allowlist...")
        allowlists = await relayer.allowlist.get()
        print("AllowLists:", allowlists)
    except Exception as e:
        print("getAllowlist failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAllowlist())
    print("get-allowlist done")
