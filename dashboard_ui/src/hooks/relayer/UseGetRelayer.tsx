import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { Relayer } from 'rrelayer-sdk';

export const useGetRelayer = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [loading, setLoading] = useState(false);
  const [relayer, setRelayer] = useState<Relayer | null>(null);

  const getRelayer = useCallback(
    async (relayerId: string) => {
      setLoading(true);

      if (!sdk) {
        const error = new Error('RRelayerrSDKContext is undefined');
        setLoading(false);
        throw error;
      }

      try {
        const response = await sdk.admin.relayer.get(relayerId);
        if (!response) {
          setLoading(false);
          return null;
        }

        setRelayer(response.relayer);
        setLoading(false);
        return response;
      } catch (error: any) {
        setLoading(false);
        throw error;
      }
    },
    [sdk]
  );

  return { getRelayer, loading, relayer };
};
