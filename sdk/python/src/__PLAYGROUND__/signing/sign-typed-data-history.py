import asyncio

from __PLAYGROUND__.helper import begin, end


async def signTypedDataHistory():
    rrelayer_node = None
    try:
        _, relayer, info, rrelayer_node = await begin()

        typed_data = {
            "types": {
                "Person": [
                    {"name": "name", "type": "string"},
                    {"name": "wallet", "type": "address"},
                ],
            },
            "primaryType": "Person",
            "domain": {
                "name": "Test App",
                "version": "1",
                "chainId": 31337,
                "verifyingContract": "0x1234567890123456789012345678901234567890",
            },
            "message": {"name": "Alice", "wallet": info["address"]},
        }

        await relayer.sign.typedData(typed_data)

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
