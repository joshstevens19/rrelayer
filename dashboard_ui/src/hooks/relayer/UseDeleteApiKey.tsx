import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';

export interface UseDeleteRelayerApiOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useDeleteRelayerApiKey = ({
  onSuccess,
  onError,
}: UseDeleteRelayerApiOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const deleteRelayerApiKey = useCallback(
    async (relayerId: string, apiKey: string) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        await sdk.admin.relayer.apiKeys.delete(relayerId, apiKey);

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

  return deleteRelayerApiKey;
};
