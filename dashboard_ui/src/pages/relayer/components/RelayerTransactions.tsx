import { useGetTransactions } from '@/hooks/transactions';
import LoadingComponent from '@/shared/components/Loading';
import React, { useEffect } from 'react';

export interface RelayerTransactionsProps {
  relayerId: string;
}

const RelayerTransactions: React.FC<RelayerTransactionsProps> = ({
  relayerId,
}) => {
  const { getTransactions, items, loading, next, previous } =
    useGetTransactions();

  useEffect(() => {
    getTransactions(relayerId);
  }, [getTransactions, relayerId]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <div className="bg-white p-6 rounded-lg shadow">
      <div className="overflow-x-auto">
        {items.length > 0 ? (
          <table className="w-full text-sm text-left text-gray-500">
            <thead className="text-xs text-gray-700 uppercase bg-gray-50">
              <tr>
                <th scope="col" className="px-6 py-3">
                  Id
                </th>
                <th scope="col" className="px-6 py-3">
                  Status
                </th>
                <th scope="col" className="px-6 py-3">
                  To
                </th>
                <th scope="col" className="px-6 py-3">
                  Value
                </th>
                <th scope="col" className="px-6 py-3">
                  Data
                </th>
                <th scope="col" className="px-6 py-3">
                  TxHash
                </th>
                <th scope="col" className="px-6 py-3">
                  Sent at
                </th>
                <th scope="col" className="px-6 py-3">
                  Speed
                </th>
                <th scope="col" className="px-6 py-3">
                  Sent with gas
                </th>
              </tr>
            </thead>
            <tbody>
              {items.map((tx, index) => (
                <tr key={index} className="bg-white border-b">
                  <td className="px-6 py-4">{tx.id}</td>
                  <td className="px-6 py-4">{tx.status}</td>
                  <td className="px-6 py-4 flex items-center">{tx.to}</td>
                  <td className="px-6 py-4">{tx.value}</td>
                  <td className="px-6 py-4">{tx.data}</td>
                  <td className="px-6 py-4">{tx.knownTransactionHash}</td>
                  <td className="px-6 py-4">{tx.sentAt}</td>
                  <td className="px-6 py-4">{tx.speed}</td>
                  <td className="px-6 py-4">
                    Fee {tx.sentWithGas?.maxFee} 12003
                    <br />
                    Priority
                    {tx.sentWithGas?.maxPriorityFee} 13
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="text-center text-gray-500">No transactions found</div>
        )}
      </div>
    </div>
  );
};

export default RelayerTransactions;
