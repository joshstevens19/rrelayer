import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useEffect, useState } from 'react';
import { PagingContext, Relayer } from 'rrelayerr-sdk';

export const useGetRelayers = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [items, setItems] = useState<Relayer[]>([]);
  const [loading, setLoading] = useState(false);
  const [pagingContext, setPagingContext] = useState<
    | {
        next?: PagingContext;
        previous?: PagingContext;
      }
    | undefined
  >();

  const getRelayers = useCallback(
    async (context?: PagingContext) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const response = await sdk.admin.relayer.getAll(context);
      setItems(response.items);
      setPagingContext({ next: response.next, previous: response.previous });
      setLoading(false);
    },
    [sdk]
  );

  const next = useCallback(() => {
    if (pagingContext?.next) {
      getRelayers(pagingContext.next);
    }
  }, [getRelayers, pagingContext]);

  const previous = useCallback(() => {
    if (pagingContext?.previous) {
      getRelayers(pagingContext.previous);
    }
  }, [getRelayers, pagingContext]);

  useEffect(() => {
    getRelayers();
  }, [getRelayers]);

  return { items, loading, next, previous };
};

export default useGetRelayers;
