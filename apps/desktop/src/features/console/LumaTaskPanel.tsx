import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { lumaTaskIdSchema, lumaTaskResponseSchema, type LumaTask } from "../../api/console/luma-task";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LumaFilesProps } from "./useLumaFiles";
import { LumaTaskFacts } from "./LumaTaskFacts";

export function LumaTaskPanel(props: LumaFilesProps) {
  const { t } = useUiLocale();
  const [id, setId] = useState("");
  const account = useMemo(() => ({ client: props.client, token: props.managementToken, grant: props.secretGrant,
    providerId: props.providerId, credentialId: props.credentialId, revision: props.expectedRevision, disabled: props.disabled }),
  [props.client, props.managementToken, props.secretGrant, props.providerId, props.credentialId, props.expectedRevision, props.disabled]);
  const scope = useMemo(() => ({ account, id }), [account, id]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [result, setResult] = useState<{ scope: typeof scope; task: LumaTask } | null>(null);
  useEffect(() => { setId(""); }, [account]);
  useEffect(() => {
    setResult(null); setError(false); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const validId = lumaTaskIdSchema.safeParse(id.trim());
  const enabled = !account.disabled && !!account.token && !!account.credentialId && !!account.revision;
  const read = async () => {
    if (!enabled || !validId.success || pending.current) return;
    if (!account.grant) {
      try { await props.onRequestSecretAccess(); } catch { if (current.current === scope) setError(true); }
      return;
    }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false); setResult(null);
    try {
      const response = await account.client.request("/v1/internal/gateway/console/luma-agents-task", lumaTaskResponseSchema, {
        method: "POST", cache: "no-store", managementToken: account.token, secretGrant: account.grant,
        signal: controller.signal, notifyAuthenticationFailure: false,
        body: { providerId: account.providerId, credentialId: account.credentialId, expectedRevision: account.revision, generationId: validId.data },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      if (response.revision !== account.revision || response.providerId !== account.providerId
        || response.credentialId !== account.credentialId || response.result.id !== validId.data) throw new Error("binding");
      setResult({ scope, task: response.result });
    } catch {
      if (!controller.signal.aborted && current.current === scope) setError(true);
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  return <section className="n-stack" aria-label={t("Luma 任务查询", "Luma task lookup")}>
    <Field label={t("待查询任务 ID", "Task ID to look up")}><Input value={id} disabled={!enabled} maxLength={36} onChange={event => setId(event.target.value)} /></Field>
    <div className="nt-actions"><Button disabled={!enabled || !validId.success || busy} onClick={() => void read()}>{t("查询任务与费用", "Read task and charge")}</Button>
      {busy && <Button onClick={() => { pending.current?.abort(); pending.current = null; setBusy(false); }}>{t("停止查询", "Stop lookup")}</Button>}</div>
    {error && <Notice tone="danger">{t("任务查询失败，请检查账号、任务 ID 与配置版本后重试。", "Task lookup failed. Check the account, task ID and configuration before retrying.")}</Notice>}
    {result?.scope === scope && <LumaTaskFacts task={result.task} />}
  </section>;
}
