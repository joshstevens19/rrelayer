import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';
import { Address } from 'viem';

export interface UseDeleteUserOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useDeleteUser = ({
  onSuccess,
  onError,
}: UseDeleteUserOptions = {}) => {
  const sdk = useContext(RRelayerSDKContext);

  const deleteUser = useCallback(
    async (user: Address) => {
      if (!sdk) {
        throw new Error('RRelayerSDKContext is undefined');
      }

      try {
        await sdk.admin.user.delete(user);

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

  return deleteUser;
};
