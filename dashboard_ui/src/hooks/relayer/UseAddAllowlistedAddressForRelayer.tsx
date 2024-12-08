import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext } from 'react';
import { Address } from 'viem';

export interface UseAddAllowlistedAddressForRelayerOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export const useAddAllowlistedAddressForRelayer = ({
  onSuccess,
  onError,
}: UseAddAllowlistedAddressForRelayerOptions = {}) => {
  const sdk = useContext(RRelayerrSDKContext);

  const addAllowlistedAddress = useCallback(
    async (relayerId: string, address: Address) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      try {
        await (
          await sdk.admin.relayer.createRelayerClient(relayerId)
        ).allowlist.add(address);

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

  return addAllowlistedAddress;
};
