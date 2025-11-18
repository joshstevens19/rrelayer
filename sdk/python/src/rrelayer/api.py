import asyncio
from base64 import b64encode
from typing import Any

import aiohttp
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

    def build_headers(self, baseConfig: dict[str, str]) -> dict[str, str]:
        headers = {
            "Content-Type": "application/json",
        }

        if "apiKey" in baseConfig:
            headers["x-api-key"] = baseConfig["apiKey"]

        elif "username" in baseConfig and "password" in baseConfig:
            credentials = f"{baseConfig['username']}:{baseConfig['password']}"

            headers["Authorization"] = (
                f"Basic {b64encode(credentials.encode()).decode()}"
            )
        else:
            raise ValueError("API::Invalid authentication credentials")

        return headers

    async def _get_session(self):
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession()
        return self._session

    async def getApi(
        self, baseConfig: dict[str, str], endpoint: str, params: dict[str, Any] = {}
    ) -> dict[str, Any]:
        session = await self._get_session()

        headers = self.build_headers(baseConfig)

        async with session.get(
            f"{baseConfig['serverURL']}/{endpoint}", headers=headers, params=params
        ) as response:
            response.raise_for_status()
            return await response.json()

    async def postApi(
        self, baseConfig: dict[str, str], endpoint: str, body: dict[str, Any]
    ) -> dict[str, Any]:
        session = await self._get_session()

        headers = self.build_headers(baseConfig)

        async with session.post(
            f"{baseConfig['serverURL']}/{endpoint}", headers=headers, json=body
        ) as response:
            response.raise_for_status()
            return await response.json()

    async def putApi(
        self, baseConfig: dict[str, str], endpoint: str, body: dict[str, Any]
    ) -> dict[str, Any]:
        session = await self._get_session()

        headers = self.build_headers(baseConfig)

        async with session.put(
            f"{baseConfig['serverURL']}/{endpoint}", headers=headers, json=body
        ) as response:
            response.raise_for_status()
            return await response.json()

    async def deleteApi(
        self, baseConfig: dict[str, str], endpoint: str, body: dict = {}
    ) -> dict[str, Any]:
        session = await self._get_session()

        headers = self.build_headers(baseConfig)

        async with session.delete(
            f"{baseConfig['serverURL']}/{endpoint}", headers=headers, json=body
        ) as response:
            response.raise_for_status()
            return await response.json()

    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()
