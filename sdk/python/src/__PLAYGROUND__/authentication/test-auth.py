import asyncio

from __PLAYGROUND__.helper import begin, end


async def testAuth():
    client, _, rrelayer_node = await begin()

    print("Testing authentication...")

    try:
        networks = await client.network.getAll()
        print("Authentication successful - got networks:", networks)
    except Exception as e:
        print("Authentication failed:", e)

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(testAuth())
    print("test-auth done")
