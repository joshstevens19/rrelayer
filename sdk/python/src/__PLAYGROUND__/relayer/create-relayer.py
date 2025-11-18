import asyncio
from datetime import datetime

from __PLAYGROUND__.helper import begin, end


async def createRelayer():
    client, _, rrelayer_node = await begin()

    print("Creating new relayer...")

    relayer = await client.relayer.create(31337, f"test-relayer-{datetime.now()}")

    print("Created relayer:", relayer)

    # Clean up - delete the test relayer

    await client.relayer.delete(relayer["id"])
    print("Test relayer cleaned up")

    end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(createRelayer())
    print("create-relayer done")
