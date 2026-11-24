// Catalog contents are untrusted display data, never executable tools or inferred model access.
import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Field, Notice, Select, Textarea } from "@neuro/ui";
import { z } from "zod";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { downloadLumaMcpSnapshot, lumaMcpToolSchema } from "./lumaMcpSnapshot";

const responseSchema = z.object({ revision: z.string(), providerId: z.string(), credentialId: z.string(),
  tools: z.array(lumaMcpToolSchema).max(256).refine((tools) => new Set(tools.map((tool) => tool.name)).size === tools.length) });
type Tools = z.infer<typeof responseSchema>["tools"];
type Props = Pick<LumaOAuthProps, "client" | "managementToken" | "secretGrant" | "providerId" | "credentialId"
  | "expectedRevision" | "disabled" | "onRequestSecretAccess">;

export function LumaMcpToolsPanel(props: Props) {
  const { t } = useUiLocale();
  const scope = useMemo(() => ({ client: props.client, token: props.managementToken, grant: props.secretGrant,
    providerId: props.providerId, credentialId: props.credentialId, revision: props.expectedRevision, disabled: props.disabled }),
  [props.client, props.managementToken, props.secretGrant, props.providerId, props.credentialId, props.expectedRevision, props.disabled]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [exportError, setExportError] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; tools: Tools; capturedAt: string } | null>(null);
  const [selected, setSelected] = useState("");
  useEffect(() => {
    setResult(null); setSelected(""); setError(false); setExportError(false); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.token && !!scope.credentialId && !!scope.revision;
  const read = async () => {
    if (!enabled || pending.current) return;
    if (!scope.grant) {
      try { await props.onRequestSecretAccess(); } catch { if (current.current === scope) setError(true); }
      return;
    }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false); setExportError(false); setResult(null); setSelected("");
    try {
      const response = await scope.client.request("/v1/internal/gateway/console/luma-mcp-tools", responseSchema, {
        method: "POST", cache: "no-store", managementToken: scope.token, secretGrant: scope.grant, signal: controller.signal,
        body: { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.revision },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      if (response.revision !== scope.revision || response.providerId !== scope.providerId || response.credentialId !== scope.credentialId) {
        setError(true); return;
      }
      setResult({ scope, tools: response.tools, capturedAt: new Date().toISOString() }); setSelected(response.tools[0]?.name ?? "");
    } catch {
      // Do not echo arbitrary server/upstream content, which may contain authentication material.
      if (!controller.signal.aborted && current.current === scope) setError(true);
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const tools = result?.scope === scope ? result.tools : null;
  const tool = tools?.find((entry) => entry.name === selected);
  const save = () => {
    if (!enabled || busy || !result || result.scope !== scope || current.current !== scope) return;
    setExportError(false);
    try { downloadLumaMcpSnapshot(result.tools, result.capturedAt); }
    catch { setExportError(true); }
  };
  return <section className="n-stack" aria-label={t("Luma MCP 工具目录", "Luma MCP tools")}>
    <Button disabled={!enabled || busy} loading={busy} onClick={() => void read()}>{t("读取工具目录", "Read tool catalog")}</Button>
    <Button disabled={!enabled || busy || !tools} onClick={save}>{t("导出工具目录", "Export tool catalog")}</Button>
    {busy && <Button onClick={() => { pending.current?.abort(); pending.current = null; setBusy(false); }}>{t("停止读取", "Stop reading")}</Button>}
    {error && <Notice tone="danger">{t("工具目录读取失败，请检查授权与配置版本后重试。", "Tool catalog read failed. Check authorization and configuration before retrying.")}</Notice>}
    {exportError && <Notice tone="danger">{t("工具目录导出失败，请重试。", "Tool catalog export failed. Please retry.")}</Notice>}
    {tools?.length === 0 && <Notice>{t("工具目录为空", "Tool catalog is empty")}</Notice>}
    {!!tools?.length && <Field label={t("工具", "Tool")}><Select value={selected} onChange={(event) => setSelected(event.target.value)}>
      {tools.map((entry) => <option key={entry.name} value={entry.name}>{entry.name}</option>)}
    </Select></Field>}
    {tool && <>
      {tool.title && <Field label={t("名称", "Title")}><Textarea readOnly rows={2} value={tool.title} /></Field>}
      {tool.description && <Field label={t("说明", "Description")}><Textarea readOnly rows={4} value={tool.description} /></Field>}
      <Field label="Input schema"><Textarea readOnly rows={6} value={JSON.stringify(tool.inputSchema, null, 2)} /></Field>
      {tool.outputSchema && <Field label="Output schema"><Textarea readOnly rows={6} value={JSON.stringify(tool.outputSchema, null, 2)} /></Field>}
    </>}
  </section>;
}
