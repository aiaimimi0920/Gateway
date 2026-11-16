import { Checkbox, Input } from "@neuro/ui";
import type { Dispatch, SetStateAction } from "react";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import { CatalogField } from "./ProviderCatalogFields";
import { supportsCatalogApiKey } from "./catalogFirstAccount";

export function CatalogFirstAccountFields({ template, draft, setDraft, locked, t }: {
  template: ProviderCatalogTemplate; draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>;
  locked: boolean; t(zh: string, en: string): string;
}) {
  if (!supportsCatalogApiKey(template)) return <p className="n-muted">{t("添加服务商后，在账号列表中完成授权或导入。", "After adding the provider, authorize or import from its account list.")}</p>;
  return <div className="nt-stack">
    <Checkbox label={t("同时添加首个账号", "Add first account")} checked={Boolean(draft.credentialId)} disabled={locked}
      onChange={e => setDraft(current => ({ ...current, credentialId: e.target.checked ? crypto.randomUUID() : undefined, apiKey: undefined, accountName: undefined }))} />
    {draft.credentialId && <>
      <CatalogField name={t("账号名称", "Account name")}><Input disabled={locked} value={draft.accountName ?? ""} onChange={e => setDraft(current => ({ ...current, accountName: e.target.value }))} /></CatalogField>
      <CatalogField name="API Key"><Input disabled={locked} type="password" autoComplete="new-password" value={draft.apiKey ?? ""} onChange={e => setDraft(current => ({ ...current, apiKey: e.target.value }))} /></CatalogField>
    </>}
  </div>;
}
