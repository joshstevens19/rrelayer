import { RRelayerSDKContext } from '@/contexts/RRelayerSDKContext';
import { useCallback, useContext, useState } from 'react';
import { Network } from 'rrelayer-sdk';

export const useNetworks = () => {
  const sdk = useContext(RRelayerSDKContext);
  const [loading, setLoading] = useState(false);
  const [networks, setNetworks] = useState<Network[] | null>(null);

  const getNetworks = useCallback(async () => {
    setLoading(true);

    if (!sdk) {
      const error = new Error('RRelayerSDKContext is undefined');
      setLoading(false);
      throw error;
    }

    try {
      const response = await sdk.admin.networks.get();
      setNetworks(response);
      setLoading(false);
      return response;
    } catch (error: any) {
      setLoading(false);
      throw error;
    }
  }, [sdk]);

  return { getNetworks, loading, networks };
};
