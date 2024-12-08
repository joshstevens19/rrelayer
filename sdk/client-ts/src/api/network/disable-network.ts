import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const disableNetwork = async (
  chainId: string | number,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `disable/${chainId}`);
  } catch (error) {
    console.error('Failed to disabled network:', error);
    throw error;
  }
};
