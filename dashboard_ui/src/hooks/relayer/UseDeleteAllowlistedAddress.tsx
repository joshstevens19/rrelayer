import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';
import { Address } from 'viem';

export interface UseDeleteAllowlistedAddressOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useDeleteAllowlistedAddress = ({
  onSuccess,
  onError,
}: UseDeleteAllowlistedAddressOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const deleteAllowlistedAddress = useCallback(
    async (relayerId: string, address: Address) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        await (
          await sdk.admin.relayer.createRelayerClient(relayerId)
        ).allowlist.delete(address);

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

  return deleteAllowlistedAddress;
};
