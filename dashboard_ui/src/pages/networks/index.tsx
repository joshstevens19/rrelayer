import { useNetworks } from '@/hooks/network/UseNetworks';
import MainLayout from '@/layouts/MainLayout';
import LoadingComponent from '@/shared/components/Loading';
import React, { useEffect } from 'react';
import NetworkCard from './components/NetworkCard';

const Networks: React.FC = () => {
  const { getNetworks, networks, loading } = useNetworks();

  useEffect(() => {
    getNetworks();
  }, [getNetworks]);

  return (
    <MainLayout>
      {loading && <LoadingComponent></LoadingComponent>}

      {!loading && networks && networks.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {networks.map((network) => (
            <NetworkCard key={network.chainId} {...network} />
          ))}
        </div>
      )}

      {(!loading && !networks) ||
        (networks?.length === 0 && (
          <div className="text-center mt-8">
            <p>No Networks found</p>
          </div>
        ))}
    </MainLayout>
  );
};

export default Networks;
