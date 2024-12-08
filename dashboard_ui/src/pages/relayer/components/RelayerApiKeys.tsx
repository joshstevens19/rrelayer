import {
  useCreateNewRelayerApiKey,
  useDeleteRelayerApiKey,
  useGetApiKeys,
} from '@/hooks/relayer';
import LoadingComponent from '@/shared/components/Loading';
import React, { useEffect } from 'react';

export interface RelayerApiKeysProps {
  relayerId: string;
}

const RelayerApiKeys: React.FC<RelayerApiKeysProps> = ({ relayerId }) => {
  const { getApiKeys, items, loading, next, previous } = useGetApiKeys();
  const createNewRelayerApiKey = useCreateNewRelayerApiKey({
    onSuccess: (_) => {
      getApiKeys(relayerId);
    },
  });

  const create = async () => {
    await createNewRelayerApiKey(relayerId);
  };

  const deleteApiKey = useDeleteRelayerApiKey();

  const revoke = async (apiKey: string) => {
    await deleteApiKey(relayerId, apiKey);
    getApiKeys(relayerId);
  };

  useEffect(() => {
    getApiKeys(relayerId);
  }, [getApiKeys, relayerId]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div className="bg-white p-6 rounded-lg shadow">
      <div className="flex justify-end mb-4">
        <button
          className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
          onClick={() => create()}
        >
          Create new API Key
        </button>
      </div>
      {items.length > 0 ? (
        <div>
          {items.map((apiKey, index) => (
            <div
              key={index}
              className="flex items-center justify-between bg-gray-50 p-4 rounded-lg mb-4 last:mb-0"
            >
              <div className="flex items-center">
                <span className="text-blue-500 mr-3">🔑</span>
                <span>{apiKey}</span>
              </div>
              <div className="text-sm text-gray-600">Created on TODO!</div>
              <button
                className="text-gray-400 hover:text-gray-600"
                onClick={() => revoke(apiKey)}
              >
                Revoke
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center text-gray-500">No API keys found</div>
      )}
    </div>
  );
};

export default RelayerApiKeys;
