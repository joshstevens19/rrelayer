import asyncio

from __PLAYGROUND__.helper import begin, end


async def updateEip1559():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Updating EIP1559 status to True...")
        await relayer.updateEIP1559Status(True)
        print("EIP1559 status updated to True")

        print("Updating EIP1559 status to False...")
        await relayer.updateEIP1559Status(False)
        print("EIP1559 status updated to False")

    except Exception as e:
        print("updateEip1559 failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(updateEip1559())
    print("update-eip1559 done")
