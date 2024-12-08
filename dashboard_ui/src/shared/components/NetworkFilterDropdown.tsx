import { useNetworks } from '@/hooks/network/UseNetworks';
import LoadingComponent from '@/shared/components/Loading';
import React, { useEffect, useState } from 'react';
import { Network } from 'rrelayerr-sdk';

export interface NetworkFilterDropdownProps {
  onSelect?: (network: Network) => void;
}

const NetworkFilterDropdown: React.FC = ({
  onSelect,
}: NetworkFilterDropdownProps) => {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const { getNetworks, loading, networks } = useNetworks();

  useEffect(() => {
    getNetworks();
  }, [getNetworks]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div className="relative inline-block">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="relative z-10 block p-2 text-gray-700 bg-white border border-transparent rounded-md dark:text-white focus:border-blue-500 focus:ring-opacity-40 dark:focus:ring-opacity-40 focus:ring-blue-300 dark:focus:ring-blue-400 focus:ring dark:bg-gray-800 focus:outline-none"
      >
        All Networks
        <svg
          className="w-5 h-5 text-gray-800 dark:text-white"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
            clipRule="evenodd"
          />
        </svg>
      </button>
      {isOpen && (
        <div
          onClick={() => setIsOpen(false)}
          className="absolute right-0 z-20 w-48 py-2 mt-2 origin-top-right bg-white rounded-md shadow-xl dark:bg-gray-800"
        >
          {networks &&
            networks.map((network) => (
              <a
                key={network.chainId}
                onClick={(event) => {
                  event.preventDefault();

                  if (onSelect) {
                    onSelect(network);
                  }
                }}
                className="block px-4 py-3 text-sm text-gray-600 capitalize transition-colors duration-300 transform dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 dark:hover:text-white"
              >
                {network.name}
              </a>
            ))}
        </div>
      )}
    </div>
  );
};

export default NetworkFilterDropdown;
