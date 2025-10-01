import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export interface CancelTransactionResult {
  success: boolean;
  cancelTransactionId?: string;
}

export const cancelTransaction = async (
  transactionId: string,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<CancelTransactionResult> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await putApi<CancelTransactionResult>(
      baseConfig,
      `transactions/cancel/${transactionId}`,
      {},
      config
    );
    return response.data;
  } catch (error) {
    console.error('Failed to cancelTransaction', error);
    throw error;
  }
};
