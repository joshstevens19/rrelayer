import { getApi } from '../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../types';
import { Transaction } from './types';

export const getTransactions = async (
  relayerId: string,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<Transaction>> => {
  try {
    const response = await getApi<PagingResult<Transaction>>(
      baseConfig,
      `transactions/relayers/${relayerId}`,
      { ...pagingContext }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getTransactions:', error);
    throw error;
  }
};
