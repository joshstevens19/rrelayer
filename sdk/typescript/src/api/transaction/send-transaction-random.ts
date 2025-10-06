import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionSent, TransactionToSend } from './types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export const sendTransactionRandom = async (
  chainId: string,
  transactionToSend: TransactionToSend,
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
      `transactions/relayers/${chainId}/send_random`,
      {
        ...transactionToSend,
      },
      config
    );
    return response.data;
  } catch (error) {
    console.error('Failed to sendTransactionRandom', error);
    throw error;
  }
};
