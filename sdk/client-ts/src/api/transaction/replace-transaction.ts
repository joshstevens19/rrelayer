import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionSent, TransactionToSend } from './types';

export const replaceTransaction = async (
  transactionId: string,
  replacementTransaction: TransactionToSend,
  baseConfig: ApiBaseConfig
): Promise<TransactionSent> => {
  try {
    const response = await postApi<TransactionSent>(
      baseConfig,
      `transactions/replace/${transactionId}`,
      {
        ...replacementTransaction,
      }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to replaceTransaction', error);
    throw error;
  }
};
