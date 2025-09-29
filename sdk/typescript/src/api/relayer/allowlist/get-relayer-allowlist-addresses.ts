import { getApi } from '../../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../../types';

export const getRelayerAllowlistAddress = async (
  relayerId: string,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<`0x${string}`>> => {
  try {
    const response = await getApi<PagingResult<`0x${string}`>>(
      baseConfig,
      `relayers/${relayerId}/allowlists`,
      { ...pagingContext }
    );

    return response.data;
  } catch (error) {
    console.error('Failed to getRelayerAllowlistAddress:', error);
    throw error;
  }
};
