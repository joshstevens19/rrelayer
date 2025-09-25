import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { CreateRelayerResult } from './create-relayer';

export const cloneRelayer = async (
  id: string,
  chainId: number,
  name: string,
  baseConfig: ApiBaseConfig
): Promise<CreateRelayerResult> => {
  try {
    const response = await postApi<CreateRelayerResult>(
      baseConfig,
      `relayers/${id}/clone`,
      {
        new_relayer_name: name,
        chain_id: chainId,
      }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to clone relayer:', error);
    throw error;
  }
};
