import { TransactionReceipt } from 'viem';
import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionStatus } from './types';

export interface TransactionStatusResult {
  hash?: `0x${string}`;
  status: TransactionStatus;
  receipt?: TransactionReceipt;
}

export const getTransactionStatus = async (
  transactionId: string,
  baseConfig: ApiBaseConfig
): Promise<TransactionStatusResult | null> => {
  try {
    const response = await getApi<TransactionStatusResult | null>(
      baseConfig,
      `transactions/status/${transactionId}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getTransactionStatus:', error);
    throw error;
  }
};
