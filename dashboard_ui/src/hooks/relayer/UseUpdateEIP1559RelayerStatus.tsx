import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';

export interface UseUpdateEIP1559RelayerStatusOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useUpdateEIP1559RelayerStatus = ({
  onSuccess,
  onError,
}: UseUpdateEIP1559RelayerStatusOptions = {}) => {
  const sdk = useContext(RRelayerSDKContext);

  const updateEIP1559RelayerStatus = useCallback(
    async (relayerId: string, status: boolean) => {
      if (!sdk) {
        throw new Error('RRelayerSDKContext is undefined');
      }

      try {
        const relayerClient = await sdk.admin.relayer.createRelayerClient(
          relayerId
        );

        await relayerClient.updateEIP1559Status(status);

        if (onSuccess) {
          onSuccess();
        }
      } catch (error: any) {
        if (onError) {
          onError(error.message);
        }
      }
    },
    [sdk, onSuccess, onError]
  );

  return updateEIP1559RelayerStatus;
};
