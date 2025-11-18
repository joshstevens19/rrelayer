import asyncio

from __PLAYGROUND__.helper import begin, end


async def getAllowlist():
    _, relayer, rrelayer_node = await begin()

    print("Getting relayer allowlist...")
    allowlists = await relayer.allowlist.get()
    print("AllowLists:", allowlists)

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAllowlist())
    print("get-allowlist done")
