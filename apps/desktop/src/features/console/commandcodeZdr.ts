import type { CredentialDialogValue } from "./credentialDialogTypes";
import { catalogProviderKind } from "./officialModelCatalog";
import { isRecord } from "./routeDocument";

/** An account may require ZDR, but cannot disable its provider's requirement. */
export function commandcodeZdrDraft(provider: Record<string, unknown>, credential: Record<string, unknown>): CredentialDialogValue["commandcodeZdr"] {
  if (catalogProviderKind(provider) !== "commandcode") return undefined;
  return isRecord(credential.headers) && Object.entries(credential.headers).some(([name, value]) => name.toLowerCase() === "x-cmd-zdr" && value === "1") ? "required" : "inherit";
}

export function commandcodeZdrFields(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): Record<string, unknown> {
  if (!provider || catalogProviderKind(provider) !== "commandcode" || value.commandcodeZdr === undefined) return {};
  const credential = Array.isArray(provider.credentials) ? provider.credentials.find((entry) => isRecord(entry) && entry.id === value.credentialId) : undefined;
  const headers = isRecord(credential) && isRecord(credential.headers) ? { ...credential.headers } : {};
  for (const name of Object.keys(headers)) if (name.toLowerCase() === "x-cmd-zdr") delete headers[name];
  if (value.commandcodeZdr === "required") headers["x-cmd-zdr"] = "1";
  return { headers };
}
