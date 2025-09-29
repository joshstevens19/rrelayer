import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const updateRelayerMaxGasPrice = async (
  relayerId: string,
  cap: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `relayers/${relayerId}/gas/max/${cap}`);
  } catch (error) {
    console.error('Failed to updateRelayerEIP1559Status:', error);
    throw error;
  }
};
