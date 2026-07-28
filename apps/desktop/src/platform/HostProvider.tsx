import { createContext, type ReactNode, useContext } from "react";
import { BrowserRouter, HashRouter } from "react-router";
import type { GatewayHostAdapter } from "./types";

const GatewayHostContext = createContext<GatewayHostAdapter | undefined>(undefined);

export type HostProviderProps = {
  adapter: GatewayHostAdapter;
  children: ReactNode;
};

export function HostProvider({ adapter, children }: HostProviderProps) {
  const content = (
    <GatewayHostContext.Provider value={adapter}>{children}</GatewayHostContext.Provider>
  );
  if (adapter.routing.mode === "browser") {
    return <BrowserRouter basename={adapter.routing.basename}>{content}</BrowserRouter>;
  }
  return <HashRouter>{content}</HashRouter>;
}

export function useGatewayHost(): GatewayHostAdapter {
  const host = useContext(GatewayHostContext);
  if (!host) {
    throw new Error("useGatewayHost must be used within HostProvider.");
  }
  return host;
}
