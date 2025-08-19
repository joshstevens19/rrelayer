import { RRelayerSDKProvider } from '@/RRelayerSDKProvider';
import { Web3Provider } from '@/Web3Provider';
import { AppProps } from 'next/app';
import '../styles/globals.css';

const App: React.FC<AppProps> = ({ Component, pageProps }) => {
  return (
    <Web3Provider>
      <RRelayerSDKProvider>
        <Component {...pageProps} />;
      </RRelayerSDKProvider>
    </Web3Provider>
  );
};

export default App;
