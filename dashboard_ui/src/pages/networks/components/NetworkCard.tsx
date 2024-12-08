import React from 'react';
import { Network } from 'rrelayerr-sdk';
import ChainIdPrettyComponent from '../../../shared/components/ChainIdPretty';

const NetworkCard: React.FC<Network> = ({
  name,
  chainId,
  disabled,
  providerUrls,
}) => {
  return (
    <div className="bg-white shadow-md rounded-lg overflow-hidden border border-gray-200 hover:shadow-lg transition duration-300 cursor-pointer">
      <div className="px-6 py-4">
        <div className="flex justify-between items-center">
          <span className="text-sm text-gray-600">
            {new Date().toDateString()}
          </span>
          <span className="px-3 py-1 text-sm font-bold text-gray-100 bg-gray-600 rounded">
            <ChainIdPrettyComponent chainId={chainId} />
          </span>
        </div>
        <div className="mt-2">
          <h3 className="text-xl font-semibold text-gray-800 hover:text-gray-600 transition duration-300 cursor-pointer">
            {name}
          </h3>
          <p className="mt-2 text-gray-600">
            {providerUrls && providerUrls.length > 0 && (
              <ul>
                {providerUrls.map((url, index) => (
                  <li key={index}>{url}</li>
                ))}
              </ul>
            )}
          </p>
          {/* <button className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded mt-4">
            {disabled ? 'Enable' : 'Disable'}
          </button> */}
        </div>
      </div>
      {disabled && (
        <div className="px-6 py-2 bg-red-100 text-red-700 text-sm font-semibold">
          Disabled
        </div>
      )}
    </div>
  );
};

export default NetworkCard;
