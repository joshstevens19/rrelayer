import asyncio
from datetime import datetime

from __PLAYGROUND__.helper import begin, end


async def cloneRelayer():
    rrelayer_node = None

    try:
        client, _, info, rrelayer_node = await begin()

        print("Creating new relayer...")
        relayer = await client.relayer.clone(
            info["id"], 31337, f"test-relayer-{datetime.now()}"
        )

        print("Created relayer", relayer)

        await client.relayer.delete(relayer["id"])
        print("Test relayer cleaned up")
    except Exception as e:
        print("cloneRelayer failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(cloneRelayer())
    print("clone-relayer done")
