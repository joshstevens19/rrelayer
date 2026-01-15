import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export interface ImportRelayerRequest {
  name: string;
  keyId: string;
  address: string;
}

export interface ImportRelayerResult {
  id: string;
  address: string;
  walletIndex: number;
  keyAlias: string;
}

export const importRelayer = async (
  chainId: number,
  name: string,
  keyId: string,
  address: string,
  baseConfig: ApiBaseConfig
): Promise<ImportRelayerResult> => {
  try {
    const response = await postApi<ImportRelayerResult>(
      baseConfig,
      `relayers/${chainId}/import`,
      { name, keyId, address }
    );
    return response.data;
  } catch (error) {
    console.error('Failed to import relayer', error);
    throw error;
  }
};
