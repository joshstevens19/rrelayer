import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Transaction } from './types';

export const getTransactionByTxHash = async (
  txHash: string,
  baseConfig: ApiBaseConfig
): Promise<Transaction | null> => {
  try {
    const response = await getApi<Transaction | null>(
      baseConfig,
      `transactions/hash/${txHash}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to get transaction by txHash:', error);
    throw error;
  }
};
