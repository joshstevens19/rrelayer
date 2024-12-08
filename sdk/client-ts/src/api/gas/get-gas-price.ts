import { getApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export interface GasPriceResult {
  maxPriorityFee: number;
  maxFee: number;
  minWaitTimeEstimate?: number;
  maxWaitTimeEstimate?: number;
}

export interface GasEstimatorResult {
  slow: GasPriceResult;
  medium: GasPriceResult;
  fast: GasPriceResult;
  superFast: GasPriceResult;
}

export const getGasPrices = async (
  chainId: string | number,
  baseConfig: ApiBaseConfig
): Promise<GasEstimatorResult | null> => {
  try {
    const response = await getApi<GasEstimatorResult | null>(
      baseConfig,
      `gas/price/${chainId}`
    );
    return response.data;
  } catch (error: any) {
    console.error('Failed to fetch gas prices:', error.message);
    throw error;
  }
};
