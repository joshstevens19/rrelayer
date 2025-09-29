export type ApiBaseConfig =
  | {
      serverUrl: string;
      apiKey: string;
    }
  | {
      serverUrl: string;
      username: string;
      password: string;
    };

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
