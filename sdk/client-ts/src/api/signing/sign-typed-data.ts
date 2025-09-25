import { postApi } from '../../axios-wrapper';

import { TypedDataDefinition } from 'viem/_types/types/typedData';
import { ApiBaseConfig } from '../../types';

export interface SignTypedDataResult {
  signature: string;
}

export const signTypedData = async (
  relayerId: string,
  typedData: TypedDataDefinition,
  baseConfig: ApiBaseConfig
): Promise<SignTypedDataResult> => {
  try {
    const response = await postApi<SignTypedDataResult>(
      baseConfig,
      `relayers/${relayerId}/sign/typed-data`,
      typedData
    );
    return response.data;
  } catch (error) {
    console.error('Failed to signText', error);
    throw error;
  }
};
