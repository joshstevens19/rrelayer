import { useGetRelayers } from '@/hooks/relayer';
import MainLayout from '@/layouts/MainLayout';
import RelayerCard from '@/pages/relayers/components/RelayerCard';
import CreateNewRelayer from '@/shared/components/CreateNewRelayer';
import LoadingComponent from '@/shared/components/Loading';
import React from 'react';

const Relayers: React.FC = () => {
  const { items, loading } = useGetRelayers();
  return (
    <MainLayout>
      {/* <NetworkFilterDropdown /> */}
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold">Relayers</h1>
        <CreateNewRelayer />
      </div>

      {loading && <LoadingComponent></LoadingComponent>}

      {!loading && items.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {items.map((relayer) => (
            <RelayerCard key={relayer.id} {...relayer} />
          ))}
        </div>
      )}

      {!loading && items.length === 0 && (
        <div className="text-center mt-8">
          <p>No relayers found</p>
        </div>
      )}
    </MainLayout>
  );
};

export default Relayers;
