from typing import Any
from pydantic import BaseModel, validate_call, ConfigDict, PrivateAttr
from rrelayer.api import API


class Client(BaseModel):
    _apiBaseConfig: dict[str, str] = PrivateAttr()

    _api: API = PrivateAttr()

    relayer: "Client.Relayer | None" = None
    network: "Client.Network | None" = None
    transaction: "Client.Transaction | None" = None
    allowlist: "Client.AllowList | None" = None

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def __init__(self, serverURL: str, auth_username: str, auth_password: str, **data):
        super().__init__(**data)

        self._apiBaseConfig = {
            "serverURL": serverURL,
            "username": auth_username,
            "password": auth_password,
        }

        self._api = API()

        self.relayer = self.Relayer(self)
        self.network = self.Network(self)
        self.transaction = self.Transaction(self)
        self.allowlist = self.AllowList(self)

    class Relayer:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

        @validate_call
        async def create(self, chainId: int, name: str):
            return await self._client._api.postApi(
                self._client._apiBaseConfig, f"relayers/{chainId}/new", {"name": name}
            )

        @validate_call
        async def clone(self, relayerId: str, chainId: int, name: str):
            return await self._client._api.postApi(
                self._client._apiBaseConfig,
                f"relayers/{relayerId}/clone",
                {"newRelayerName": name, "chainId": chainId},
            )

        @validate_call
        async def delete(self, id: str):
            _ = await self._client._api.deleteApi(
                self._client._apiBaseConfig, f"relayers/{id}"
            )

        @validate_call
        async def get(self, id: str) -> dict[str, Any]:
            return await self._client._api.getApi(
                self._client._apiBaseConfig,
                f"relayers/{id}",
            )

        @validate_call
        async def getAll(self):
            pass

    class Network:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

    class Transaction:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

    class AllowList:
        def __init__(self, client: "Client"):
            self._client: "Client" = client


@validate_call
def createClient(serverURL: str, auth_username: str, auth_password: str) -> Client:
    return Client(
        serverURL=serverURL,
        auth_username=auth_username,
        auth_password=auth_password,
    )
