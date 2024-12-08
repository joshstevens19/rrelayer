import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { Network } from 'rrelayerr-sdk';

export const useNetworks = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [loading, setLoading] = useState(false);
  const [networks, setNetworks] = useState<Network[] | null>(null);

  const getNetworks = useCallback(async () => {
    setLoading(true);

    if (!sdk) {
      const error = new Error('RRelayerrSDKContext is undefined');
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
