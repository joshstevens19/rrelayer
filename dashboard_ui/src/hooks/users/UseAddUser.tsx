import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';
import { JwtRole } from 'rrelayerr-sdk/dist/api/authentication/types';
import { Address } from 'viem';

export interface UseAddUserOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useAddUser = ({ onSuccess, onError }: UseAddUserOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const addUser = useCallback(
    async (user: Address, role: JwtRole) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        await sdk.admin.user.add(user, role);

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

  return addUser;
};
