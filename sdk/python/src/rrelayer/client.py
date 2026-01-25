from typing import Any

from pydantic import BaseModel, ConfigDict, PrivateAttr, validate_call

from .admin import AdminRelayerClient
from .api import API
from .relayer import RelayerClient
from .types import PagingContext, defaultPagingContext


class Client(BaseModel):
    class Relayer:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

        @validate_call
        async def create(self, chainId: int, name: str):
            try:
                return await self._client._api.postApi(
                    self._client._apiBaseConfig,
                    f"relayers/{chainId}/new",
                    {"name": name},
                )

            except Exception as error:
                print("Failed to create relayer", error)
                raise error

        @validate_call
        async def clone(self, relayerId: str, chainId: int, name: str):
            try:
                return await self._client._api.postApi(
                    self._client._apiBaseConfig,
                    f"relayers/{relayerId}/clone",
                    {"newRelayerName": name, "chainId": chainId},
                )
            except Exception as error:
                print("Failed to clone relayer", error)
                raise error

        @validate_call
        async def delete(self, id: str):
            try:
                await self._client._api.deleteApi(
                    self._client._apiBaseConfig, f"relayers/{id}"
                )

            except Exception as error:
                print("Failed to delete relayer", error)
                raise error

        @validate_call
        async def get(self, id: str) -> dict[str, Any]:
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, f"relayers/{id}"
                )

            except Exception as error:
                print("Failed to fetch getRelayer", error)
                raise error

        @validate_call
        async def getAll(
            self,
            pagingContext: PagingContext = defaultPagingContext,
            onlyForChainId: int | None = None,
        ):
            try:
                params = pagingContext.model_dump()

                if onlyForChainId:
                    params["chainId"] = onlyForChainId

                return await self._client._api.getApi(
                    self._client._apiBaseConfig, "relayers", params
                )

            except Exception as error:
                print("Failed to fetch getRelayers", error)
                raise error

    class Network:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

        @validate_call
        async def get(self, chainId: int):
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, f"networks/{chainId}"
                )

            except Exception as error:
                print("Failed to fetch all networks:", error)
                raise error

        @validate_call
        async def getAll(self):
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, "networks"
                )
            except Exception as error:
                print("Failed to fetch all networks:", error)
                raise error

        @validate_call
        async def getGasPrices(self, chainId: int):
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, f"networks/gas/price/{chainId}"
                )
            except Exception as error:
                print("Failed to fetch gas prices:", error)
                raise error

    class Transaction:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

        @validate_call
        async def get(self, transactionId: str):
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, f"transactions/{transactionId}"
                )
            except Exception as e:
                print("Failed to fetch getTransaction:", e)

        @validate_call
        async def getStatus(self, transactionId: str):
            try:
                return await self._client._api.getApi(
                    self._client._apiBaseConfig, f"transactions/status/{transactionId}"
                )
            except Exception as error:
                print("Failed to fetch getTransactionStatus:", error)
                raise error

        @validate_call
        async def sendRandom(self):
            pass

    class AllowList:
        def __init__(self, client: "Client"):
            self._client: "Client" = client

        @validate_call
        async def get(
            self, relayerId: str, pagingContext: PagingContext = defaultPagingContext
        ):
            try:
                params = pagingContext.model_dump()

                return await self._client._api.getApi(
                    self._client._apiBaseConfig,
                    f"relayers/{relayerId}/allowlists",
                    params,
                )
            except Exception as error:
                print("Failed to getRelayerAllowlistAddress:", error)
                raise error

    _apiBaseConfig: dict[str, str] = PrivateAttr()

    _api: API = PrivateAttr()

    _allowlist: AllowList = PrivateAttr()
    _network: Network = PrivateAttr()
    _relayer: Relayer = PrivateAttr()
    _transaction: Transaction = PrivateAttr()

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def __init__(self, serverURL: str, auth_username: str, auth_password: str, **data):
        super().__init__(**data)

        self._apiBaseConfig = {
            "serverURL": serverURL,
            "username": auth_username,
            "password": auth_password,
        }

        self._api = API()

        self._allowlist = self.AllowList(self)
        self._network = self.Network(self)
        self._relayer = self.Relayer(self)
        self._transaction = self.Transaction(self)

    @property
    def allowlist(self) -> AllowList:
        return self._allowlist

    @property
    def network(self) -> Network:
        return self._network

    @property
    def relayer(self) -> Relayer:
        return self._relayer

    @property
    def transaction(self) -> Transaction:
        return self._transaction

    @validate_call
    async def getRelayerClient(
        self, relayerId: str, providerURL: str, defaultSpeed: None = None
    ) -> AdminRelayerClient:
        relayer = await self._relayer.get(relayerId)
        if relayer:
            # TODO
            pass

        auth = {
            "username": self._apiBaseConfig["username"],
            "password": self._apiBaseConfig["password"],
        }

        return AdminRelayerClient(
            self._apiBaseConfig["serverURL"], providerURL, relayerId, auth
        )


@validate_call
def createClient(serverURL: str, auth_username: str, auth_password: str) -> Client:
    return Client(
        serverURL=serverURL,
        auth_username=auth_username,
        auth_password=auth_password,
    )


@validate_call
def createRelayerClient(
    serverURL: str,
    providerURL: str,
    relayerId: str,
    apiKey: str,
) -> RelayerClient:
    auth = {
        "apiKey": apiKey,
    }

    return RelayerClient(serverURL, providerURL, relayerId, auth)
