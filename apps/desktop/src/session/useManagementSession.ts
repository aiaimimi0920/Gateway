import { useContext } from "react";
import { ManagementSessionContext, type ManagementSessionContextValue } from "./ManagementSessionProvider";

export function useManagementSession(): ManagementSessionContextValue {
  const session = useContext(ManagementSessionContext);
  if (!session) {
    throw new Error("useManagementSession must be used within ManagementSessionProvider.");
  }
  return session;
}
