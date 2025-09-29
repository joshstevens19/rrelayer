import { deleteApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const deleteRelayer = async (
  id: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await deleteApi(baseConfig, `relayers/${id}`);
  } catch (error) {
    console.error('Failed to deleteRelayer', error);
    throw error;
  }
};
