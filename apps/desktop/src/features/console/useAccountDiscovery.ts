import { useEffect, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { pushAppToast } from "../../components/AppToast";
import { applyAccountDiscovery, discoveryResponseSchema, supportsAccountDiscovery, type DiscoveryInput } from "./accountDiscovery";
import { parseRouteDocument } from "./routeDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useBackgroundDiscoverySync } from "./useBackgroundDiscoverySync";

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

  const discoverAccount = async (input: DiscoveryInput, signal?: AbortSignal) => {
    const initial = current.current;
    if (!initial.managementToken || !initial.secretGrant) {
      initial.requestSecretAccess(); throw new Error("请先确认敏感信息权限。");
    }
    if (pending.current) throw new Error("正在识别，请稍候。");
    const controller = new AbortController(); pending.current = controller; setBusy(true);
    const abort = () => controller.abort(); signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) controller.abort();
    try {
      const result = await initial.client.request("/v1/internal/gateway/console/account-discovery", discoveryResponseSchema, {
        method: "POST", managementToken: initial.managementToken, secretGrant: initial.secretGrant,
        body: input, signal: controller.signal,
      });
      const latest = current.current;
      if (controller.signal.aborted || latest.client !== initial.client || latest.managementToken !== initial.managementToken
        || latest.secretGrant !== initial.secretGrant || latest.editorText !== initial.editorText
        || latest.revision !== initial.revision || result.revision !== initial.revision) {
        throw new Error("配置或登录状态已变化，本次识别结果未保存，请重试。");
      }
      return result.discovery;
    } finally {
      signal?.removeEventListener("abort", abort);
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };

  const refreshAccountDiscovery = async (providerId: string, credentialId: string) => {
    if (current.current.draftDirty || pending.current) { current.current.setError("请等待当前配置保存后再刷新识别。"); return; }
    const doc = parseRouteDocument(current.current.editorText);
    const provider = doc.providers.find((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === providerId) as Record<string, unknown> | undefined;
    if (!provider || !supportsAccountDiscovery(provider)) { current.current.setError("此账号使用专用适配器，不支持通用 API 自动识别。"); return; }
    try {
      pushAppToast("info", "正在识别模型与协议…");
      const discovery = await discoverAccount({ credentialId });
      current.current.replaceEditorDocument(applyAccountDiscovery(doc, providerId, credentialId, discovery), true);
      current.current.setError(null);
    } catch (error) { current.current.setError(error instanceof Error ? error.message : String(error)); }
  };
  return { discoverAccount, refreshAccountDiscovery, discoveryBusy, editorLocked };
}
