import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';

export interface UseUpdateEIP1559RelayerStatusOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useUpdateEIP1559RelayerStatus = ({
  onSuccess,
  onError,
}: UseUpdateEIP1559RelayerStatusOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const updateEIP1559RelayerStatus = useCallback(
    async (relayerId: string, status: boolean) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
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
