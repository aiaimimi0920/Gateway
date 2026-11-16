import { Input } from "@neuro/ui";
import type { Dispatch, SetStateAction } from "react";
import { CatalogField } from "./ProviderCatalogFields";
import type { ProviderCatalogDraft } from "./providerCatalog";
/** Only deployment routing fields belong to the pool; secrets are entered on the account. */
export function SapAiCoreFields({ draft, setDraft, disabled }: { draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>; disabled: boolean }) {
  const target = draft.sapTarget ?? { resourceGroup: "", apiVersion: "" };
  return <>
    <CatalogField name="AI-Resource-Group"><Input value={target.resourceGroup} disabled={disabled} maxLength={253}
      onChange={(event) => setDraft((current) => ({ ...current, sapTarget: { ...target, resourceGroup: event.target.value } }))} /></CatalogField>
    <CatalogField name="API Version"><Input value={target.apiVersion} disabled={disabled} maxLength={18} placeholder="2024-10-21"
      onChange={(event) => setDraft((current) => ({ ...current, sapTarget: { ...target, apiVersion: event.target.value } }))} /></CatalogField>
  </>;
}
