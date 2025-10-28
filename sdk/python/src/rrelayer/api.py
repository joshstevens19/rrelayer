import aiohttp
import asyncio
from pydantic import BaseModel, ConfigDict, PrivateAttr


class API(BaseModel):
    _session: aiohttp.ClientSession | None = PrivateAttr(default=None)

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def __del__(self):
        if self._session and not self._session.closed:
            try:
                loop = asyncio.get_running_loop()
                loop.create_task(self._session.close())
            except RuntimeError:
                # No running event loop (e.g., program exit) — close synchronously
                asyncio.run(self._session.close())
        print("Destroyed API Session")

    async def _get_session(self):
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession()
        return self._session

    async def getApi(self, url: str) -> dict:
        session = await self._get_session()
        async with session.get(url) as response:
            print(response.content_type)
            return await response.json()

    async def postApi(self, url: str, headers, body) -> dict:
        session = await self._get_session()

        async with session.post(url) as response:
            return await response.json()

    async def putApi(self, url: str, headers, body) -> dict:
        session = await self._get_session()

        async with session.put(url) as response:
            return await response.json()

    async def deleteApi(self, url: str, headers, body) -> dict:
        session = await self._get_session()

        async with session.delete(url) as response:
            return await response.json()

    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()
