from pydantic import BaseModel, validate_call, ConfigDict, PrivateAttr
from rrelayer.api import API


class Client(BaseModel):
    _serverURL: str = PrivateAttr()
    _auth_username: str = PrivateAttr()
    _auth_password: str = PrivateAttr()
    _api: API = PrivateAttr()

    relayer: "Client.Relayer" = None
    network: "Client.Network | None" = None
    transaction: "Client.Transaction | None" = None
    allowlist: "Client.AllowList | None" = None

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def __init__(self, serverURL: str, auth_username: str, auth_password: str, **data):
        super().__init__(**data)

        self._serverURL = serverURL
        self._auth_username = auth_username
        self._auth_password = auth_password

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
            pass

        @validate_call
        async def clone(self, relayerId: str, chainId: int, name: str):
            pass

        @validate_call
        async def delete(self, id: str):
            pass

        @validate_call
        async def get(self, id: str):
            pass

        @validate_call
        async def getAll(self):
            pass

        def printValues(self):
            print("Print Values")
            print(self._client._auth_username)

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
