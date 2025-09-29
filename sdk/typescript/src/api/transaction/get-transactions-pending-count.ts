import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const getTransactionsPendingCount = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<number> => {
  try {
    const response = await getApi<number>(
      baseConfig,
      `transactions/relayers/${relayerId}/pending/count`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getTransactionsPendingCount:', error);
    throw error;
  }
};
