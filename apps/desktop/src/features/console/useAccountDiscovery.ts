import { useEffect, useRef, useState } from "react";
import type { ZodType } from "zod";
import type { GatewayApiClient } from "../../api/client";
import { pushAppToast } from "../../components/AppToast";
import { applyAccountDiscovery, discoveryResponseSchema, supportsAccountDiscovery, type DiscoveryInput } from "./accountDiscovery";
import { isRecord, parseRouteDocument } from "./routeDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useBackgroundDiscoverySync } from "./useBackgroundDiscoverySync";
import { applyChatgptCatalog, chatgptCatalogResponseSchema, isChatgptCatalogProvider, type ChatgptCatalogInput } from "./chatgptModelCatalog";
import { nvidiaCatalogResponseSchema, type NvidiaCatalogInput } from "./nvidiaModelCatalog";
import { applyOfficialCatalog, catalogProviderKind, catalogSources, officialCatalogResponseSchema, type CatalogInput } from "./officialModelCatalog";

type Options = {
  editorBusy?: boolean;
  background?: Omit<NonNullable<Parameters<typeof useBackgroundDiscoverySync>[0]>, "editorText" | "managementToken">;
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  editorText: string; revision?: string; draftDirty: boolean;
  replaceEditorDocument(document: ConsoleRouteDocument, sync?: boolean): void;
  requestSecretAccess(): void; setError(error: string | null): void;
};

export function useAccountDiscovery(options: Options) {
  const current = useRef(options); current.current = options;
  const pending = useRef<AbortController | null>(null);
  const [discoveryBusy, setBusy] = useState(false);
  const editorLocked = !!options.editorBusy || discoveryBusy;
  useBackgroundDiscoverySync(options.background ? { ...options.background,
    editorText: options.editorText, managementToken: options.managementToken,
    blocked: editorLocked || options.background.blocked } : null);
  useEffect(() => () => { pending.current?.abort(); pending.current = null; setBusy(false); },
    [options.client, options.managementToken, options.secretGrant]);

  const requestDiscovery = async <T extends { revision: string }>(path: string, schema: ZodType<T>, input: unknown, signal?: AbortSignal) => {
    const initial = current.current;
    if (!initial.managementToken || !initial.secretGrant) {
      initial.requestSecretAccess(); throw new Error("请先确认敏感信息权限。");
    }
    if (pending.current) throw new Error("正在识别，请稍候。");
    const controller = new AbortController(); pending.current = controller; setBusy(true);
    const abort = () => controller.abort(); signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) controller.abort();
    try {
      const result = await initial.client.request(path, schema, {
        method: "POST", managementToken: initial.managementToken, secretGrant: initial.secretGrant,
        body: input, signal: controller.signal,
      });
      const latest = current.current;
      if (controller.signal.aborted || latest.client !== initial.client || latest.managementToken !== initial.managementToken
        || latest.secretGrant !== initial.secretGrant || latest.editorText !== initial.editorText
        || latest.revision !== initial.revision || result.revision !== initial.revision) {
        throw new Error("配置或登录状态已变化，本次识别结果未保存，请重试。");
      }
      return result;
    } finally {
      signal?.removeEventListener("abort", abort);
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const discoverAccount = async (input: DiscoveryInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/account-discovery", discoveryResponseSchema, input, signal)).discovery;
  const discoverChatgptModels = async (input: ChatgptCatalogInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/chatgpt-model-catalog", chatgptCatalogResponseSchema, input, signal)).catalog;
  const discoverNvidiaModels = async (input: NvidiaCatalogInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/nvidia-model-catalog", nvidiaCatalogResponseSchema, input, signal)).catalog;
  const discoverProviderModels = async (input: CatalogInput, signal?: AbortSignal) => {
    const doc = parseRouteDocument(current.current.editorText);
    const provider = doc.providers.find((p) => isRecord(p) && ("providerId" in input ? p.id === input.providerId
      : Array.isArray(p.credentials) && p.credentials.some((c) => isRecord(c) && c.id === input.credentialId)));
    const kind = isRecord(provider) ? catalogProviderKind(provider) : undefined;
    if (!kind) throw new Error("此服务商不支持官方模型目录。");
    const result = await requestDiscovery(`/v1/internal/gateway/console/${kind}-model-catalog`, officialCatalogResponseSchema, input, signal);
    if (result.catalog.sourceUrl !== catalogSources[kind]) throw new Error("模型目录与服务商不匹配。");
    return result.catalog;
  };

  const refreshAccountDiscovery = async (providerId: string, credentialId: string) => {
    if (current.current.editorBusy || current.current.draftDirty || pending.current) { current.current.setError("请等待当前配置保存后再刷新识别。"); return; }
    const doc = parseRouteDocument(current.current.editorText);
    const provider = doc.providers.find((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === providerId) as Record<string, unknown> | undefined;
    if (!provider || (!catalogProviderKind(provider) && !isChatgptCatalogProvider(provider) && !supportsAccountDiscovery(provider))) { current.current.setError("此账号使用专用适配器，不支持通用 API 自动识别。"); return; }
    try {
      if (catalogProviderKind(provider)) {
        pushAppToast("info", "正在读取模型目录，不执行模型调用…");
        const catalog = await discoverProviderModels({ credentialId });
        current.current.replaceEditorDocument(applyOfficialCatalog(doc, providerId, credentialId, catalog), true);
      } else if (isChatgptCatalogProvider(provider)) {
        pushAppToast("info", "正在读取 ChatGPT 真实模型目录；目录可见不代表逐模型生成验证…");
        const catalog = await discoverChatgptModels({ credentialId });
        current.current.replaceEditorDocument(applyChatgptCatalog(doc, providerId, credentialId, catalog), true);
      } else {
        pushAppToast("info", "正在识别模型与协议…");
        const discovery = await discoverAccount({ credentialId });
        current.current.replaceEditorDocument(applyAccountDiscovery(doc, providerId, credentialId, discovery), true);
      }
      current.current.setError(null);
    } catch (error) { current.current.setError(error instanceof Error ? error.message : String(error)); }
  };
  return { discoverAccount, discoverChatgptModels, discoverNvidiaModels, discoverProviderModels, refreshAccountDiscovery, discoveryBusy, editorLocked };
}
