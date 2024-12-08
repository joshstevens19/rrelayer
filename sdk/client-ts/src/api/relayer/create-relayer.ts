import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export interface CreateRelayerResult {
  id: string;
  address: string;
}

export const createRelayer = async (
  chainId: string | number,
  name: string,
  baseConfig: ApiBaseConfig
): Promise<CreateRelayerResult> => {
  try {
    const response = await postApi<CreateRelayerResult>(
      baseConfig,
      `relayers/${chainId}/new`,
      { name }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to createRelayer', error);
    throw error;
  }
};
