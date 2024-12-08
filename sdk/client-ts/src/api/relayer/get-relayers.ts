import { getApi } from '../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../types';
import { Relayer } from './types';

export const getRelayers = async (
  chainId: string | number | undefined,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<Relayer>> => {
  try {
    const response = await getApi<PagingResult<Relayer>>(
      baseConfig,
      'relayers',
      { chainId, ...pagingContext }
    );

    return response.data;
  } catch (error) {
    console.error('Failed to fetch getRelayers', error);
    throw error;
  }
};
