import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionSent, TransactionToSend } from './types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export const replaceTransaction = async (
  transactionId: string,
  replacementTransaction: TransactionToSend,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<TransactionSent> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await postApi<TransactionSent>(
      baseConfig,
      `transactions/replace/${transactionId}`,
      {
        ...replacementTransaction,
      },
      config
    );
    return response.data;
  } catch (error) {
    console.error('Failed to replaceTransaction', error);
    throw error;
  }
};
