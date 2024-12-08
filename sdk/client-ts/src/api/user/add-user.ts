import { JwtRole } from '../authentication/types';
import { postApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const addUser = async (
  user: string,
  role: JwtRole,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await postApi(baseConfig, 'users/add', {
      user,
      role,
    });
  } catch (error) {
    console.error('Failed to addUser:', error);
    throw error;
  }
};
