import { Button, Checkbox, Dialog, Notice } from "@neuro/ui";
import { useEffect, useState } from "react";
import type { ConsoleRouteDocument, ConsoleRouteConfigResponse } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { QwenEndpointFields } from "./QwenEndpointFields";
import { buildQwenEndpoint, emptyQwenEndpoint, isQwenProvider, parseQwenEndpoint, type QwenEndpointDraft } from "./qwenEndpoint";

type Props = {
  providerId: string; providerLabel: string; document: ConsoleRouteDocument | null;
  activeConfig: ConsoleRouteConfigResponse | null; locked: boolean;
  onApply(providerId: string, expected: string, target: QwenEndpointDraft, preserveInherited: boolean): boolean;
  t(zh: string, en: string): string;
};
/** Dialog-local public inputs only. No Key reads, probes, catalog requests or separate persistence. */
export function QwenProviderEndpointSettings({ providerId, providerLabel, document, activeConfig, locked, onApply, t }: Props) {
  const provider = document?.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  const [snapshot, setSnapshot] = useState<string | null>(null);
  const [target, setTarget] = useState<QwenEndpointDraft>(emptyQwenEndpoint);
  const [preserveInherited, setPreserveInherited] = useState(false);
  useEffect(() => { if (locked) setSnapshot(null); }, [locked]);
  if (!isRecord(provider) || !isQwenProvider(provider)) return null;
  const activeIndex = activeConfig?.routeConfig.document.providers.findIndex((entry) => isRecord(entry) && entry.id === providerId) ?? -1;
  const parentMaterial = activeConfig?.routeConfig.secrets.some((secret) => secret.configured
    && secret.path.startsWith(`/providers/${activeIndex}/`)
    && /^(api_key|auth_token|refresh_token|headers\/|extra_body\/|keepalive\/)/.test(secret.path.slice(`/providers/${activeIndex}/`.length))) ?? false;
  const unsupported = provider.explicit_credentials_only !== true || parentMaterial;
  const current = JSON.stringify(provider);
  const stale = snapshot !== null && current !== snapshot;
  const before = typeof provider.base_url === "string" ? parseQwenEndpoint(provider.base_url) : null;
  const after = buildQwenEndpoint(target);
  const changed = after !== (before ? buildQwenEndpoint(before) : null);
  const inherited = Array.isArray(provider.credentials) ? provider.credentials.filter((entry) => isRecord(entry) && entry.base_url == null).length : 0;
  const validBefore = before !== null || ((provider.base_url == null || provider.base_url === "") && inherited === 0);
  const openChange = (open: boolean) => {
    if (!open) { setSnapshot(null); return; }
    if (locked || unsupported) return;
    setTarget(before ?? emptyQwenEndpoint()); setPreserveInherited(false); setSnapshot(current);
  };
  return <Dialog open={snapshot !== null && !locked} onOpenChange={openChange}
    title={t(`${providerLabel} · 默认目标`, `${providerLabel} · Default target`)} closeLabel={t("关闭", "Close")}
    trigger={<Button compact disabled={locked || unsupported}
      title={unsupported ? t("仅支持没有父级秘密材料的显式账号池；不会迁移默认材料。", "Requires an explicit account pool without parent secrets; default material is not migrated.") : undefined}>
      {t("Qwen 默认目标", "Qwen default target")}</Button>}>
    <div className="nt-stack">
      <QwenEndpointFields value={target} onChange={(value) => { setTarget(value); setPreserveInherited(false); }} disabled={locked || stale || unsupported} t={t} />
      <p>{t("仅修改凭据池的默认目标。现有账号不会随之迁移；需要换目标的账号请单独编辑并替换或清除 Key。", "Only changes the pool default. Existing accounts are not migrated; edit an account separately and replace or clear its Key to change its audience.")}</p>
      {changed && inherited > 0 ? <Checkbox checked={preserveInherited} disabled={locked || stale || unsupported}
        onChange={(event) => setPreserveInherited(event.target.checked)}
        label={t(`固定原目标：将 ${inherited} 个继承账号显式绑定原地域/工作空间，保留其 Key 和目录。`, `Pin the original target for ${inherited} inheriting accounts, keeping their Keys and catalogs.`)} /> : null}
      {stale ? <Notice tone="danger">{t("Qwen 服务商草稿已变化，请关闭并重新打开。", "The Qwen provider draft changed. Close and reopen this editor.")}</Notice> : null}
      {unsupported ? <Notice tone="danger">{t("账号池或父级材料状态已变化；此编辑器不会迁移默认认证材料。", "Pool or parent material state changed; this editor does not migrate default authentication material.")}</Notice> : null}
      {!validBefore ? <Notice tone="danger">{t("原默认目标无效，不能为继承账号固定未知受众。请先修复账号的明确目标和材料。", "The original default is invalid. An unknown audience cannot be pinned for inheriting accounts; repair their explicit targets and material first.")}</Notice> : null}
      <p>{t("应用后沿现有流程自动保存；只有保存成功才生效，失败信息显示在控制台。", "Applying stages the draft for normal autosave. It takes effect only after saving; failures appear in the console.")}</p>
    </div>
    <div className="n-dialog-actions"><Button onClick={() => openChange(false)}>{t("取消", "Cancel")}</Button>
      <Button variant="primary" disabled={locked || unsupported || stale || !validBefore || !after || !changed || (inherited > 0 && !preserveInherited)}
        onClick={() => { if (!locked && !stale && onApply(providerId, snapshot!, target, preserveInherited)) setSnapshot(null); }}>{t("应用到草稿", "Apply to draft")}</Button></div>
  </Dialog>;
}
