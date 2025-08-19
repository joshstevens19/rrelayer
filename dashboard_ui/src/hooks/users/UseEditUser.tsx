import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';
import { JwtRole } from 'rrelayer-sdk/dist/api/authentication/types';
import { Address } from 'viem';

export interface UseEditUserOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useEditUser = ({
  onSuccess,
  onError,
}: UseEditUserOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const editUser = useCallback(
    async (user: Address, role: JwtRole) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        await sdk.admin.user.edit(user, role);

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

  return editUser;
};
