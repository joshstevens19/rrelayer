import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';
import { CreateRelayerResult } from 'rrelayer-sdk';

export interface UseCreateRelayerOptions {
  onSuccess?: (result: CreateRelayerResult) => void;
  onError?: (error: string) => void;
}

export const useCreateNewRelayer = ({
  onSuccess,
  onError,
}: UseCreateRelayerOptions) => {
  const sdk = useContext(RRelayerSDKContext);

  const createNewRelayer = useCallback(
    async (name: string, chainId: number) => {
      if (!sdk) {
        throw new Error('RRelayerSDKContext is undefined');
      }

      try {
        const response = await sdk.admin.relayer.createNewRelayer(
          chainId,
          name
        );

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

  return createNewRelayer;
};
