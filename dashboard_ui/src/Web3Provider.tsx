import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ConnectKitProvider, getDefaultConfig } from 'connectkit';
import { WagmiProvider, createConfig, http } from 'wagmi';
import { mainnet } from 'wagmi/chains';

export const config = createConfig(
  getDefaultConfig({
    chains: [mainnet],
    transports: {
      [mainnet.id]: http('https://rpc.ankr.com/eth'),
    },
    walletConnectProjectId: 'cf3f224a7a594e8d73a2700148c199da',
    appName: 'rrelayer Dashboard',
    appDescription: 'Dashboard to config the rrelayer',
    // appUrl: 'https://family.co',
    // appIcon: 'https://family.co/logo.png',
  })
);

const queryClient = new QueryClient();

export const Web3Provider = ({ children }: { children: React.ReactNode }) => {
  return (
    <WagmiProvider config={config}>
      <QueryClientProvider client={queryClient}>
        <ConnectKitProvider>{children}</ConnectKitProvider>
      </QueryClientProvider>
    </WagmiProvider>
  );
};
