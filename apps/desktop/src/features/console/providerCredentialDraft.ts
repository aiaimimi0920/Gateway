import type { ConsoleRouteDocument } from "../../api/contracts";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { CredentialDialogValue } from "./CredentialDialog";
import { optionalString, readStringArray } from "./routeAccountCatalog";
import { isRecord } from "./routeDocument";
import { accountDiscoverySchema } from "./accountDiscovery";
import { officialCatalogSchema, catalogMatchesProvider } from "./officialModelCatalog";
import { modelSelectionSchemaFor, withModelEditors } from "./credentialModelSelection";
import { modelIdPolicy } from "./providerModelId";
import { bedrockMaterialDraft, isBedrockProvider } from "./bedrockMaterial";
import { isQwenProvider } from "./qwenEndpoint";
import { dimModelCatalogSchema, savedDimBinding, dimCatalogMatches } from "./dimModelCatalog";
import { dimSite } from "./dimCatalog";
import { lobsteraiModelCatalogSchema, savedLobsterAiBinding } from "./lobsteraiModelCatalog";
import { isLobsterAiProvider } from "./lobsteraiCatalog";
import { commandcodeZdrDraft } from "./commandcodeZdr";
import { filterVercelAccountModels, VERCEL_CREDIT_AUTH_MODE, VERCEL_PRESET } from "./vercelFreeCatalog";

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
  const filterModels = (models: string[]) => filterVercelAccountModels(provider.preset, credential.auth_mode, models);
  const storedSelection = modelSelectionSchemaFor(modelIdPolicy(provider, credential)).safeParse(credential.model_selection).data;
  const selection = storedSelection && { manual: filterModels(storedSelection.manual), fetched: filterModels(storedSelection.fetched) };
  const discovery = accountDiscoverySchema.safeParse(credential.discovery).data;
  const modelCatalog = officialCatalogSchema.safeParse(credential.model_catalog).data;
  const dimCatalog = dimModelCatalogSchema.safeParse(credential.dim_catalog).data;
  const lobsteraiCatalog = lobsteraiModelCatalogSchema.safeParse(credential.lobsterai_catalog).data;
  if (isLobsterAiProvider(provider) && credential.lobsterai_catalog != null && (!lobsteraiCatalog || !savedLobsterAiBinding(provider, credentialId))) return null;
  const dimBinding = savedDimBinding(provider, credentialId);
  if (dimSite(provider) && credential.dim_catalog != null && (!dimCatalog || !dimBinding || !dimCatalogMatches(dimCatalog, dimBinding))) return null;
  if (isQwenProvider(provider) && credential.model_catalog != null
    && (!modelCatalog || !catalogMatchesProvider(provider, credential.base_url ?? provider.base_url, modelCatalog))) return null;
  const declared = readStringArray(credential.supported_models);
  // Legacy empty lists inherit provider models; opening the editor must not silently revoke them.
  const effective = selection ? [...selection.manual, ...selection.fetched]
    : [...(discovery?.models ?? (declared.length ? declared : readStringArray(provider.supported_models))), ...(modelCatalog?.models ?? [])];
  return withModelEditors({
    ...(isBedrockProvider(provider) ? { bedrockMaterial: bedrockMaterialDraft(provider, credential) } : {}),
    providerId,
    credentialId,
    chatgptGroup: optionalString(credential, "credential_identity_category_id") ?? undefined,
    codexBasispointsEnabled: typeof credential.codex_basispoints_enabled === "boolean" ? credential.codex_basispoints_enabled : undefined,
    commandcodeZdr: commandcodeZdrDraft(provider, credential),
    vercelMonthlyCredit: provider.preset === VERCEL_PRESET && credential.auth_mode === VERCEL_CREDIT_AUTH_MODE,
    discovery,
    modelCatalog,
    dimCatalog,
    lobsteraiCatalog,
    discoveryStatus: isRecord(credential.discovery_job) ? optionalString(credential.discovery_job, "status") ?? undefined : undefined,
    accountName: optionalString(credential, "account_name") ?? "",
    enabled: typeof credential.enabled === "boolean" ? credential.enabled : true,
    baseUrl: optionalString(credential, "base_url") ?? (provider.preset === "cloudflare-workers-ai" ? optionalString(provider, "base_url") ?? "" : ""),
    supportedModelsText: [...new Set(filterModels(effective))].join("\n"),
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
  const provider = document.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  // A duplicate has no Key; another account's fetched directory is not manual authorization.
  const models = isRecord(provider) && (isQwenProvider(provider) || provider.preset === "cloudflare-workers-ai")
    ? copiedValue?.manualModelsText ?? "" : copiedValue?.supportedModelsText ?? "";
  return {
    providerId,
    credentialId: `${account.accountId}-copy`,
    bedrockMaterial: copiedValue?.bedrockMaterial,
    chatgptGroup: copiedValue?.chatgptGroup,
    accountName: "",
    enabled: account.enabled,
    codexBasispointsEnabled: copiedValue?.codexBasispointsEnabled,
    commandcodeZdr: copiedValue?.commandcodeZdr,
    vercelMonthlyCredit: copiedValue?.vercelMonthlyCredit,
    baseUrl: copiedValue?.baseUrl ?? "",
    supportedModelsText: models,
    fetchedModelsText: "",
    manualModelsText: models,
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
