import { postApi } from '../../axios-wrapper';
import { ApiBaseConfig } from '../../types';

export const createRelayerApiKey = async (
  relayerId: string,
  baseConfig: ApiBaseConfig
): Promise<string> => {
  try {
    const response = await postApi<string>(
      baseConfig,
      `relayers/${relayerId}/api-keys`
    );
    return response.data;
  } catch (error) {
    console.error('Failed to createRelayerApiKey', error);
    throw error;
  }
};
