import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';
import { TokenPair } from 'rrelayer-sdk/dist/api/authentication/types';

export interface UseAuthenticateOptions {
  onSuccess?: (tokenPair: TokenPair) => void;
  onError?: (error: string) => void;
}

export const useAuthenticate = ({
  onSuccess,
  onError,
}: UseAuthenticateOptions) => {
  const sdk = useContext(RRelayerSDKContext);

  const authenticate = useCallback(async () => {
    if (!sdk) {
      throw new Error('RRelayerSDKContext is undefined');
    }

    try {
      const response = await sdk.admin.authentication.authenticate();
      localStorage.setItem(
        'rrelayer__authentication',
        JSON.stringify(response)
      );

      if (onSuccess) {
        onSuccess(response);
      }
    } catch (error: any) {
      if (onError) {
        onError(error.message);
      }
    }
  }, [sdk, onSuccess, onError]);

  return authenticate;
};
