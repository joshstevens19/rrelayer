import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { Relayer } from './types';

export interface GetRelayerResult {
  relayer: Relayer;
  providerUrls: string[];
}

export const getRelayer = async (
  id: string,
  baseConfig: ApiBaseConfig
): Promise<GetRelayerResult | null> => {
  try {
    const response = await getApi<GetRelayerResult | null>(
      baseConfig,
      `relayers/${id}`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to fetch getRelayer:', error);
    throw error;
  }
};
