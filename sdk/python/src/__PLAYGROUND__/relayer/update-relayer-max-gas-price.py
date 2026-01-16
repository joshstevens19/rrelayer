import asyncio

from __PLAYGROUND__.helper import begin, end


async def updateMaxGasPrice():
    rrelayer_node = None

    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Setting max gas price to 2 gwei...")
        await relayer.updateMaxGasPrice("2000000000")
        print("Max gas price set to 2 gwei")

        print("Setting max gas price to 5 gwei...")
        await relayer.updateMaxGasPrice("5000000000")
        print("Max gas price set to 5 gwei")

    except Exception as e:
        print("updateMaxGasPrice failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(updateMaxGasPrice())
    print("update-max-gas-price done")
