import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const enableNetwork = async (
  chainId: string | number,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `enable/${chainId}`);
  } catch (error) {
    console.error('Failed to enable network:', error);
    throw error;
  }
};
