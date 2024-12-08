import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Network } from './types';

export const getEnabledNetworks = async (
  baseConfig: ApiBaseConfig
): Promise<Network[]> => {
  try {
    const response = await getApi<Network[]>(baseConfig, 'networks/enabled');
    return response.data;
  } catch (error) {
    console.error('Failed to fetch enabled networks:', error);
    throw error;
  }
};
