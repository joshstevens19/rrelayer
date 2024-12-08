import { postApi } from '../../axios-wrapper';
import { ApiBaseConfig } from '../../types';

export const deleteRelayerApiKey = async (
  relayerId: string,
  apiKey: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    // it is a post due to delete not taking a body in general spec
    await postApi(baseConfig, `relayers/${relayerId}/api-keys/delete`, {
      apiKey,
    });
  } catch (error) {
    console.error('Failed to deleteRelayerApiKey:', error);
    throw error;
  }
};
