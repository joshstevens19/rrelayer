import React from 'react';

const chainIdPretty = (chainId: number): string => {
  switch (chainId) {
    case 80001:
      return 'Mumbai';
    // @ts-ignore
    case '0x13881':
      return 'Mumbai';
    default:
      throw new Error(`Unsupported chainId: ${chainId}`);
  }
};

const ChainIdPrettyComponent: React.FC<{ chainId: number }> = ({ chainId }) => {
  const networkName = chainIdPretty(chainId);

  return <span>{networkName}</span>;
};

export default ChainIdPrettyComponent;
