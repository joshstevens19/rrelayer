import asyncio

from __PLAYGROUND__.helper import begin, end


async def signTextHistory():
    rrelayer_node = None
    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Getting signing text history...")
        result = await relayer.sign.textHistory({"limit": 100, "offset": 0})

        print("result:", result)

    except Exception as e:
        print("signTextHistory failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(signTextHistory())
    print("sign-text-history done")
