import type { CredentialDialogValue } from "./CredentialDialog";
import { editModelSources, withModelEditors } from "./credentialModelSelection";
import { buildQwenEndpoint, isQwenProvider, requireQwenEndpoint } from "./qwenEndpoint";
import { catalogMatchesProvider, officialCatalogSchema } from "./officialModelCatalog";
import { hasFreshQwenCatalog, isQwenReplacementCatalog } from "./qwenModelCatalog";

export function clearQwenEvidence(value: CredentialDialogValue): CredentialDialogValue {
  const initialized = withModelEditors(value);
  return editModelSources({ ...initialized, discovery: undefined, discoveryStatus: undefined, modelCatalog: undefined },
    "", initialized.manualModelsText ?? "");
}
export function qwenCredentialBase(provider: Record<string, unknown>, value: CredentialDialogValue): string {
  if (value.qwenEndpoint) {
    const base = buildQwenEndpoint(value.qwenEndpoint);
    if (!base || base !== value.baseUrl) throw new Error("请选择 Qwen 地域并填写有效工作空间。");
    return base;
  }
  return requireQwenEndpoint(value.baseUrl || (typeof provider.base_url === "string" ? provider.base_url : ""));
}
export function normalizeQwenCredentialDraft(provider: Record<string, unknown> | undefined, value: CredentialDialogValue,
  previous?: CredentialDialogValue | null): CredentialDialogValue {
  if (!provider || !isQwenProvider(provider)) return value;
  const after = qwenCredentialBase(provider, value);
  let changed = false;
  if (previous) {
    try { changed = qwenCredentialBase(provider, previous) !== after; }
    catch { changed = true; } // An invalid/unknown old audience requires explicit replace or clear, not keep.
  }
  if (changed && value.apiKeyOperation === "keep") {
    throw new Error("Qwen 目标已变化，请明确录入匹配地域/工作空间的新 API Key，或清除原 Key；不能保留旧材料。");
  }
  if (value.apiKeyOperation === "keep" && isQwenReplacementCatalog(value.modelCatalog)) return clearQwenEvidence(value);
  const catalog = officialCatalogSchema.safeParse(value.modelCatalog).data;
  if (value.modelCatalog && (!catalog || !catalogMatchesProvider(provider, after, catalog))) {
    throw new Error("模型目录与 Qwen 账号目标不匹配。");
  }
  if (changed || value.apiKeyOperation !== "keep") {
    if (!hasFreshQwenCatalog(value, after)) return clearQwenEvidence(value);
  }
  // A freshly fetched native directory survives a replacement; old fetched evidence never becomes manual.
  return value.discovery ? clearQwenEvidence(value) : value;
}
export function changeQwenCredentialTarget(provider: Record<string, unknown>, value: CredentialDialogValue,
  patch: Pick<CredentialDialogValue, "baseUrl" | "qwenEndpoint">): CredentialDialogValue {
  const next = { ...value, ...patch };
  try { if (qwenCredentialBase(provider, value) === qwenCredentialBase(provider, next)) return next; }
  catch { /* Incomplete public target input is not an inherited address or fresh model evidence. */ }
  return clearQwenEvidence(next);
}
