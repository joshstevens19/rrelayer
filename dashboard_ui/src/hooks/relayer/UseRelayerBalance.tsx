import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useEffect, useState } from 'react';

export const useRelayerBalance = (id: string) => {
  const sdk = useContext(RRelayerrSDKContext);
  const [loading, setLoading] = useState(false);
  const [balance, setBalance] = useState<string | null>(null);

  const getRelayerBalance = useCallback(
    async (relayerId: string) => {
      setLoading(true);

      if (!sdk) {
        const error = new Error('RRelayerrSDKContext is undefined');
        setLoading(false);
        throw error;
      }

      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const balance = await (
        await sdk.admin.relayer.createRelayerClient(relayerId)
      ).balanceOf();
      setBalance(balance);
      setLoading(false);
    },
    [sdk]
  );

  useEffect(() => {
    if (id) {
      getRelayerBalance(id);
    }
  }, [getRelayerBalance, id]);

  return { loading, balance };
};
