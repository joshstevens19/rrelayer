import {
  PauseRelayerToggleType,
  usePauseRelayerToggle,
  useRelayerBalance,
} from '@/hooks/relayer';
import ChainIdPrettyComponent from '@/shared/components/ChainIdPretty';
import LoadingComponent from '@/shared/components/Loading';
import React, { useState } from 'react';
import { Relayer } from 'rrelayerr-sdk';

interface RelayerHeaderProps {
  relayer: Relayer;
}

const RelayerHeader: React.FC<RelayerHeaderProps> = ({ relayer }) => {
  const { balance, loading } = useRelayerBalance(relayer.id);
  const pauseToggle = usePauseRelayerToggle();

  const [paused, setPausedState] = useState(relayer.paused);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  const handlePauseToggle = async () => {
    await pauseToggle(
      relayer.id,
      paused ? PauseRelayerToggleType.Unpause : PauseRelayerToggleType.Pause
    );

    setPausedState(!paused);
  };

  return (
    <div className="bg-white shadow-sm p-6 rounded-lg flex items-center justify-between">
      <div>
        <div>
          <button className="bg-purple-600 text-white text-xs px-4 py-1 rounded-full shadow-sm">
            <ChainIdPrettyComponent chainId={relayer.chainId} />
          </button>
          <button
            className={`text-xs px-4 py-1 rounded-full shadow-sm ${
              paused ? 'bg-red-600 text-white' : 'bg-green-600 text-white'
            }`}
          >
            {paused ? 'PAUSED' : 'RUNNING'}
          </button>
        </div>
        <div className="flex items-center text-sm text-gray-600">
          <span className="font-medium">Relayer id: {relayer.id}</span>
        </div>
        <span className="text-sm text-gray-600">
          {new Date(relayer.createdAt).toDateString()}
        </span>
        <h1 className="text-2xl font-bold text-gray-800">
          {relayer.name} - {balance} MATIC
        </h1>
        <div className="flex items-center mt-4">
          <div className="flex items-center text-sm text-gray-600">
            <span className="font-medium">Address:</span>
            <span className="ml-2">{relayer.address}</span>
          </div>
          <div className="ml-6">
            <button
              className={`text-xs px-4 py-1 rounded-full shadow-sm ${
                !paused ? 'bg-red-600 text-white' : 'bg-green-600 text-white'
              }`}
              onClick={handlePauseToggle}
            >
              {paused ? 'START' : 'PAUSE'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RelayerHeader;
