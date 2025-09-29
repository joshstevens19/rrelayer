import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const removeRelayerMaxGasPrice = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `relayers/${relayerId}/gas/max/0`);
  } catch (error) {
    console.error('Failed to removeRelayerMaxGasPrice:', error);
    throw error;
  }
};
