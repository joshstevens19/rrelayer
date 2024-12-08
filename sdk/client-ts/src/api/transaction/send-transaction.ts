import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionSent, TransactionToSend } from './types';

export const sendTransaction = async (
  relayerId: string,
  transactionToSend: TransactionToSend,
  baseConfig: ApiBaseConfig
): Promise<TransactionSent> => {
  try {
    const response = await postApi<TransactionSent>(
      baseConfig,
      `transactions/relayers/${relayerId}/send`,
      {
        ...transactionToSend,
      }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to sendTransaction', error);
    throw error;
  }
};
