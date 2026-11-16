// Stable selections and explicit business consent use the ordinary credential save owner.
import { z } from "zod";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { editModelSources, withModelEditors } from "./credentialModelSelection";
import { freebuffBusinessOrigin, freebuffCatalogResponse, freebuffClient, type FreebuffCatalog } from "./freebuffCatalog";
import { validModelId } from "./providerModelId";
import { isRecord } from "./routeDocument";

export const freebuffBusinessTarget = z.object({ origin: freebuffBusinessOrigin, client: freebuffClient }).strict();
export type FreebuffBusinessTarget = z.infer<typeof freebuffBusinessTarget>;
export function savedFreebuffModelKeys(provider: Record<string, unknown> | undefined, credentialId: string): string[] {
  if (!savedFreebuffBusiness(provider, credentialId) || !Array.isArray(provider?.credentials)) return [];
  const credential = provider.credentials.find((c) => isRecord(c) && c.id === credentialId);
  return isRecord(credential) && Array.isArray(credential.supported_models)
    ? [...new Set(credential.supported_models.filter((key): key is string => typeof key === "string" && validModelId(key)))] : [];
}
export function savedFreebuffBusiness(provider: Record<string, unknown> | undefined, credentialId: string): FreebuffBusinessTarget | undefined {
  if (provider?.preset !== "freebuff" || !Array.isArray(provider.credentials)) return undefined;
  const credential = provider.credentials.find((c) => isRecord(c) && c.id === credentialId);
  return isRecord(credential) && isRecord(credential.extra_body)
    ? freebuffBusinessTarget.safeParse(credential.extra_body.freebuffBusiness).data : undefined;
}

export function stageFreebuffCatalog(value: CredentialDialogValue, response: FreebuffCatalog, keys: string[]): CredentialDialogValue {
  const catalog = freebuffCatalogResponse.parse(response);
  if (value.providerId !== catalog.providerId || value.credentialId !== catalog.credentialId || value.apiKeyOperation !== "keep") {
    throw new Error("目录与当前已保存账号不匹配。");
  }
  const selected = [...new Set(keys)];
  if (!selected.length || selected.some((key) => !validModelId(key) || !catalog.models.some((row) => row.key === key && row.access === "open"))) {
    throw new Error("请选择目录中的开放模型。");
  }
  const current = withModelEditors(value);
  return { ...editModelSources(current, selected.join("\n"), current.manualModelsText ?? ""),
    freebuffBusiness: freebuffBusinessTarget.parse({ origin: catalog.businessOrigin, client: catalog.client }) };
}

/** Only the edited account receives this non-secret target; no provider inheritance or token extraction. */
export function freebuffBusinessFields(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): Record<string, unknown> {
  if (value.freebuffBusiness === undefined) return {};
  const credential = Array.isArray(provider?.credentials) ? provider.credentials.find((c) => isRecord(c) && c.id === value.credentialId) : undefined;
  if (provider?.preset !== "freebuff" || !isRecord(credential) || value.apiKeyOperation !== "keep") {
    throw new Error("FreeBuff 业务目标只能应用到原已保存账号。");
  }
  return { extra_body: { ...(isRecord(credential.extra_body) ? credential.extra_body : {}),
    freebuffBusiness: freebuffBusinessTarget.parse(value.freebuffBusiness) } };
}
