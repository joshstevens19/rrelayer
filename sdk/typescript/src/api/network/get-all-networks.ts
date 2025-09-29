import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Network } from './types';

export const getAllNetworks = async (
  baseConfig: ApiBaseConfig
): Promise<Network[]> => {
  try {
    const response = await getApi<Network[]>(baseConfig, 'networks');
    return response.data;
  } catch (error) {
    console.error('Failed to fetch all networks:', error);
    throw error;
  }
};
