import { useRelayerBalance } from '@/hooks/relayer';
import LoadingComponent from '@/shared/components/Loading';
import { useRouter } from 'next/router';
import React from 'react';
import { Relayer } from 'rrelayerr-sdk';
import AddressComponent from '../../../shared/components/Address';
import ChainIdPrettyComponent from '../../../shared/components/ChainIdPretty';

const RelayerCard: React.FC<Relayer> = ({
  id,
  name,
  chainId,
  address,
  maxGasPrice,
  paused,
  allowlistedOnly,
  eip1559Enabled,
  createdAt,
}) => {
  const router = useRouter();
  const { balance, loading } = useRelayerBalance(id);

  const loadRelayer = () => {
    router.push(`/relayer/${id}`);
  };

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div
      className="bg-white shadow-md rounded-lg overflow-hidden border border-gray-200 hover:shadow-lg transition duration-300 cursor-pointer"
      onClick={loadRelayer}
    >
      <div className="px-6 py-4">
        <div className="flex justify-between items-center">
          <span className="text-sm text-gray-600">
            {new Date(createdAt).toDateString()}
          </span>
          <span className="px-3 py-1 text-sm font-bold text-gray-100 bg-gray-600 rounded">
            <ChainIdPrettyComponent chainId={chainId} />
          </span>
        </div>
        <div className="mt-2">
          <h3 className="text-xl font-semibold text-gray-800 hover:text-gray-600 transition duration-300 cursor-pointer">
            {name} - {balance} MATIC
          </h3>
          <p className="mt-2 text-gray-600">
            <AddressComponent address={address} />
          </p>
        </div>
      </div>
      {paused && (
        <div className="px-6 py-2 bg-red-100 text-red-700 text-sm font-semibold">
          Paused
        </div>
      )}
    </div>
  );
};

export default RelayerCard;
