import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TokenPair } from './types';

export const refreshAuthToken = async (
  token: string,
  baseConfig: ApiBaseConfig
): Promise<TokenPair> => {
  try {
    const result = await postApi<TokenPair>(
      baseConfig,
      'authentication/refresh',
      {
        token,
      }
    );

    return result.data;
  } catch (error) {
    console.error('Failed to refreshAuthToken:', error);
    throw error;
  }
};
