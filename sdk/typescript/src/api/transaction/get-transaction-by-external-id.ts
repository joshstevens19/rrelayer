import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Transaction } from './types';

export const getTransactionByExternalId = async (
  externalId: string,
  baseConfig: ApiBaseConfig
): Promise<Transaction | null> => {
  try {
    const response = await getApi<Transaction | null>(
      baseConfig,
      `transactions/external/${externalId}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to get transaction by externalId:', error);
    throw error;
  }
};
