import asyncio

from __PLAYGROUND__.helper import begin, end


async def getGasPrice():
    try:
        client, _, _, rrelayer_node = await begin()

        print("Getting gas price...")
        gasPrice = await client.network.getGasPrices(31337)
        print("Gas Price:", gasPrice)
    except Exception as e:
        print("getGasPrice failed:", e)
    finally:
        end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(getGasPrice())
    print("get-gas-price done")
