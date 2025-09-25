import { postApi } from '../../axios-wrapper';
import { ApiBaseConfig } from '../../types';

export interface SignTextResult {
  messageSigned: string;
  signature: string;
}

export const signText = async (
  relayerId: string,
  text: string,
  baseConfig: ApiBaseConfig
): Promise<SignTextResult> => {
  try {
    const response = await postApi<SignTextResult>(
      baseConfig,
      `relayers/${relayerId}/sign/message`,
      {
        text,
      }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to signText', error);
    throw error;
  }
};
