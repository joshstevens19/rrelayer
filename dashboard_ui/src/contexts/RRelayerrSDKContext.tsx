import { createContext } from 'react';
import { AdminClient } from 'rrelayerr-sdk/dist/clients/admin-client';
import { RRelayerrClient } from 'rrelayerr-sdk/dist/clients/core-client';

export interface RRelayerrSDKContextType {
  core: RRelayerrClient;
  admin: AdminClient;
}

export const RRelayerrSDKContext =
  createContext<RRelayerrSDKContextType | null>(null);
