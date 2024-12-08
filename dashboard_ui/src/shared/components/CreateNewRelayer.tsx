import { useNetworks } from '@/hooks/network/UseNetworks';
import { useCreateNewRelayer } from '@/hooks/relayer';
import { useRouter } from 'next/router';
import React, { useState } from 'react';
import LoadingComponent from './Loading';

const CreateNewRelayer: React.FC = () => {
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [name, setName] = useState<string>('');
  const [network, setNetwork] = useState('0');

  const { getNetworks, loading, networks } = useNetworks();
  const createNewRelayer = useCreateNewRelayer({
    onSuccess: (result) => {
      router.push(`/relayer/${result.id}`);
    },
  });

  const open = () => {
    setIsOpen(true);
    getNetworks();
  };

  const create = async () => {
    await createNewRelayer(name, parseInt(network));
  };

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div>
      <button
        onClick={() => open()}
        className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
      >
        Create New Relayer
      </button>

      {isOpen && (
        <div className="fixed inset-0 z-10 flex items-center justify-center overflow-x-hidden overflow-y-auto outline-none focus:outline-none">
          <div className="relative w-1/3 max-w-lg mx-auto my-6">
            <div className="bg-white rounded-lg shadow-lg outline-none focus:outline-none">
              <div className="flex items-center justify-between p-5 border-b border-solid border-gray-300 rounded-t">
                <h3 className="text-lg font-semibold">Add a new relayer</h3>
                <button
                  onClick={() => setIsOpen(false)}
                  className="p-1 ml-auto bg-transparent border-0 text-black float-right text-3xl leading-none font-semibold outline-none focus:outline-none"
                >
                  <span className="bg-transparent text-black h-6 w-6 text-2xl block outline-none focus:outline-none">
                    ×
                  </span>
                </button>
              </div>
              <div className="relative p-6">
                <form className="mt-4" action="#">
                  <label
                    htmlFor="emails-list"
                    className="text-lg font-medium text-gray-700"
                  >
                    Relayer name
                  </label>

                  <div className="mt-2">
                    <input
                      type="text"
                      name="name"
                      id="name"
                      placeholder="Enter relayer name..."
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      className="w-full px-4 py-2 text-sm text-gray-700 bg-white border border-gray-300 rounded-md focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
                    />
                  </div>

                  <label
                    htmlFor="emails-list"
                    className="block mt-6 text-lg font-medium text-gray-700"
                  >
                    Network
                  </label>

                  <div className="relative inline-block w-full text-gray-700 mt-2">
                    <select
                      className="w-full px-4 py-2 text-sm text-gray-700 bg-white border border-gray-300 rounded-md focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
                      name="network"
                      value={network}
                      onChange={(e) => setNetwork(e.target.value)}
                    >
                      <option value="0">Select network</option>
                      {networks &&
                        networks.map((network) => (
                          <option key={network.chainId} value={network.chainId}>
                            {network.name}
                          </option>
                        ))}
                    </select>
                  </div>
                </form>
              </div>
              <div className="flex items-center justify-end p-6 border-t border-solid border-gray-300 rounded-b">
                <button
                  className="px-6 py-2 mb-1 mr-1 text-sm font-bold text-white uppercase bg-blue-600 outline-none rounded-md focus:outline-none hover:bg-blue-700"
                  type="button"
                  onClick={() => create()}
                >
                  Create
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default CreateNewRelayer;
