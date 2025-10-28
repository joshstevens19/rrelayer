from pydantic import BaseModel, ConfigDict, PrivateAttr
from asyncinit import asyncinit
from web3 import AsyncWeb3


@asyncinit
class Relayer(BaseModel):
    _id: str = PrivateAttr()

    _apiBaseConfig: dict = PrivateAttr()
    model_config = ConfigDict(arbitrary_types_allowed=True)
    _ethereumProvider: AsyncWeb3 | None = None

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

        self._ethereumProvider = await AsyncWeb3(
            AsyncWeb3.AsyncHTTPProvider(providerUrl)
        )

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

    @property
    def name(self):
        return self._id
