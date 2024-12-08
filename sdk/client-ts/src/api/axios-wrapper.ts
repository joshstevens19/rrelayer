import axios, {
  AxiosRequestConfig,
  AxiosResponse,
  RawAxiosRequestHeaders,
} from 'axios';
import { ApiBaseConfig } from './types';

const buildUrl = (serverUrl: string, endpoint: string): string => {
  return `${serverUrl}/${endpoint}`;
};

const buildHeaders = (
  baseConfig: ApiBaseConfig,
  knownHeaders: RawAxiosRequestHeaders = {}
): RawAxiosRequestHeaders => {
  let headers: RawAxiosRequestHeaders = {};
  if ('apiKey' in baseConfig) {
    headers = {
      ...headers,
      'x-api-key': baseConfig.apiKey,
    };
  }

  if ('authToken' in baseConfig) {
    headers = {
      ...knownHeaders,
      Authorization: `Bearer ${baseConfig.authToken}`,
    };
  }

  return {
    ...knownHeaders,
    ...headers,
    'Content-Type': 'application/json',
  };
};

export const getApi = async <T>(
  baseConfig: ApiBaseConfig,
  endpoint: string,
  params?: any,
  config?: AxiosRequestConfig
): Promise<AxiosResponse<T>> => {
  return axios.get<T>(buildUrl(baseConfig.serverUrl, endpoint), {
    ...config,
    params,
    headers: buildHeaders(baseConfig, config?.headers),
  });
};

export const postApi = async <T>(
  baseConfig: ApiBaseConfig,
  endpoint: string,
  body?: any,
  config?: AxiosRequestConfig
): Promise<AxiosResponse<T>> => {
  return axios.post<T>(buildUrl(baseConfig.serverUrl, endpoint), body, {
    ...config,
    headers: buildHeaders(baseConfig, config?.headers),
  });
};

export const putApi = async <T>(
  baseConfig: ApiBaseConfig,
  endpoint: string,
  body?: any,
  config?: AxiosRequestConfig
): Promise<AxiosResponse<T>> => {
  return axios.put<T>(buildUrl(baseConfig.serverUrl, endpoint), body, {
    ...config,
    headers: buildHeaders(baseConfig, config?.headers),
  });
};

export const deleteApi = async <T>(
  baseConfig: ApiBaseConfig,
  endpoint: string,
  config?: AxiosRequestConfig
): Promise<AxiosResponse<T>> => {
  return axios.delete<T>(buildUrl(baseConfig.serverUrl, endpoint), {
    ...config,
    headers: buildHeaders(baseConfig, config?.headers),
  });
};
