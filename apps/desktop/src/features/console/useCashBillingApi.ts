import { useMemo } from "react";
import { createGatewayApiClient } from "../../api/client";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { cashBillingApi } from "./cashBillingApi";

// Dialog-local clients issue no background reads when the financial view is closed.
export function useCashBillingApi() {
  const host = useGatewayHost();
  const session = useManagementSession();
  const api = useMemo(
    () => cashBillingApi(createGatewayApiClient({ host })),
    [host],
  );
  const token =
    session.phase === "authenticated" ? session.managementToken : null;
  return { api, token, sessionBusy: session.busy };
}
