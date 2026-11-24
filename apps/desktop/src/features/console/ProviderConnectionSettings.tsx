import { useState, type FormEvent } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { normalizeProviderTarget, providerConnectionEditable, providerConnectionValue, providerTargetEditable, type ProviderConnectionDraft } from "./providerConnectionDraft";
import "./ProviderConnectionSettings.css";

type Props = {
  providerId: string; document: ConsoleRouteDocument | null; locked: boolean;
  pending: boolean; error: string | null; onRetry(): void;
  onApply(providerId: string, expected: string, value: ProviderConnectionDraft): boolean;
  t(zh: string, en: string): string;
};

/** Inputs stay local until Save; background updates must never overwrite an in-progress edit. */
export function ProviderConnectionSettings({ providerId, document, locked, pending, error, onRetry, onApply, t }: Props) {
  const [draft, setDraft] = useState<(ProviderConnectionDraft & { snapshot: string }) | null>(null);
  const [validation, setValidation] = useState<string | null>(null);
  const [submitted, setSubmitted] = useState(false);
  const provider = document?.providers.find(p => isRecord(p) && p.id === providerId);
  if (!isRecord(provider) || !providerConnectionEditable(provider)) return null;
  const current = providerConnectionValue(provider);
  const snapshot = JSON.stringify(provider);
  const value = draft ?? current;
  const stale = draft !== null && draft.snapshot !== snapshot;
  const changed = value.label !== current.label || value.baseUrl !== current.baseUrl;
  const editableTarget = providerTargetEditable(provider);
  const change = (field: keyof ProviderConnectionDraft, text: string) => {
    setDraft({ ...value, snapshot: draft?.snapshot ?? snapshot, [field]: text });
    setValidation(null); setSubmitted(false);
  };
  const reset = () => { setDraft(null); setValidation(null); setSubmitted(false); };
  const save = (event: FormEvent) => {
    event.preventDefault();
    if (locked || stale || !changed) return;
    const label = value.label.trim();
    if (!label || label.length > 256 || /[\u0000-\u001f\u007f]/.test(label)) {
      setValidation(t("请输入有效的服务商名称（最多 256 个字符）。", "Enter a valid provider name (up to 256 characters).")); return;
    }
    let baseUrl = value.baseUrl;
    try { if (editableTarget) baseUrl = normalizeProviderTarget(baseUrl); }
    catch (cause) { setValidation(cause instanceof Error ? cause.message : String(cause)); return; }
    if (onApply(providerId, draft?.snapshot ?? snapshot, { label, baseUrl })) {
      setDraft(null); setValidation(null); setSubmitted(true);
    }
  };
  return <form className="nt-provider-connection" aria-label={t("服务商信息", "Provider details")} onSubmit={save} noValidate>
    <div className="nt-provider-connection__fields">
      <Field label={t("服务商名称", "Provider name")}><Input compact value={value.label} disabled={locked} maxLength={256}
        onChange={event => change("label", event.target.value)} /></Field>
      {provider.preset !== "cloudflare-workers-ai" && <Field label={t("目标网址", "Target URL")}><Input compact type="url" value={value.baseUrl} readOnly={!editableTarget} disabled={locked}
        placeholder="https://api.example.com/v1" onChange={event => change("baseUrl", event.target.value)} /></Field>}
    </div>
    <div className="nt-provider-connection__actions">
      <span role="status">{submitted ? error ? t("保存失败", "Save failed") : pending ? t("正在保存…", "Saving…") : t("已保存", "Saved")
        : !editableTarget ? t("此接入的目标由专用配置管理", "This integration manages its target separately") : ""}</span>
      {(changed || stale) && <Button compact type="button" disabled={locked} onClick={reset}>{t("取消修改", "Cancel changes")}</Button>}
      {submitted && error && pending ? <Button compact type="button" disabled={locked} onClick={onRetry}>{t("重试保存", "Retry save")}</Button> : null}
      <Button compact type="submit" variant="primary" disabled={locked || !changed || stale}>{t("保存", "Save")}</Button>
    </div>
    {validation || stale || (submitted && error) ? <Notice tone="danger">{validation ?? (stale ? t("配置已变化，请取消修改后重试。", "Configuration changed. Cancel these edits and retry.") : error)}</Notice> : null}
  </form>;
}
