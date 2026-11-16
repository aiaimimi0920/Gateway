import { Button, Dialog, Field, Notice, Select } from "@neuro/ui";
import { useId, useRef } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { CredentialAccountFields } from "./CredentialAccountFields";
import { CredentialProtocolList } from "./CredentialProtocolList";
import type { CredentialDialogProps } from "./credentialDialogTypes";
import { useCredentialDialog } from "./useCredentialDialog";
import "./CredentialDialog.css";

export type {
  CredentialDialogMode, CredentialApiKeyOperation, CredentialDialogValue,
  CredentialDialogProviderOption, CredentialDialogProps
} from "./credentialDialogTypes";

export function CredentialDialog(props: CredentialDialogProps) {
  const { t } = useUiLocale();
  const model = useCredentialDialog(props);
  const formId = useId();
  const initialFocus = useRef<HTMLElement | null>(null);
  const { open, mode, locked, hasSecretAccess, onOpenChange, onRequestSecretAccess, supportsBasispoints,
    discoveryProvider, onDiscover } = props;
  const { value, setValue, authenticationMode, setAuthenticationMode, authenticationActive, setValidationError,
    oauth, manual, authentication, reauthentication, reauthorizing, setReauthorizing, localEdits,
    busy, secretMutationRequested, productSession, bedrock, qwen, discovery, catalog, official,
    submit, refreshProtocols, validationError } = model;
  const error = validationError ?? discovery.error ?? catalog.error ?? official.error;

  return <Dialog open={open} onOpenChange={onOpenChange} className="nt-credential-dialog"
    title={mode === "add" ? t("新增账号", "Add account") : t("编辑账号", "Edit account")}
    closeLabel={t("关闭", "Close")} initialFocusRef={initialFocus}>
    <div className="nt-credential-scroll nt-credential-editor" ref={(node) => {
      // OAuth is supplied by the application; keep its first available action as the initial target.
      initialFocus.current = node?.querySelector<HTMLElement>("button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href]") ?? null;
    }}>
      {oauth && (supportsBasispoints?.(value.providerId) || manual) && <Field label={t("录入方式", "Import method")}>
        <Select compact value={authenticationMode} disabled={locked || authenticationActive} onChange={(event) => {
          if (authenticationActive) return;
          setAuthenticationMode(event.target.value); setValidationError(null);
          setValue((current) => ({ ...current, apiKeyValue: "", chatgptAccountId: "", apiKeyOperation: "replace" }));
        }}>
          <option value="oauth">{manual?.oauthLabel ?? t("ChatGPT 登录", "ChatGPT sign-in")}</option>
          <option value="token">{manual?.manualLabel ?? t("现有 Access Token", "Existing access token")}</option>
        </Select>
      </Field>}
      {authentication}
      {!authentication && manual?.note && <p>{manual.note}</p>}
      {reauthentication && <div className="nt-stack">
        <Button compact disabled={locked || busy || authenticationActive || !reauthorizing && localEdits}
          onClick={() => { if (!authenticationActive) setReauthorizing((current) => !current); }}>
          {reauthorizing ? t("返回账号编辑", "Back to account editing") : t("重新授权此账号", "Reauthorize this account")}
        </Button>
        {!reauthorizing && localEdits && <Notice tone="warning">{t("请先保存或放弃当前表单修改，再重新授权。", "Save or discard local form changes before reauthorization.")}</Notice>}
      </div>}
      <form id={formId} className="nt-stack" onSubmit={submit}>
        <CredentialAccountFields {...props} model={model} />
        {!authentication && (secretMutationRequested || productSession || bedrock) && !hasSecretAccess && <Notice tone="warning">
          <strong>{t("需要敏感信息访问权限", "Secret access is required")}</strong>
          <Button compact onClick={onRequestSecretAccess}>{t("确认敏感信息访问权限", "Confirm secret access")}</Button>
        </Notice>}
        {!authentication && !bedrock && !qwen && discoveryProvider?.(value.providerId) && onDiscover && <div className="nt-credential-protocols">
          <div className="nt-credential-protocol-status" tabIndex={value.discoveryStatus === "pending" || value.discoveryStatus === "unconfirmed" ? 0 : undefined}
            aria-label={t("协议识别状态", "Protocol discovery status")}>
            {value.discoveryStatus === "pending" && <Notice>{t("后台识别中", "Discovering in background")}</Notice>}
            {value.discoveryStatus === "unconfirmed" && <Notice tone="warning">{t("尚未确认可用协议，可重新刷新", "Protocols unconfirmed; refresh to retry")}</Notice>}
          </div>
          <Button compact disabled={locked || discovery.busy} loading={discovery.busy} onClick={refreshProtocols}>
            {t("刷新协议", "Refresh protocols")}
          </Button>
          <CredentialProtocolList discovery={value.discovery} />
        </div>}
        {error && <Notice tone="danger">{error}</Notice>}
      </form>
      {open && mode === "edit" && !authentication && props.initialValue && props.renderStoredAccountTools?.(
        props.initialValue.providerId, props.initialValue.credentialId, locked || busy || localEdits, (update) => {
          if (locked || busy || localEdits) return false;
          try { setValue(update(value)); setValidationError(null); return true; }
          catch { setValidationError(t("目录无法应用到当前账号。", "The catalog cannot be applied to this account.")); return false; }
        })}
    </div>
    <div className="n-dialog-actions">
      <Button onClick={() => onOpenChange(false)}>{t("取消", "Cancel")}</Button>
      {!authentication && <Button variant="primary" type="submit" form={formId} disabled={locked || busy} loading={busy}>
        {t("保存", "Save")}
      </Button>}
    </div>
  </Dialog>;
}
