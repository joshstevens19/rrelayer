import { deleteApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const deleteUser = async (
  user: string,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await deleteApi(baseConfig, `users/${user}`);
  } catch (error) {
    console.error('Failed to deleteUser:', error);
    throw error;
  }
};
