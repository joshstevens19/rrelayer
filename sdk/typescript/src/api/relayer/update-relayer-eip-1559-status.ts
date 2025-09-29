import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const updateRelayerEIP1559Status = async (
  relayerId: string,
  status: boolean,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `relayers/${relayerId}/gas/eip1559/${status}`);
  } catch (error) {
    console.error('Failed to updateRelayerEIP1559Status:', error);
    throw error;
  }
};
