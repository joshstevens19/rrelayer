import { postApi } from '../../axios-wrapper';
import { ApiBaseConfig } from '../../types';

export const addRelayerAllowlistAddress = async (
  relayerId: string,
  address: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await postApi(baseConfig, `relayers/${relayerId}/allowlists/${address}`);
  } catch (error) {
    console.error('Failed to addRelayerAllowlistAddress:', error);
    throw error;
  }
};
