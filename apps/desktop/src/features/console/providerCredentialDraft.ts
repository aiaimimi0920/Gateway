import type { ConsoleRouteDocument } from "../../api/contracts";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { CredentialDialogValue } from "./CredentialDialog";
import { optionalString, readStringArray } from "./routeAccountCatalog";
import { isRecord } from "./routeDocument";
import { accountDiscoverySchema } from "./accountDiscovery";
import { officialCatalogSchema } from "./officialModelCatalog";
import { modelSelectionSchema, withModelEditors } from "./credentialModelSelection";

export type ProviderDraftRow = {
  id: string;
  providerId: string;
  preset: string;
  vendorKey: string;
  vendorName: string;
  baseUrl: string;
  supportedModelsText: string;
  provider: Record<string, unknown>;
};

export function createProviderDraftRow(
  providerId = "",
  preset = "",
  vendorKey = "",
  vendorName = "",
  baseUrl = "",
  supportedModelsText = "",
  provider: Record<string, unknown> = {},
): ProviderDraftRow {
  return {
    id: `provider-${Math.random().toString(36).slice(2, 10)}`,
    providerId,
    preset,
    vendorKey,
    vendorName,
    baseUrl,
    supportedModelsText,
    provider,
  };
}

export function supportedModelsTextFromProvider(provider: Record<string, unknown>): string {
  const supportedModels = provider.supported_models;
  if (!Array.isArray(supportedModels)) {
    return "";
  }
  return supportedModels
    .filter((entry): entry is string => typeof entry === "string")
    .join("\n");
}

export function parseSupportedModelsText(value: string): string[] {
  return value
    .split(/\r?\n|,/)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function emptyCredentialDialogValue(providerId = ""): CredentialDialogValue {
  return {
    providerId,
    credentialId: "",
    accountName: "",
    enabled: true,
    baseUrl: "",
    supportedModelsText: "",
    apiKeyOperation: "replace",
    apiKeyValue: "",
  };
}

export function credentialDialogValueFromDocument(
  document: ConsoleRouteDocument,
  providerId: string,
  credentialId: string,
): CredentialDialogValue | null {
  const provider = document.providers.find(
    (entry) => isRecord(entry) && entry.id === providerId,
  );
  if (!isRecord(provider) || !Array.isArray(provider.credentials)) {
    return null;
  }
  const credential = provider.credentials.find(
    (entry) => isRecord(entry) && entry.id === credentialId,
  );
  if (!isRecord(credential)) {
    return null;
  }
  const selection = modelSelectionSchema.safeParse(credential.model_selection).data;
  const discovery = accountDiscoverySchema.safeParse(credential.discovery).data;
  const modelCatalog = officialCatalogSchema.safeParse(credential.model_catalog).data;
  const declared = readStringArray(credential.supported_models);
  // Legacy empty lists inherit provider models; opening the editor must not silently revoke them.
  const effective = selection ? [...selection.manual, ...selection.fetched]
    : [...(discovery?.models ?? (declared.length ? declared : readStringArray(provider.supported_models))), ...(modelCatalog?.models ?? [])];
  return withModelEditors({
    providerId,
    credentialId,
    chatgptGroup: optionalString(credential, "credential_identity_category_id") ?? undefined,
    codexBasispointsEnabled: typeof credential.codex_basispoints_enabled === "boolean" ? credential.codex_basispoints_enabled : undefined,
    discovery,
    modelCatalog,
    discoveryStatus: isRecord(credential.discovery_job) ? optionalString(credential.discovery_job, "status") ?? undefined : undefined,
    accountName: optionalString(credential, "account_name") ?? "",
    enabled: typeof credential.enabled === "boolean" ? credential.enabled : true,
    baseUrl: optionalString(credential, "base_url") ?? "",
    supportedModelsText: [...new Set(effective)].join("\n"),
    ...(selection ? { fetchedModelsText: selection.fetched.join("\n"), manualModelsText: selection.manual.join("\n") } : {}),
    apiKeyOperation: "keep",
    apiKeyValue: "",
  });
}

export function duplicateCredentialDialogValue(
  document: ConsoleRouteDocument,
  providerId: string,
  account: AccountsLedgerPilotAccount,
): CredentialDialogValue {
  const copiedValue = credentialDialogValueFromDocument(document, providerId, account.accountId);
  return {
    providerId,
    credentialId: `${account.accountId}-copy`,
    chatgptGroup: copiedValue?.chatgptGroup,
    accountName: "",
    enabled: account.enabled,
    codexBasispointsEnabled: copiedValue?.codexBasispointsEnabled,
    baseUrl: copiedValue?.baseUrl ?? "",
    supportedModelsText: copiedValue?.supportedModelsText ?? "",
    fetchedModelsText: "",
    manualModelsText: copiedValue?.supportedModelsText ?? "",
    apiKeyOperation: "replace",
    apiKeyValue: "",
  };
}

export function providerDraftRowsFromDocument(document: ConsoleRouteDocument): ProviderDraftRow[] {
  return document.providers.map((provider) => {
    if (isRecord(provider)) {
      return createProviderDraftRow(
        typeof provider.id === "string" ? provider.id : "",
        typeof provider.preset === "string" ? provider.preset : "",
        typeof provider.vendor_key === "string" ? provider.vendor_key : "",
        typeof provider.vendor_name === "string" ? provider.vendor_name : "",
        typeof provider.base_url === "string" ? provider.base_url : "",
        supportedModelsTextFromProvider(provider),
        provider,
      );
    }
    return createProviderDraftRow();
  });
}
