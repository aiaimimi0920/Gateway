import { useId } from "react";
import { Button, CopyButton, Input, Notice, SegmentedControl } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import "./BackendConnectionForm.css";
import "./AppearanceSettings.css";

export type BackendConnectionFormProps = {
  current: string; localAddress: string; mode: "local" | "existing"; remote: string;
  busy: "" | "test" | "apply"; message: string; error: boolean; native?: boolean;
  onMode(mode: "local" | "existing"): void; onRemote(value: string): void;
  onLocalPort(value: number): void;
  onTest(): void; onApply(): void;
};
export function BackendConnectionForm(props: BackendConnectionFormProps) {
  const { t } = useUiLocale();
  const id = useId();
  const target = props.mode === "local" ? props.localAddress : props.remote;
  const origin = (address: string) => { try { return new URL(address).origin; } catch { return ""; } };
  const changed = !props.current || origin(target) !== origin(props.current);
  const localPort = props.localAddress.match(/^http:\/\/127\.0\.0\.1:(\d+)\/?$/)?.[1] ?? "";
  const invalid = props.mode === "local" ? !/^\d+$/.test(localPort) || Number(localPort) < 1 || Number(localPort) > 65535 : !origin(props.remote);
  return <>
    <div className="nt-backend-summary">
      <div className="nt-setting-row"><span className="nt-setting-label">{t("当前地址", "Current address")}</span>
        <div className="nt-backend-current"><code title={props.current}>{props.current || "—"}</code>
          <CopyButton key={props.current} compact className="nt-backend-copy" disabled={!props.current} label={t("复制", "Copy")}
            copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")}
            onCopy={() => navigator.clipboard.writeText(props.current)} />
        </div>
      </div>
    </div>
    <div className="nt-settings-fields">
      <div className="nt-setting-row"><span className="nt-setting-label">{t("网关地址", "Gateway address")}</span>
        <SegmentedControl<"local" | "existing"> hideLabel name={`${id}-mode`} label={t("连接方式", "Mode")} value={props.mode} disabled={!!props.busy} onValueChange={props.onMode}
          options={[{ value: "local", label: t("本机", "Local") }, { value: "existing", label: t("远程", "Remote") }]} />
      </div>
      <div className="nt-setting-row nt-backend-address-row"><label htmlFor={`${id}-address`}>{t("后端地址", "Address")}</label>
        <div className="nt-backend-editor">
          {props.mode === "local" ? <div className="nt-backend-local-address"><code>http://127.0.0.1 :</code>
            <Input compact aria-label={t("本机端口", "Local port")} value={localPort === "0" ? "" : localPort} inputMode="numeric" maxLength={5} disabled={!!props.busy}
              aria-invalid={invalid || undefined} onChange={event => { if (/^\d*$/.test(event.target.value)) props.onLocalPort(Number(event.target.value)); }} /></div> :
          <Input compact id={`${id}-address`} type="url" value={props.remote}
            disabled={!!props.busy} maxLength={2048} autoComplete="off" spellCheck={false}
            placeholder="https://gateway.example.com" aria-describedby={`${id}-feedback`} aria-invalid={props.error || undefined}
            onChange={event => props.onRemote(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && changed && !invalid && !props.busy) { event.preventDefault(); props.onApply(); } }} />}
          <div className="nt-backend-actions">
            <Button compact disabled={!!props.busy || invalid} loading={props.busy === "test"} onClick={props.onTest}>{t("测试连接", "Test")}</Button>
            <Button compact variant="primary" disabled={!!props.busy || !changed || invalid} loading={props.busy === "apply"} onClick={props.onApply}>{t("应用", "Apply")}</Button>
          </div>
        </div>
      </div>
    </div>
    {props.mode === "local" && changed && props.current && <Notice>{t("更换本机端口会重启本客户端管理的网关；不会停止外部实例。", "Changing the local port restarts only this client's managed Gateway, never external instances.")}</Notice>}
    <div className="nt-settings-feedback-slot" id={`${id}-feedback`}>{props.message && <Notice tone={props.error ? "danger" : "info"}>{props.message}</Notice>}</div>
  </>;
}
