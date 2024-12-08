import { getApi } from '../axios-wrapper';
import { ApiBaseConfig, PagingContext, PagingResult } from '../types';
import { User } from './types';

export const getUsers = async (
  pagingContext: PagingContext,
  baseConfig: ApiBaseConfig
): Promise<PagingResult<User>> => {
  try {
    const result = await getApi<PagingResult<User>>(baseConfig, 'users', {
      ...pagingContext,
    });

    return result.data;
  } catch (error) {
    console.error('Failed to getUsers:', error);
    throw error;
  }
};
