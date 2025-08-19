import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';

export interface UsePauseRelayerToggleOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export enum DisableNetworkToggleType {
  Disable = 'disable',
  Enable = 'enable',
}

export const useDisableNetworkToggle = ({
  onSuccess,
  onError,
}: UsePauseRelayerToggleOptions = {}) => {
  const sdk = useContext(RRelayerSDKContext);

  const pauseRelayerToggle = useCallback(
    async (chainId: string, toggleType: DisableNetworkToggleType) => {
      if (!sdk) {
        throw new Error('RRelayerSDKContext is undefined');
      }

      try {
        toggleType === DisableNetworkToggleType.Disable
          ? await sdk.admin.networks.disable(chainId)
          : await sdk.admin.networks.enable(chainId);

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

  return pauseRelayerToggle;
};
