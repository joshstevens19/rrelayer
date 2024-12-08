import { RRelayerrSDKProvider } from '@/RRelayerrSDKProvider';
import { Web3Provider } from '@/Web3Provider';
import { AppProps } from 'next/app';
import '../styles/globals.css';

const App: React.FC<AppProps> = ({ Component, pageProps }) => {
  return (
    <Web3Provider>
      <RRelayerrSDKProvider>
        <Component {...pageProps} />;
      </RRelayerrSDKProvider>
    </Web3Provider>
  );
};

export default App;
