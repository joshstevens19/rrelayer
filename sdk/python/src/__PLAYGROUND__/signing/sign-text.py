import asyncio
from datetime import datetime

from __PLAYGROUND__.helper import begin, end


async def signText():
    rrelayer_node = None
    try:
        _, relayer, _, rrelayer_node = await begin()

        print("Signing text message...")

        message = f"Hello from SDK test at {datetime.now().isoformat()}"
        signature = await relayer.sign.text(message)
        print("Message:", message)
        print("Signature:", signature)
    except Exception as e:
        print("signText failed:", e)
    finally:
        if rrelayer_node is not None:
            end(rrelayer_node)


if __name__ == "__main__":
    asyncio.run(signText())
    print("sign-text done")
