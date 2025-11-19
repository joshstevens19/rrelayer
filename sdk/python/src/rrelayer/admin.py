from pydantic import validate_call

from rrelayer.relayer import RelayerClient


class AdminRelayerClient(RelayerClient):
    class AdminTransaction(RelayerClient.Transaction):
        def __init__(self, object):
            super().__init__(object)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self._admin_transaction = AdminRelayerClient.AdminTransaction(self)

    @property
    def transaction(self) -> AdminTransaction:
        return self._admin_transaction

    async def pause(self):
        try:
            await self._api.putApi(
                self._apiBaseConfig, f"relayers/{self._id}/pause", {}
            )
        except Exception as error:
            print("Failed to pauseRelayer:", error)
            raise error

    async def unpause(self):
        try:
            await self._api.putApi(
                self._apiBaseConfig, f"relayers/{self._id}/unpause", {}
            )
        except Exception as error:
            print("Failed to unpauseRelayer:", error)
            raise error

    @validate_call
    async def updateEIP1559Status(self, status: bool):
        try:
            await self._api.putApi(
                self._apiBaseConfig, f"relayers/{self._id}/gas/eip1559/{status}", {}
            )
        except Exception as error:
            print("Failed to updateRelayerEIP1559Status:", error)
            raise error

    @validate_call
    async def updateMaxGasPrice(self, cap: str):
        try:
            await self._api.putApi(
                self._apiBaseConfig, f"relayers/{self._id}/gas/max/{cap}", {}
            )
        except Exception as error:
            print("Failed to updateRelayerEIP1559Status:", error)
            raise error

    async def removeMaxGasPrice(self):
        try:
            await self._api.putApi(
                self._apiBaseConfig, f"relayers/{self._id}/gas/max/0", {}
            )
        except Exception as error:
            print("Failed to removeRelayerMaxGasPrice:", error)
            raise error

    @validate_call
    async def clone(self, chainId: int, name: str):
        pass
