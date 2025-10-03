import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';
import { RATE_LIMIT_HEADER_NAME } from '../index';

export interface SignTextResult {
  messageSigned: string;
  signature: string;
  signedBy: `0x${string}`;
}

export const signText = async (
  relayerId: string,
  text: string,
  rateLimitKey: string | undefined,
  baseConfig: ApiBaseConfig
): Promise<SignTextResult> => {
  try {
    const config: any = {};
    if (rateLimitKey) {
      config.headers = {
        [RATE_LIMIT_HEADER_NAME]: rateLimitKey,
      };
    }

    const response = await postApi<SignTextResult>(
      baseConfig,
      `signing/relayers/${relayerId}/message`,
      { text },
      config
    );
    return response.data;
  } catch (error) {
    console.error('Failed to signText', error);
    throw error;
  }
};
