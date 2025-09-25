import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { TokenPair } from './types';

export interface AuthenticateRequest {
  id: string;
  signedBy: string;
  signature: string;
}

export const authenticate = async (
  request: AuthenticateRequest,
  baseConfig: ApiBaseConfig
): Promise<TokenPair> => {
  try {
    const result = await postApi<TokenPair>(
      baseConfig,
      'authentication/authenticate',
      {
        ...request,
      }
    );

    return result.data;
  } catch (error) {
    console.error('Failed to authenticate:', error);
    throw error;
  }
};
