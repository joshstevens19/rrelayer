import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TransactionToSend } from './types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export interface ReplaceTransactionResult {
  success: boolean;
  replaceTransactionId?: string;
  replaceTransactionHash?: `0x${string}`
}

export const replaceTransaction = async (
  transactionId: string,
  replacementTransaction: TransactionToSend,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<ReplaceTransactionResult> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await putApi<ReplaceTransactionResult>(
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
