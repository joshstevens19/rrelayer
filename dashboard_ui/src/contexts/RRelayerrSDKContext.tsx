import { createContext } from 'react';
import { AdminClient } from 'rrelayer-sdk/dist/clients/admin-client';
import { RRelayerrClient } from 'rrelayer-sdk/dist/clients/core-client';

export interface RRelayerrSDKContextType {
  core: RRelayerrClient;
  admin: AdminClient;
}

export const RRelayerrSDKContext =
  createContext<RRelayerrSDKContextType | null>(null);
