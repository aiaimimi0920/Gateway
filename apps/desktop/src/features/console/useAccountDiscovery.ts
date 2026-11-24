import { useEffect, useRef, useState } from "react";
import type { ZodType } from "zod";
import type { GatewayApiClient } from "../../api/client";
import { pushAppToast } from "../../components/AppToast";
import { applyAccountDiscovery, discoveryResponseSchema, accountModelsResponseSchema, supportsAccountDiscovery, type DiscoveryInput } from "./accountDiscovery";
import { isRecord, parseRouteDocument } from "./routeDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useBackgroundDiscoverySync } from "./useBackgroundDiscoverySync";
import { applyChatgptCatalog, chatgptCatalogResponseSchema, isChatgptCatalogProvider, type ChatgptCatalogInput } from "./chatgptModelCatalog";
import { nvidiaCatalogResponseSchema, type NvidiaCatalogInput } from "./nvidiaModelCatalog";
import { applyOfficialCatalog, catalogProviderKind, catalogMatchesProvider, officialCatalogResponseSchema, type CatalogInput } from "./officialModelCatalog";
import { requireQwenEndpoint } from "./qwenEndpoint";
import { requireCloudflareBase } from "./cloudflareWorkersAi";

type Options = {
  editorBusy?: boolean;
  background?: Omit<NonNullable<Parameters<typeof useBackgroundDiscoverySync>[0]>, "editorText" | "managementToken">;
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  editorText: string; revision?: string; draftDirty: boolean;
  replaceEditorDocument(document: ConsoleRouteDocument, sync?: boolean): void;
  requestSecretAccess(): void; setError(error: string | null): void;
};

const staleDiscovery = () => new Error("配置或登录状态已变化，本次识别结果未保存，请重试。");

export function useAccountDiscovery(options: Options) {
  const current = useRef(options); current.current = options;
  const pending = useRef<AbortController | null>(null);
  const [discoveryBusy, setBusy] = useState(false);
  const editorLocked = !!options.editorBusy || discoveryBusy;
  useBackgroundDiscoverySync(options.background ? { ...options.background,
    editorText: options.editorText, managementToken: options.managementToken,
    blocked: editorLocked || options.background.blocked } : null);
  // Renewal changes the grant string, not the session's authority. Actual access loss still cancels I/O.
  const hasSecretAccess = !!options.secretGrant;
  useEffect(() => () => { pending.current?.abort(staleDiscovery()); pending.current = null; setBusy(false); },
    [options.client, options.managementToken, hasSecretAccess]);

  const requestDiscovery = async <T extends { revision: string }>(path: string, schema: ZodType<T>, input: unknown, signal?: AbortSignal) => {
    const initial = current.current;
    if (!initial.managementToken || !initial.secretGrant) {
      initial.requestSecretAccess(); throw new Error("请先确认敏感信息权限。");
    }
    if (pending.current) throw new Error("正在识别，请稍候。");
    const controller = new AbortController(); pending.current = controller; setBusy(true);
    const abort = () => controller.abort(signal?.reason); signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
    try {
      const result = await initial.client.request(path, schema, {
        method: "POST", managementToken: initial.managementToken, secretGrant: initial.secretGrant,
        body: input, signal: controller.signal,
      });
      controller.signal.throwIfAborted();
      const latest = current.current;
      if (latest.client !== initial.client || latest.managementToken !== initial.managementToken
        || !latest.secretGrant || latest.editorText !== initial.editorText
        || latest.revision !== initial.revision || result.revision !== initial.revision) {
        throw staleDiscovery();
      }
      return result;
    } finally {
      signal?.removeEventListener("abort", abort);
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const discoverAccount = async (input: DiscoveryInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/account-discovery", discoveryResponseSchema, input, signal)).discovery;
  const discoverAccountModels = async (input: Omit<DiscoveryInput, "models">, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/account-discovery", accountModelsResponseSchema, { ...input, catalogOnly: true }, signal)).catalog;
  const discoverChatgptModels = async (input: ChatgptCatalogInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/chatgpt-model-catalog", chatgptCatalogResponseSchema, input, signal)).catalog;
  const discoverNvidiaModels = async (input: NvidiaCatalogInput, signal?: AbortSignal) =>
    (await requestDiscovery("/v1/internal/gateway/console/nvidia-model-catalog", nvidiaCatalogResponseSchema, input, signal)).catalog;
  const discoverProviderModels = async (input: CatalogInput, signal?: AbortSignal) => {
    const doc = parseRouteDocument(current.current.editorText);
    const provider = doc.providers.find((p) => isRecord(p) && ("providerId" in input ? p.id === input.providerId
      : Array.isArray(p.credentials) && p.credentials.some((c) => isRecord(c) && c.id === input.credentialId)));
    const kind = isRecord(provider) ? catalogProviderKind(provider) : undefined;
    if (!kind || !isRecord(provider)) throw new Error("此服务商不支持官方模型目录。");
    if ((kind === "qwen" || kind === "cloudflare-workers-ai") && "credentialId" in input && (current.current.draftDirty || current.current.editorBusy)) {
      throw new Error("请先保存当前配置，再使用已保存账号拉取目录；新 Key 可使用显式录入模式。");
    }
    const credential = "credentialId" in input && Array.isArray(provider.credentials)
      ? provider.credentials.find((c) => isRecord(c) && c.id === input.credentialId) : undefined;
    const base = "providerId" in input && kind === "qwen" ? requireQwenEndpoint(input.accountBaseUrl ?? "")
      : "providerId" in input && kind === "cloudflare-workers-ai" ? requireCloudflareBase(input.accountBaseUrl ?? "")
      : isRecord(credential) ? credential.base_url ?? provider.base_url : provider.base_url;
    if (kind !== "qwen" && kind !== "cloudflare-workers-ai" && "accountBaseUrl" in input) throw new Error("此目录不接受动态账号目标。");
    const result = await requestDiscovery(`/v1/internal/gateway/console/${kind}-model-catalog`, officialCatalogResponseSchema, input, signal);
    if (!catalogMatchesProvider(provider, base, result.catalog)) throw new Error("模型目录与服务商或账号目标不匹配。");
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
  return { discoverAccount, discoverAccountModels, discoverChatgptModels, discoverNvidiaModels, discoverProviderModels, refreshAccountDiscovery, discoveryBusy, editorLocked };
}
