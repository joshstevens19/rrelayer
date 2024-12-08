import {
  useAddAllowlistedAddressForRelayer,
  useDeleteAllowlistedAddress,
  useGetAllowlisted,
} from '@/hooks/relayer';
import LoadingComponent from '@/shared/components/Loading';
import React, { useEffect, useState } from 'react';
import { isAddress } from 'viem';

export interface RelayerApiKeysProps {
  relayerId: string;
}

const RelayerAllowlisted: React.FC<RelayerApiKeysProps> = ({ relayerId }) => {
  const [inputValue, setInputValue] = useState('');
  const { getAllowlisted, items, loading, next, previous } =
    useGetAllowlisted();

  const addAllowlistedAddress = useAddAllowlistedAddressForRelayer();

  const handleAddAllowlistedAddress = async () => {
    if (!isAddress(inputValue)) {
      throw new Error('Invalid address');
    }

    await addAllowlistedAddress(relayerId, inputValue);
    setInputValue('');
    getAllowlisted(relayerId);
  };

  const deleteAllowlistedAddress = useDeleteAllowlistedAddress();

  const revoke = async (address: string) => {
    if (!isAddress(address)) {
      throw new Error('Invalid address');
    }

    await deleteAllowlistedAddress(relayerId, address);
    getAllowlisted(relayerId);
  };

  useEffect(() => {
    getAllowlisted(relayerId);
  }, [getAllowlisted, relayerId]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div className="bg-white p-6 rounded-lg shadow">
      <div className="w-1/3 mb-4">
        <label
          htmlFor="allowlistedAddress"
          className="block text-gray-700 text-sm font-bold mb-2"
        >
          New Allowlisted Address
        </label>
        <input
          id="allowlistedAddress"
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          className="w-full py-2 px-4 rounded border border-gray-300 focus:outline-none focus:ring-2 focus:ring-blue-200 mb-2"
        />
        <button
          onClick={handleAddAllowlistedAddress}
          className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
        >
          Create New Allowlisted Address
        </button>
      </div>
      {items.length > 0 ? (
        <div>
          {items.map((allowlisted, index) => (
            <div
              key={index}
              className="flex items-center justify-between bg-gray-50 p-4 rounded-lg mb-4 last:mb-0"
            >
              <div className="flex items-center">
                <span className="text-blue-500 mr-3">🔑</span>
                <span>{allowlisted}</span>
              </div>
              <div className="text-sm text-gray-600">Created on TODO!</div>
              <button
                className="text-gray-400 hover:text-gray-600"
                onClick={() => revoke(allowlisted)}
              >
                Revoke
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center text-gray-500">
          Relayer can send transactions to any address
        </div>
      )}
    </div>
  );
};

export default RelayerAllowlisted;
