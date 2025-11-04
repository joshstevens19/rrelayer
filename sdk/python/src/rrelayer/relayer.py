from pydantic import BaseModel, ConfigDict, PrivateAttr
from web3 import AsyncWeb3, Web3
from typing import Any
from rrelayer.api import API


class RelayerClient(BaseModel):
    class AllowList:
        def __init__(self, relayerClient: "RelayerClient"):
            self._relayer: "RelayerClient" = relayerClient

        async def get(self):
            pass

    class Sign:
        def __init__(self, relayerClient: "RelayerClient"):
            self._relayer: "RelayerClient" = relayerClient

    class Transaction:
        def __init__(self, relayerClient: "RelayerClient"):
            self._relayer: "RelayerClient" = relayerClient

        async def get(self):
            pass

        async def getStatus(self):
            pass

        async def getAll(self):
            pass

        async def replace(self):
            pass

        async def cancel(self):
            pass

        async def send(self):
            pass

    _id: str = PrivateAttr()

    _apiBaseConfig: dict[str, str] = PrivateAttr()
    _ethereumProvider: AsyncWeb3 = PrivateAttr()

    _api: API = PrivateAttr()

    _allowlist: AllowList = PrivateAttr()
    _sign: Sign = PrivateAttr()
    _transaction: Transaction = PrivateAttr()

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def __init__(
        self,
        serverURL: str,
        providerUrl: str,
        relayerId: str,
        auth: dict[str, str],
        **data,
    ):
        super().__init__(**data)

        self._id = relayerId

        self._ethereumProvider = AsyncWeb3(AsyncWeb3.AsyncHTTPProvider(providerUrl))

        if "apiKey" in auth:
            self._apiBaseConfig = {"apiKey": auth["apiKey"], "serverURL": serverURL}
        elif "username" in auth and "password" in auth:
            self._apiBaseConfig = {
                "username": auth["username"],
                "password": auth["password"],
                "serverURL": serverURL,
            }
        else:
            raise ValueError("Invalid authentication credentials")

        self._api = API()

        self._allowlist = self.AllowList(self)
        self._sign = self.Sign(self)
        self._transaction = self.Transaction(self)

    @property
    def name(self):
        return self._id

    @property
    def allowlist(self) -> AllowList:
        return self._allowlist

    @property
    def sign(self) -> Sign:
        return self._sign

    @property
    def transaction(self) -> Transaction:
        return self._transaction

    async def address(self) -> str | None:
        response = await self.getInfo()
        return Web3.to_checksum_address(response["address"]) if response else None

    async def getInfo(self):
        response = await self._api.getApi(self._apiBaseConfig, f"relayers/{self._id}")
        return response["relayer"] if response else None

    async def getBalanceOf(self):
        address = await self.address()
        if address:
            balance = await self._ethereumProvider.eth.get_balance(
                Web3.to_checksum_address(address)
            )
            return Web3.from_wei(balance, "ether")
        else:
            return 0

    def ethereumProvider(self) -> AsyncWeb3[Any]:
        return self._ethereumProvider
