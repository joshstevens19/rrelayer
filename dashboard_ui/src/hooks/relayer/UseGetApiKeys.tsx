import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { PagingContext } from 'rrelayer-sdk';

export const useGetApiKeys = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [items, setItems] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [relayerId, setRelayerId] = useState<string | null>(null);
  const [pagingContext, setPagingContext] = useState<
    | {
        next?: PagingContext;
        previous?: PagingContext;
      }
    | undefined
  >();

  const getApiKeys = useCallback(
    async (relayerId: string, context?: PagingContext) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const response = await sdk.admin.relayer.apiKeys.get(relayerId, context);
      setItems(response.items);
      setPagingContext({ next: response.next, previous: response.previous });
      setRelayerId(relayerId);
      setLoading(false);
    },
    [sdk]
  );

  const next = useCallback(() => {
    if (pagingContext?.next && relayerId) {
      getApiKeys(relayerId, pagingContext.next);
    }
  }, [getApiKeys, relayerId, pagingContext]);

  const previous = useCallback(() => {
    if (pagingContext?.previous && relayerId) {
      getApiKeys(relayerId, pagingContext.previous);
    }
  }, [getApiKeys, relayerId, pagingContext]);

  return { getApiKeys, items, loading, next, previous };
};
