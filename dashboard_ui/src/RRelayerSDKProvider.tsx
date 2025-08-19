import { useEffect, useState } from 'react';
import { createRRelayerClient } from 'rrelayer-sdk/dist/clients/core-client';
import { useAccount } from 'wagmi';
import { getWalletClient } from 'wagmi/actions';
import { config } from './Web3Provider';
import {
  RRelayerrSDKContext,
  RRelayerrSDKContextType,
} from './contexts/RRelayerrSDKContext';
import LoadingComponent from './shared/components/Loading';

export const RRelayerSDKProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const [sdk, setSDK] = useState<RRelayerrSDKContextType | null>(null);
  const [loading, setLoading] = useState(true);
  const { isConnected, connector } = useAccount();

  useEffect(() => {
    const initialize = async () => {
      if (!isConnected) {
        setSDK(null);
        setLoading(false);
        return;
      }

      if (connector === null) {
        setSDK(null);
        setLoading(false);
        return;
      }

      if (!connector?.getAccounts) {
        setSDK(null);
        setLoading(true);
        return;
      }

      const core = createRRelayerClient({
        // serverUrl: process.env.NEXT_PUBLIC_RELAYER_SERVER_URL!,
        serverUrl: 'http://127.0.0.1:8000',
      });

      const client = await getWalletClient(config, {
        connector,
      });

      const knownAuth = localStorage.getItem('rrelayer__authentication');

      setSDK({
        core,
        admin: core.createAdminClient({
          walletClient: client,
          validTokenPair: knownAuth ? JSON.parse(knownAuth) : undefined,
        }),
      });

      setLoading(false);
    };

    initialize();
  }, [isConnected, connector]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <RRelayerrSDKContext.Provider value={sdk}>
      {children}
    </RRelayerrSDKContext.Provider>
  );
};
