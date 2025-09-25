import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import {RATE_LIMIT_HEADER_NAME} from "../index";

export const cancelTransaction = async (
  transactionId: string,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<boolean> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await postApi<boolean>(
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
