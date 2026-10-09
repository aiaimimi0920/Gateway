import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import type { CredentialDialogValue } from "./CredentialDialog";
import { addProviderWithCredential } from "./credentialDocument";
import {
  PROVIDER_CATALOG_TEMPLATES,
  providerDefinitionFromCatalogDraft,
  type ProviderCatalogDraft,
} from "./providerCatalog";
import { parseRouteDocument } from "./routeDocument";

type ProviderCatalogEditorOptions = {
  editorText: string;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  updateCredentialSecretEdit: (value: CredentialDialogValue) => void;
  setActiveWorkspace: (workspace: "accounts") => void;
  setError: Dispatch<SetStateAction<string | null>>;
  t: (zh: string, en: string) => string;
};

export function useProviderCatalogEditor({
  editorText,
  replaceEditorDocument,
  updateCredentialSecretEdit,
  setActiveWorkspace,
  setError,
  t,
}: ProviderCatalogEditorOptions) {
  const applyProviderCatalogDraft = useCallback(
    (value: ProviderCatalogDraft) => {
      const template = PROVIDER_CATALOG_TEMPLATES.find(
        (candidate) => candidate.id === value.templateId,
      );
      if (!template) {
        setError(
          t(
            `找不到服务商模板 ${value.templateId}。`,
            `Provider template ${value.templateId} could not be found.`,
          ),
        );
        return;
      }

      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      try {
        const provider = providerDefinitionFromCatalogDraft(template, value);
        const nextDocument = addProviderWithCredential(document, {
          provider,
          credential: {
            id: value.credentialId,
            account_name: value.accountName || undefined,
            enabled: true,
            supported_models: [...value.supportedModels],
            discovery: value.discovery,
          },
          routePatterns: value.supportedModels,
        });
        updateCredentialSecretEdit({
          providerId: value.providerId,
          credentialId: value.credentialId,
          accountName: value.accountName,
          enabled: true,
          baseUrl: "",
          supportedModelsText: value.supportedModels.join("\n"),
          apiKeyOperation: "replace",
          apiKeyValue: value.apiKey,
        });
        replaceEditorDocument(nextDocument, true);
        setActiveWorkspace("accounts");
        setError(null);
        pushAppToast(
          "info",
          t(
            `服务商 ${value.providerLabel}、首个账号和 ${value.supportedModels.length} 条模型聚合路由已写入草稿。保存路由配置后生效。`,
            `Provider ${value.providerLabel}, its first account, and ${value.supportedModels.length} model aggregation routes were added to the draft. Save the route config to apply them.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, setActiveWorkspace, setError, t, updateCredentialSecretEdit],
  );

  return { applyProviderCatalogDraft };
}
