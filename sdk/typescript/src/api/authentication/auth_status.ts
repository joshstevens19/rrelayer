import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export interface StatusResponse {
  authenticated: Boolean;
  message: string;
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
