import { RRelayerrSDKContext } from '@/contexts/RRelayerrSDKContext';
import { useCallback, useContext, useState } from 'react';
import { PagingContext, User } from 'rrelayerr-sdk';

export const useGetUsers = () => {
  const sdk = useContext(RRelayerrSDKContext);
  const [items, setItems] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);
  const [relayerId, setRelayerId] = useState<string | null>(null);
  const [pagingContext, setPagingContext] = useState<
    | {
        next?: PagingContext;
        previous?: PagingContext;
      }
    | undefined
  >();

  const getUsers = useCallback(
    async (context?: PagingContext) => {
      if (!sdk) {
        throw new Error('RRelayerrSDKContext is undefined');
      }

      setLoading(true);
      const response = await sdk.admin.user.get(context);
      setItems(response.items);
      setPagingContext({ next: response.next, previous: response.previous });
      setRelayerId(relayerId);
      setLoading(false);
    },
    [sdk, relayerId]
  );

  const next = useCallback(() => {
    if (pagingContext?.next && relayerId) {
      getUsers(pagingContext.next);
    }
  }, [getUsers, relayerId, pagingContext]);

  const previous = useCallback(() => {
    if (pagingContext?.previous && relayerId) {
      getUsers(pagingContext.previous);
    }
  }, [getUsers, relayerId, pagingContext]);

  return { getUsers, items, loading, next, previous };
};
