import { getApi } from '../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../types';

export interface SignedTypedDataHistory {
  relayerId: string;
  domainData: unknown;
  messageData: unknown;
  primaryType: string;
  signature: string;
  chainId: number;
  signedAt: Date;
}

export const getSignedTypedDataHistory = async (
  relayerId: string,
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<SignedTypedDataHistory>> => {
  try {
    const response = await getApi<PagingResult<SignedTypedDataHistory>>(
      baseConfig,
      `signing/relayers/${relayerId}/typed-data-history`,
      { ...pagingContext }
    );

    return response.data;
  } catch (error) {
    console.error('Failed to getSignedTypedDataHistory:', error);
    throw error;
  }
};
