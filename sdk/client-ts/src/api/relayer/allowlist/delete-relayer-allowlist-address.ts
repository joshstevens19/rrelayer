import { deleteApi } from '../../axios-wrapper';
import { ApiBaseConfig } from '../../types';

export const deleteRelayerAllowlistAddress = async (
  relayerId: string,
  address: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await deleteApi(baseConfig, `relayers/${relayerId}/allowlists/${address}`);
  } catch (error) {
    console.error('Failed to deleteRelayerAllowlistAddress:', error);
    throw error;
  }
};
