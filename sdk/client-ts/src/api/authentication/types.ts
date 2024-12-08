export enum JwtRole {
  ADMIN = 'ADMIN',
  READONLY = 'READONLY',
  MANAGER = 'MANAGER',
  INTEGRATION = 'INTEGRATION',
}

export interface TokenPair {
  accessToken: string;
  refreshToken: string;
}
