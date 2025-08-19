import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
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
  const sdk = useContext(RRelayerrSDKContext);

  const authenticate = useCallback(async () => {
    if (!sdk) {
      throw new Error('RRelayerrSDKContext is undefined');
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
