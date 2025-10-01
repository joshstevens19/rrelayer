import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export enum AuthType {
  BASIC = 'BASIC',
  APIKEY = 'APIKEY',
}

export interface ApiKeyAccess {
  chainId: number;
  relayers: `0x${string}`[];
}

export interface StatusResponse {
  authenticatedWith: AuthType;
  apiKeyAccess?: ApiKeyAccess[];
}

export const auth_status = async (
  baseConfig: ApiBaseConfig
): Promise<StatusResponse> => {
  try {
    const result = await getApi<StatusResponse>(baseConfig, 'auth/status');

    return result.data;
  } catch (error) {
    console.error('Failed to test auth status:', error);
    throw error;
  }
};
