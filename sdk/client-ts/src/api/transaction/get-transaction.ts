import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Transaction } from './types';

export const getTransaction = async (
  transactionId: string,
  baseConfig: ApiBaseConfig
): Promise<Transaction | null> => {
  try {
    const response = await getApi<Transaction | null>(
      baseConfig,
      `transactions/${transactionId}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getTransaction:', error);
    throw error;
  }
};
