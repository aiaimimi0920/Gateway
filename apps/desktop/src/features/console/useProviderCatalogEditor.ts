import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { parseRouteDocument } from "./routeDocument";

type ProviderCatalogEditorOptions = {
  editorText: string;
  replaceEditorDocument(document: ConsoleRouteDocument, syncStructuredEditors?: boolean): void;
  stageProviderCreationSecrets(document: ConsoleRouteDocument, value: ProviderCatalogDraft): void;
  setActiveWorkspace(workspace: "accounts"): void;
  setError: Dispatch<SetStateAction<string | null>>;
  t(zh: string, en: string): string;
};
export function useProviderCatalogEditor({ editorText, replaceEditorDocument, stageProviderCreationSecrets, setActiveWorkspace, setError, t }: ProviderCatalogEditorOptions) {
  const applyProviderCatalogDraft = useCallback((value: ProviderCatalogDraft) => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((candidate) => candidate.id === value.templateId);
    if (!template) { setError(t(`找不到服务商模板 ${value.templateId}。`, `Provider template ${value.templateId} was not found.`)); return; }
    try {
      const next = addCatalogProvider(parseRouteDocument(editorText), template, value);
      stageProviderCreationSecrets(next, value);
      replaceEditorDocument(next, true);
      setActiveWorkspace("accounts"); setError(null);
      pushAppToast("info", t(`正在保存服务商 ${value.providerId}。`, `Saving provider ${value.providerId}.`));
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
  }, [editorText, replaceEditorDocument, stageProviderCreationSecrets, setActiveWorkspace, setError, t]);
  return { applyProviderCatalogDraft };
}
