import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';

export interface UseUpdateMaxGasPriceRelayerOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useUpdateMaxGasPriceRelayer = ({
  onSuccess,
  onError,
}: UseUpdateMaxGasPriceRelayerOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const updateMaxGasPriceRelayer = useCallback(
    async (relayerId: string, cap: string) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        const relayerClient = await sdk.admin.relayer.createRelayerClient(
          relayerId
        );

        await relayerClient.updateMaxGasPrice(cap);

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

  return updateMaxGasPriceRelayer;
};
