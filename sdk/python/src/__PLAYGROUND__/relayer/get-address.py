import asyncio
from datetime import datetime

from __PLAYGROUND__.helper import begin, end


async def getAddress():
    _, relayer, rrelayer_node = await begin()

    print("Getting relayer address...")
    address = await relayer.address()
    print("Relayer address:", address)

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getAddress())
    print("get-address done")
