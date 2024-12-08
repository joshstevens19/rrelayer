import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';

export interface UseCreateNewRelayerApiKeyOptions {
  onSuccess?: (key: string) => void;
  onError?: (error: string) => void;
}

export const useCreateNewRelayerApiKey = ({
  onSuccess,
  onError,
}: UseCreateNewRelayerApiKeyOptions) => {
  const sdk = useContext(RRelayerrSDKContext);

  const createNewRelayerApiKey = useCallback(
    async (relayerId: string) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        const response = await sdk.admin.relayer.apiKeys.create(relayerId);

        if (onSuccess) {
          onSuccess(response);
        }
      } catch (error: any) {
        if (onError) {
          onError(error.message);
        }
      }
    },
    [sdk, onSuccess, onError]
  );

  return createNewRelayerApiKey;
};
