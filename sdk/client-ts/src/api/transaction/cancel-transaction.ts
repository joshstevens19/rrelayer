import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const cancelTransaction = async (
  transactionId: string,
  baseConfig: ApiBaseConfig
): Promise<boolean> => {
  try {
    const response = await postApi<boolean>(
      baseConfig,
      `transactions/cancel/${transactionId}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to cancelTransaction', error);
    throw error;
  }
};
