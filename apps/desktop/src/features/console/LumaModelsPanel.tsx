import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Field, Notice, Select, Textarea } from "@neuro/ui";
import { lumaModelsSchema, lumaModelDetailsSchema, type LumaModel, type LumaModelDetails } from "../../api/console/luma-models";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LumaOAuthProps } from "./useLumaOAuth";

type Props = Pick<LumaOAuthProps, "client" | "managementToken" | "secretGrant" | "providerId" | "credentialId"
  | "expectedRevision" | "disabled" | "onRequestSecretAccess">;
export function LumaModelsPanel(props: Props) {
  const { t } = useUiLocale();
  const scope = useMemo(() => ({ client: props.client, token: props.managementToken, grant: props.secretGrant,
    providerId: props.providerId, credentialId: props.credentialId, revision: props.expectedRevision, disabled: props.disabled }),
  [props.client, props.managementToken, props.secretGrant, props.providerId, props.credentialId, props.expectedRevision, props.disabled]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; models: LumaModel[] } | null>(null);
  const [selected, setSelected] = useState("");
  const [detail, setDetail] = useState<{ scope: typeof scope; data: LumaModelDetails } | null>(null);
  useEffect(() => {
    setResult(null); setDetail(null); setSelected(""); setError(false); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.token && !!scope.credentialId && !!scope.revision;
  const models = result?.scope === scope ? result.models : null;
  const model = models?.find((row) => row.id === selected);
  const details = detail?.scope === scope && detail.data.model === selected ? detail.data : null;
  const read = async (action: "list" | "describe") => {
    if (!enabled || pending.current) return;
    if (action === "describe" && (!model || !["image", "video"].includes(model.modality))) return;
    if (!scope.grant) {
      try { await props.onRequestSecretAccess(); } catch { if (current.current === scope) setError(true); }
      return;
    }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false); setDetail(null);
    if (action === "list") { setResult(null); setSelected(""); }
    const binding = { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.revision };
    const options = { method: "POST" as const, cache: "no-store" as const, managementToken: scope.token,
      secretGrant: scope.grant, signal: controller.signal };
    try {
      if (action === "list") {
        const response = await scope.client.request("/v1/internal/gateway/console/luma-mcp-models", lumaModelsSchema,
          { ...options, body: { ...binding, action } });
        if (controller.signal.aborted || current.current !== scope) return;
        if (response.revision !== scope.revision || response.providerId !== scope.providerId || response.credentialId !== scope.credentialId) throw new Error("binding");
        setResult({ scope, models: response.result.models }); setSelected(response.result.models[0]?.id ?? "");
      } else {
        const response = await scope.client.request("/v1/internal/gateway/console/luma-mcp-models", lumaModelDetailsSchema,
          { ...options, body: { ...binding, action, model: model!.id, type: model!.modality } });
        if (controller.signal.aborted || current.current !== scope) return;
        if (response.revision !== scope.revision || response.providerId !== scope.providerId || response.credentialId !== scope.credentialId
          || response.result.model !== model!.id || response.result.type !== model!.modality) throw new Error("binding");
        setDetail({ scope, data: response.result });
      }
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(true);
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  return <section className="n-stack" aria-label={t("Luma 模型", "Luma models")}>
    <Button disabled={!enabled || busy} onClick={() => void read("list")}>{t("读取模型", "Read models")}</Button>
    {busy && <Button onClick={() => { pending.current?.abort(); pending.current = null; setBusy(false); }}>{t("停止读取模型", "Stop reading models")}</Button>}
    {error && <Notice tone="danger">{t("模型读取失败，请检查授权与配置版本。", "Model read failed. Check authorization and configuration.")}</Notice>}
    {models?.length === 0 && <Notice>{t("模型列表为空", "Model list is empty")}</Notice>}
    {!!models?.length && <Field label={t("可选模型", "Available models")}><Select value={selected} disabled={busy}
      onChange={(event) => { setSelected(event.target.value); setDetail(null); }}>
      {models.map((row) => <option key={row.id} value={row.id}>{row.id} · {row.modality}</option>)}
    </Select></Field>}
    {model && <>
      <Field label={t("公布费率", "Published rates")}><Textarea readOnly rows={3}
        value={model.pricing === null ? t("未知", "Unknown") : JSON.stringify(model.pricing, null, 2)} /></Field>
      <Button disabled={busy || !["image", "video"].includes(model.modality)} onClick={() => void read("describe")}>{t("读取模型参数", "Read model options")}</Button>
    </>}
    {details && <Field label={t("模型参数", "Model options")}><Textarea readOnly rows={8} value={JSON.stringify(details.parameters, null, 2)} /></Field>}
  </section>;
}
