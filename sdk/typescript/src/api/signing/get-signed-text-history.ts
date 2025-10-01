import { getApi } from '../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../types';

export interface SignedTextHistory {
  relayerId: string;
  message: string;
  signature: string;
  chainId: number;
  signedAt: Date;
}

export const getSignedTextHistory = async (
  relayerId: string,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<SignedTextHistory>> => {
  try {
    const response = await getApi<PagingResult<SignedTextHistory>>(
      baseConfig,
      `relayers/${relayerId}/allowlists`,
      { ...pagingContext }
    );

    return response.data;
  } catch (error) {
    console.error('Failed to getSignedTextHistory:', error);
    throw error;
  }
};
