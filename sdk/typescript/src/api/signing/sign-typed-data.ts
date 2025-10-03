import { postApi } from '../axios-wrapper';
import { TypedDataDefinition } from 'viem/_types/types/typedData';
import { ApiBaseConfig } from '../types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export interface SignTypedDataResult {
  signature: string;
}

export const signTypedData = async (
  relayerId: string,
  typedData: TypedDataDefinition,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<SignTypedDataResult> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await postApi<SignTypedDataResult>(
      baseConfig,
      `signing/relayers/${relayerId}/typed-data`,
      typedData,
      config
    );
    return response.data;
  } catch (error) {
    console.error('Failed to signTypedData', error);
    throw error;
  }
};
