import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const getTransactionsInmempoolCount = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<number> => {
  try {
    const response = await getApi<number>(
      baseConfig,
      `transactions/relayers/${relayerId}/inmempool/count`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getTransactionsInmempoolCount:', error);
    throw error;
  }
};
