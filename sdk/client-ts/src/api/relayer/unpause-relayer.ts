import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const unpauseRelayer = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, `relayers/${relayerId}/unpause`);
  } catch (error) {
    console.error('Failed to unpauseRelayer:', error);
    throw error;
  }
};
