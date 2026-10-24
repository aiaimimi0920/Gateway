import type { RefObject } from "react";
import { CheckCircle2 } from "lucide-react";
import { CopyButton } from "./AccessWorkspacePrimitives";
import type { AccessRevealedSecret, TranslateFn } from "./accessKeysTypes";

/** Result stays in the dialog; plaintext is never copied into the list or persistent UI state. */
export function AccessKeySecretResult({ secret, apiBaseUrl, inputRef, onClose, t }: {
  secret: AccessRevealedSecret;
  apiBaseUrl: string;
  inputRef: RefObject<HTMLTextAreaElement | null>;
  onClose(): void;
  t: TranslateFn;
}) {
  return <>
    <div className="nt-key-dialog__body nt-key-secret-result">
      <div className="nt-key-result-status"><CheckCircle2 size={18} aria-hidden="true" /><strong>{secret.label}</strong></div>
      <p className="nt-key-form__hint">{t("将以下两项填入客户端。请勿公开或分享完整密钥。", "Use these two values in your client. Keep the full key private.")}</p>
      <section className="nt-key-result-field">
        <div className="nt-key-result-field__head"><span>API Key</span>
          <CopyButton t={t} value={secret.secret} label={t("复制 API Key", "Copy API Key")} variant="outline" />
        </div>
        <textarea ref={inputRef} className="nt-input nt-key-secret-value" aria-label="API Key"
          readOnly value={secret.secret} rows={3} spellCheck={false} />
      </section>
      <section className="nt-key-result-field">
        <div className="nt-key-result-field__head"><span>API Base URL</span>
          <CopyButton t={t} value={apiBaseUrl} label={t("复制 API 地址", "Copy API URL")} variant="outline" />
        </div>
        <input className="nt-input" aria-label="API Base URL" readOnly value={apiBaseUrl} />
      </section>
    </div>
    <footer className="nt-key-dialog__footer"><div className="dialog-actions">
      <button type="button" className="nt-btn nt-btn--primary" onClick={onClose}>{t("完成", "Done")}</button>
    </div></footer>
  </>;
}
