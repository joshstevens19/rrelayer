import { getApi } from '../../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../../types';

export const getRelayerApiKeys = async (
  relayerId: string,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<string>> => {
  try {
    const response = await getApi<PagingResult<string>>(
      baseConfig,
      `relayers/${relayerId}/api-keys`,
      { ...pagingContext }
    );

    return response.data;
  } catch (error) {
    console.error('Failed to getRelayerApiKeys:', error);
    throw error;
  }
};
