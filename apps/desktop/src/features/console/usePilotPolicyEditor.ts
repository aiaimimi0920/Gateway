import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument, ConsoleCredentialPoolAutomationResponse, ConsoleCredentialPoolAutomationProvider, ConsoleCredentialRefillResponse } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import { parseRouteDocument, isRecord } from "./routeDocument";
import { optionalString } from "./routeAccountCatalog";
import { type PilotIdentityCategoryDefinition, type PilotProviderPolicyDefinition, DEFAULT_PILOT_POOL_TARGET_SIZE, normalizePilotCategoryId, readPilotIdentityCategories, readPilotProviderPolicy } from "./pilotPoolPolicy";

type PilotPolicyEditorOptions = {
  editorText: string;
  setError: Dispatch<SetStateAction<string | null>>;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  credentialPoolAutomation: ConsoleCredentialPoolAutomationResponse | null;
  credentialPoolAutomationByProvider: ReadonlyMap<string, ConsoleCredentialPoolAutomationProvider>;
  credentialRefill: ConsoleCredentialRefillResponse | null;
  t: (zh: string, en: string) => string;
};

export function usePilotPolicyEditor({
  editorText,
  setError,
  replaceEditorDocument,
  credentialPoolAutomation,
  credentialPoolAutomationByProvider,
  credentialRefill,
  t,
}: PilotPolicyEditorOptions) {
  const handleAddIdentityCategory = useCallback(
    (providerId: string) => {
      const requestedLabel = window.prompt(
        t("输入新的账号类别名称。", "Enter the new identity class label."),
      );
      const label = requestedLabel?.trim() ?? "";
      if (label.length === 0) {
        return;
      }

      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const providerLabel = optionalString(provider, "label") ?? providerId;
      const existingCategories = readPilotIdentityCategories(provider, providerId, providerLabel);
      const existingIds = new Set(existingCategories.map((category) => category.id));
      const baseId = normalizePilotCategoryId(label, `identity-${existingCategories.length + 1}`);
      let nextId = baseId;
      let suffix = 2;
      while (existingIds.has(nextId)) {
        nextId = `${baseId}-${suffix}`;
        suffix += 1;
      }

      provider.credential_identity_categories = [
        ...existingCategories,
        {
          id: nextId,
          label,
          poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
          autoRefillEnabled: false,
          autoPruneEnabled: false,
        },
      ].map((category) => ({
        id: category.id,
        label: category.label,
        pool_target_size: category.poolTargetSize,
        auto_refill_enabled: category.autoRefillEnabled,
        auto_prune_enabled: category.autoPruneEnabled,
      }));

      replaceEditorDocument(document, true);
      setError(null);
      pushAppToast(
        "success",
        t(
          `服务商 ${providerLabel} 已新增账号类别 ${label}。`,
          `Provider ${providerLabel} now includes the identity class ${label}.`,
        ),
      );
    },
    [editorText, replaceEditorDocument, t],
  );

  const updatePilotIdentityCategoryPolicy = useCallback(
    (
      providerId: string,
      categoryId: string,
      updates: Partial<
        Pick<
          PilotIdentityCategoryDefinition,
          "poolTargetSize" | "autoRefillEnabled" | "autoPruneEnabled"
        >
      >,
    ) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const providerLabel = optionalString(provider, "label") ?? providerId;
      const existingCategories = readPilotIdentityCategories(provider, providerId, providerLabel);
      const nextCategories = existingCategories.map((category) =>
        category.id === categoryId
          ? {
              ...category,
              ...updates,
              poolTargetSize:
                updates.poolTargetSize !== undefined
                  ? Math.max(1, Math.floor(updates.poolTargetSize))
                  : category.poolTargetSize,
            }
          : category,
      );

      provider.credential_identity_categories = nextCategories.map((category) => ({
        id: category.id,
        label: category.label,
        pool_target_size: category.poolTargetSize,
        auto_refill_enabled: category.autoRefillEnabled,
        auto_prune_enabled: category.autoPruneEnabled,
      }));
      if (updates.autoRefillEnabled === true) {
        provider.auto_refill_enabled = true;
      }
      if (updates.autoPruneEnabled === true) {
        provider.auto_prune_enabled = true;
      }

      replaceEditorDocument(document, true);
      setError(null);
    },
    [editorText, replaceEditorDocument, t],
  );

  const updatePilotProviderPolicy = useCallback(
    (
      providerId: string,
      updates: Partial<
        Pick<
          PilotProviderPolicyDefinition,
          | "poolTargetSize"
          | "autoRefillEnabled"
          | "autoPruneEnabled"
          | "permanentDeleteEnabled"
        >
      >,
    ) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const currentPolicy = readPilotProviderPolicy(provider);
      provider.pool_target_size =
        updates.poolTargetSize !== undefined
          ? Math.max(1, Math.floor(updates.poolTargetSize))
          : currentPolicy.poolTargetSize;
      provider.auto_refill_enabled =
        updates.autoRefillEnabled !== undefined
          ? updates.autoRefillEnabled
          : currentPolicy.autoRefillEnabled;
      provider.auto_prune_enabled =
        updates.autoPruneEnabled !== undefined
          ? updates.autoPruneEnabled
          : currentPolicy.autoPruneEnabled;
      provider.credential_permanent_delete_enabled =
        updates.permanentDeleteEnabled !== undefined
          ? updates.permanentDeleteEnabled
          : currentPolicy.permanentDeleteEnabled;

      replaceEditorDocument(document, true);
      setError(null);
    },
    [editorText, replaceEditorDocument, t],
  );

  const updateProviderAutomationToggle = useCallback(
    (
      providerId: string,
      field: "autoRefillEnabled" | "autoPruneEnabled",
      nextEnabled: boolean,
    ) => {
      const automation = credentialPoolAutomationByProvider.get(providerId);
      const refillQueueAvailable = credentialRefill?.refill.enabled === true;
      const requiresDirectDriver = field === "autoPruneEnabled";
      if (
        nextEnabled &&
        credentialPoolAutomation &&
        !automation?.driverConfigured &&
        (requiresDirectDriver || !refillQueueAvailable)
      ) {
        pushAppToast(
          "warning",
          t(
            requiresDirectDriver
              ? `渠道 ${providerId} 尚未配置受信任的自动剔号驱动器。`
              : `渠道 ${providerId} 尚未配置补号队列或受信任驱动器。`,
            requiresDirectDriver
              ? `Provider ${providerId} does not have a trusted prune driver configured.`
              : `Provider ${providerId} has neither a refill queue nor a trusted driver.`,
          ),
        );
        return;
      }
      updatePilotProviderPolicy(providerId, { [field]: nextEnabled });
    },
    [
      credentialPoolAutomation,
      credentialPoolAutomationByProvider,
      credentialRefill,
      t,
      updatePilotProviderPolicy,
    ],
  );

  const updateIdentityCategoryAutomationToggle = useCallback(
    (
      providerId: string,
      categoryId: string,
      field: "autoRefillEnabled" | "autoPruneEnabled",
      nextEnabled: boolean,
    ) => {
      const automation = credentialPoolAutomationByProvider.get(providerId);
      const refillQueueAvailable = credentialRefill?.refill.enabled === true;
      const requiresDirectDriver = field === "autoPruneEnabled";
      if (
        nextEnabled &&
        credentialPoolAutomation &&
        !automation?.driverConfigured &&
        (requiresDirectDriver || !refillQueueAvailable)
      ) {
        pushAppToast(
          "warning",
          t(
            requiresDirectDriver
              ? `渠道 ${providerId} 尚未配置受信任的自动剔号驱动器。`
              : `渠道 ${providerId} 尚未配置补号队列或受信任驱动器。`,
            requiresDirectDriver
              ? `Provider ${providerId} does not have a trusted prune driver configured.`
              : `Provider ${providerId} has neither a refill queue nor a trusted driver.`,
          ),
        );
        return;
      }
      updatePilotIdentityCategoryPolicy(providerId, categoryId, { [field]: nextEnabled });
    },
    [
      credentialPoolAutomation,
      credentialPoolAutomationByProvider,
      credentialRefill,
      t,
      updatePilotIdentityCategoryPolicy,
    ],
  );

  return { handleAddIdentityCategory, updatePilotIdentityCategoryPolicy, updatePilotProviderPolicy, updateProviderAutomationToggle, updateIdentityCategoryAutomationToggle };
}
