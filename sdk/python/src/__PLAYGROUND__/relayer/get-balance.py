import asyncio

from __PLAYGROUND__.helper import begin, end


async def getBalance():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Getting relayer balance...")
        balance = await relayer.getBalanceOf()
        print("Relayer balance:", balance, "ETH")
    except Exception as e:
        print("getBalance failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getBalance())
    print("get-balance done")
