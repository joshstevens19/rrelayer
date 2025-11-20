import asyncio

from __PLAYGROUND__.helper import begin, end


async def signTypedDataHistory():
    rrelayer_node = None
    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Getting typed signing text history...")
        result = await relayer.sign.typedDataHistory({"limit": 100, "offset": 0})

        print("result:", result)

    except Exception as e:
        print("signTypedDataHistory failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(signTypedDataHistory())
    print("sign-typed-data-history done")
