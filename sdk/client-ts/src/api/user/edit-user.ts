import { JwtRole } from '../authentication/types';
import { putApi } from '../axios-wrapper';
import { ApiBaseConfig } from '../types';

export const editUser = async (
  user: string,
  newRole: JwtRole,
  baseConfig: ApiBaseConfig
): Promise<void> => {
  try {
    await putApi(baseConfig, 'users/edit', {
      user,
      newRole,
    });
  } catch (error) {
    console.error('Failed to editUser:', error);
    throw error;
  }
};
