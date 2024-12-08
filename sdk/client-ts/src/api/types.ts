export type ApiBaseConfig =
  | {
      serverUrl: string;
      authToken: string;
    }
  | {
      serverUrl: string;
      apiKey: string;
    }
  | { serverUrl: string };

export interface PagingContext {
  limit: number;
  offset: number;
}

export const defaultPagingContext: PagingContext = {
  limit: 100,
  offset: 0,
};

export interface PagingResult<T> {
  items: T[];
  next?: PagingContext;
  previous?: PagingContext;
}
