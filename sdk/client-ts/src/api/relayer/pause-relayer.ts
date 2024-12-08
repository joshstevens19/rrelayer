import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const pauseRelayer = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `relayers/${relayerId}/pause`);
  } catch (error) {
    console.error('Failed to pauseRelayer:', error);
    throw error;
  }
};
