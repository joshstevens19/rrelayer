import {
  useUpdateEIP1559RelayerStatus,
  useUpdateMaxGasPriceRelayer,
} from '@/hooks/relayer';
import React, { useState } from 'react';
import { Relayer } from 'rrelayer-sdk';

export interface RelayerGasSettingsProps {
  relayer: Relayer;
}

const RelayerGasSettings: React.FC<RelayerGasSettingsProps> = ({ relayer }) => {
  const updateEIP1559RelayerStatus = useUpdateEIP1559RelayerStatus();
  const updateMaxGasPriceRelayer = useUpdateMaxGasPriceRelayer();

  const [eip1559Enabled, setEip1559Enabled] = useState(relayer.eip1559Enabled);
  const [maxGasPrice, setMaxGasPrice] = useState(relayer.maxGasPrice || '');

  const handleSave = () => {
    updateEIP1559RelayerStatus(relayer.id, eip1559Enabled);
    if (maxGasPrice === '') return;
    updateMaxGasPriceRelayer(relayer.id, maxGasPrice.toString());
  };

  return (
    <div className="bg-white p-6 rounded-lg shadow">
      <ul className="mb-6 space-y-4">
        <li>
          <div className="text-sm font-medium mb-2">EIP-1559 enabled</div>
          <input
            type="checkbox"
            checked={eip1559Enabled}
            onChange={(e) => setEip1559Enabled(e.target.checked)}
            className="form-checkbox h-5 w-5 text-blue-600"
          />
        </li>

        <li>
          <div className="text-sm font-medium mb-2">Max gas price</div>
          <input
            type="text"
            value={maxGasPrice}
            onChange={(e) => setMaxGasPrice(e.target.value)}
            className="form-input border-2 border-gray-200 rounded-md p-2"
          />
        </li>
      </ul>

      <button
        onClick={handleSave}
        className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded mt-4"
      >
        Save
      </button>
    </div>
  );
};

export default RelayerGasSettings;
