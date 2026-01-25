import asyncio

from __PLAYGROUND__.helper import begin, end


async def signTypedData():
    rrelayer_node = None
    try:
        _, relayer, info, rrelayer_node = await begin()

        print("Signing typed data...")

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

        signature = await relayer.sign.typedData(typed_data)

        print("Domain:", typed_data["domain"])
        print("Types", typed_data["types"])
        print("Value", typed_data["message"])
        print("Signature", signature)

    except Exception as e:
        print("signText failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(signTypedData())
    print("sign-typed-data done")
