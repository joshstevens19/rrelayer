import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { PagingContext, Transaction } from 'rrelayer-sdk';

export const useGetTransactions = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [items, setItems] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(false);
  const [relayerId, setRelayerId] = useState<string | null>(null);
  const [pagingContext, setPagingContext] = useState<
    | {
        next?: PagingContext;
        previous?: PagingContext;
      }
    | undefined
  >();

  const getTransactions = useCallback(
    async (relayerId: string, context?: PagingContext) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const response = await (
        await sdk.admin.relayer.createRelayerClient(relayerId)
      ).transactions.getTransactions(context);
      setItems(response.items);
      setPagingContext({ next: response.next, previous: response.previous });
      setRelayerId(relayerId);
      setLoading(false);
    },
    [sdk]
  );

  const next = useCallback(() => {
    if (pagingContext?.next && relayerId) {
      getTransactions(relayerId, pagingContext.next);
    }
  }, [getTransactions, relayerId, pagingContext]);

  const previous = useCallback(() => {
    if (pagingContext?.previous && relayerId) {
      getTransactions(relayerId, pagingContext.previous);
    }
  }, [getTransactions, relayerId, pagingContext]);

  return { getTransactions, items, loading, next, previous };
};
