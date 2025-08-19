import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { PagingContext } from 'rrelayer-sdk';

export const useGetAllowlisted = () => {
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

  const getAllowlisted = useCallback(
    async (relayerId: string, context?: PagingContext) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const response = await (
        await sdk.admin.relayer.createRelayerClient(relayerId)
      ).allowlist.get(context);
      setItems(response.items);
      setPagingContext({ next: response.next, previous: response.previous });
      setRelayerId(relayerId);
      setLoading(false);
    },
    [sdk]
  );

  const next = useCallback(() => {
    if (pagingContext?.next && relayerId) {
      getAllowlisted(relayerId, pagingContext.next);
    }
  }, [getAllowlisted, relayerId, pagingContext]);

  const previous = useCallback(() => {
    if (pagingContext?.previous && relayerId) {
      getAllowlisted(relayerId, pagingContext.previous);
    }
  }, [getAllowlisted, relayerId, pagingContext]);

  return { getAllowlisted, items, loading, next, previous };
};
