import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Network } from './types';

export const getDisabledNetworks = async (
  baseConfig: ApiBaseConfig
): Promise<Network[]> => {
  try {
    const response = await getApi<Network[]>(baseConfig, 'networks/disabled');
    return response.data;
  } catch (error) {
    console.error('Failed to fetch disabled networks:', error);
    throw error;
  }
};
