import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext } from 'react';

export interface UsePauseRelayerToggleOptions {
  onSuccess?: () => void;
  onError?: (error: string) => void;
}

export enum PauseRelayerToggleType {
  Pause = 'pause',
  Unpause = 'unpause',
}

export const usePauseRelayerToggle = ({
  onSuccess,
  onError,
}: UsePauseRelayerToggleOptions = {}) => {
  const sdk = useContext(RRelayerSDKContext);

  const pauseRelayerToggle = useCallback(
    async (relayerId: string, toggleType: PauseRelayerToggleType) => {
      if (!sdk) {
        throw new Error('RRelayerSDKContext is undefined');
      }

      try {
        const relayerClient = await sdk.admin.relayer.createRelayerClient(
          relayerId
        );

        toggleType === PauseRelayerToggleType.Pause
          ? await relayerClient.pause()
          : await relayerClient.unpause();

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
