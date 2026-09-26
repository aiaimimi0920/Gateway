import { useMemo } from "react";
import type { ConsoleCredentialPoolAutomationProvider, ConsoleCredentialRefillDemand } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { optionalString } from "./routeAccountCatalog";
import type { useConsoleRouteData } from "./useConsoleRouteData";
import type { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import type { useConsoleAccountSelectors } from "./useConsoleAccountSelectors";

type RouteData = ReturnType<typeof useConsoleRouteData>;
type ConsoleCredentialSelectorsOptions = {
  credentialPoolAutomation: RouteData["credentialPoolAutomation"];
  credentialRefill: RouteData["credentialRefill"];
  draftDocumentState: ReturnType<typeof useConsoleRouteDraft>["draftDocumentState"];
  draftAccountCatalog: ReturnType<typeof useConsoleAccountSelectors>["draftAccountCatalog"];
};

function providerIdFromValue(provider: unknown, index: number): string {
  if (
    typeof provider === "object" &&
    provider !== null &&
    "id" in provider &&
    typeof provider.id === "string"
  ) {
    return provider.id;
  }
  return `provider-${index}`;
}

export function useConsoleCredentialSelectors({
  credentialPoolAutomation,
  credentialRefill,
  draftDocumentState,
  draftAccountCatalog,
}: ConsoleCredentialSelectorsOptions) {
  const credentialPoolAutomationByProvider = useMemo(
    () =>
      new Map<string, ConsoleCredentialPoolAutomationProvider>(
        credentialPoolAutomation?.automation.providers.map((provider) => [
          provider.providerId,
          provider,
        ]) ?? [],
      ),
    [credentialPoolAutomation],
  );
  const credentialRefillByProvider = useMemo(
    () =>
      new Map<string, ConsoleCredentialRefillDemand>(
        credentialRefill?.refill.providers.map((provider) => [provider.providerId, provider]) ?? [],
      ),
    [credentialRefill],
  );
  const credentialProviderOptions = useMemo(
    () =>
      (draftDocumentState.document?.providers ?? []).flatMap((provider, index) => {
        if (!isRecord(provider)) {
          return [];
        }
        const id = providerIdFromValue(provider, index);
        return [
          {
            id,
            label: optionalString(provider, "label") ?? id,
          },
        ];
      }),
    [draftDocumentState.document],
  );
  const explicitCredentialIds = useMemo(
    () =>
      draftAccountCatalog.accounts
        .filter((account) => account.mode === "credential")
        .map((account) => account.id),
    [draftAccountCatalog.accounts],
  );

  return {
    credentialPoolAutomationByProvider,
    credentialRefillByProvider,
    credentialProviderOptions,
    explicitCredentialIds,
  };
}
