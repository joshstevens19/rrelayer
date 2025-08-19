import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { ConnectKitButton } from 'connectkit';
import { motion } from 'framer-motion';
import { useRouter } from 'next/router';
import React, { useContext, useEffect } from 'react';
import { TokenPair } from 'rrelayer-sdk/dist/api/authentication/types';
import { useAccount } from 'wagmi';
import Authenticate from './components/authenticate';

const Login: React.FC = () => {
  const router = useRouter();
  const { isConnected } = useAccount();
  const sdk = useContext(RRelayerrSDKContext);

  useEffect(() => {
    if (!sdk) {
      return;
    }

    if (sdk.admin.authentication.isAuthenticated()) {
      router.push('/relayers');
      return;
    }
  }, [sdk, router]);

  const loggedIn = async (response: TokenPair) => {
    console.log('logged in', response);
    router.push('/relayers');
  };

  return (
    <div>
      <div
        className="flex flex-col items-center justify-center min-h-screen text-gray-900 bg-cover bg-center bg-no-repeat"
        style={{
          backgroundImage:
            "linear-gradient(rgba(255, 255, 255, 0.5), rgba(255, 255, 255, 0.5)), url('/logo.jpeg')",
          backgroundPosition: '50% 30%',
        }}
      >
        <div className="bg-white bg-opacity-90 p-6 rounded-lg shadow-lg">
          <motion.h1
            initial={{ opacity: 0, y: -50 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="text-3xl font-bold mb-4"
          >
            Welcome to the Dashboard for rrelayer
          </motion.h1>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.5, duration: 0.5 }}
            className="flex flex-col items-center"
          >
            {!isConnected && (
              <p className="mb-4">Please login by connecting your wallet:</p>
            )}
            {isConnected && <p className="mb-4">Connected</p>}
            <ConnectKitButton />
            {isConnected && <Authenticate onSuccess={loggedIn}></Authenticate>}
          </motion.div>
        </div>
      </div>
    </div>
  );
};

export default Login;
