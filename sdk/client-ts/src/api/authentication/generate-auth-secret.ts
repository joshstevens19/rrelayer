import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export interface GenerateAuthSecretResult {
  id: string;
  challenge: string;
}

export const generateAuthSecret = async (
  address: string,
  baseConfig: ApiBaseConfig
): Promise<GenerateAuthSecretResult> => {
  try {
    console.log('generateAuthSecret', address);
    const result = await postApi<GenerateAuthSecretResult>(
      baseConfig,
      'authentication/secret/generate',
      {
        address: address,
      }
    );

    return result.data;
  } catch (error) {
    console.error('Failed to generateAuthSecret:', error);
    throw error;
  }
};
