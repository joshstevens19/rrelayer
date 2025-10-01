import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Network } from './types';

export const getNetwork = async (
  chain_id: number,
  baseConfig: ApiBaseConfig
): Promise<Network | null> => {
  try {
    const response = await getApi<Network | null>(
      baseConfig,
      `networks/${chain_id}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch all networks:', error);
    throw error;
  }
};
