import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ConsoleAccessKey } from "../../api/contracts";
import { AccessKeyCreateForm } from "./AccessKeyCreateForm";
import { CopyButton } from "./AccessWorkspacePrimitives";
import type { AccessKeysWorkspaceProps, TranslateFn } from "./accessKeysTypes";

type KeyDialogProps = Pick<
  AccessKeysWorkspaceProps,
  | "t"
  | "catalog"
  | "keyDraft"
  | "onKeyDraftChange"
  | "creatingKey"
  | "editorLocked"
  | "revealedSecret"
  | "onCreateKey"
> & {
  open: boolean;
  onClose(): void;
  onRestoreFocus(): void;
  apiBaseUrl: string;
};

export function AccessKeyDialog(props: KeyDialogProps) {
  const { t, open, onClose, revealedSecret, creatingKey, catalog, apiBaseUrl } =
    props;
  const secret = revealedSecret?.kind === "access-key" ? revealedSecret : null;
  const secretInput = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (secret) secretInput.current?.focus();
  }, [secret]);
  return (
    <Dialog.Root
      open={open || Boolean(secret)}
      onOpenChange={(next) => {
        if (!next && !creatingKey) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-key-dialog"
          onOpenAutoFocus={(event) => {
            if (secret) {
              event.preventDefault();
              secretInput.current?.focus();
            }
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            props.onRestoreFocus();
          }}
          onPointerDownOutside={(event) => {
            if (creatingKey) event.preventDefault();
          }}
          onEscapeKeyDown={(event) => {
            if (creatingKey) event.preventDefault();
          }}
        >
          <div className="nt-key-dialog__head">
            <Dialog.Title>
              {secret
                ? t("API Key 已生成", "API Key generated")
                : t("新建 API Key", "New API Key")}
            </Dialog.Title>
            {!secret ? (
              <button
                type="button"
                className="nt-icon-action"
                aria-label={t("关闭", "Close")}
                disabled={creatingKey}
                onClick={onClose}
              >
                <X size={17} aria-hidden="true" />
              </button>
            ) : null}
          </div>
          <Dialog.Description className="nt-visually-hidden">
            {secret
              ? t("API Key", "API Key")
              : t(
                  "创建用于访问网关的 API Key。",
                  "Create an API Key for gateway access.",
                )}
          </Dialog.Description>
          {secret ? (
            <div className="nt-key-secret-result">
              <strong>{secret.label}</strong>
              <label className="nt-field">
                <span>API Key</span>
                <textarea
                  ref={secretInput}
                  className="nt-input nt-key-secret-value"
                  readOnly
                  value={secret.secret}
                  rows={3}
                  spellCheck={false}
                />
              </label>
              <CopyButton
                t={t}
                value={secret.secret}
                label={t("复制 API Key", "Copy API Key")}
              />
              <label className="nt-field">
                <span>API Base URL</span>
                <input className="nt-input" readOnly value={apiBaseUrl} />
              </label>
              <CopyButton
                t={t}
                value={apiBaseUrl}
                label={t("复制 API 地址", "Copy API URL")}
              />
              <div className="dialog-actions">
                <button
                  type="button"
                  className="nt-btn nt-btn--primary"
                  onClick={onClose}
                >
                  {t("完成", "Done")}
                </button>
              </div>
            </div>
          ) : (
            <AccessKeyCreateForm
              draft={props.keyDraft}
              onChange={props.onKeyDraftChange}
              onSubmit={() => {
                void props.onCreateKey();
              }}
              busy={creatingKey}
              locked={props.editorLocked}
              local={catalog.data?.storageMode === "local"}
              cashQuotaSupported={catalog.data?.cashQuotaSupported}
              bundles={catalog.data?.bundles ?? []}
              groups={catalog.data?.accountGroups}
              error={catalog.error}
              t={t}
            />
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export type PendingKeyAction = {
  kind: "rotate" | "enable" | "disable" | "delete";
  key: ConsoleAccessKey;
};

export function AccessKeyConfirmDialog({
  pending,
  busy,
  error,
  onClose,
  onConfirm,
  t,
}: {
  pending: PendingKeyAction | null;
  busy: boolean;
  error: string | null;
  onClose(): void;
  onConfirm(): void;
  t: TranslateFn;
}) {
  const returnFocus = useRef<HTMLElement | null>(null);
  const rotate = pending?.kind === "rotate";
  const deleting = pending?.kind === "delete";
  const enabling = pending?.kind === "enable";
  const action = rotate
    ? t("轮换", "Rotate")
    : deleting
      ? t("删除", "Delete")
      : enabling
        ? t("启用", "Enable")
        : t("停用", "Disable");
  return (
    <Dialog.Root
      open={Boolean(pending)}
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-key-dialog"
          onOpenAutoFocus={() => {
            returnFocus.current =
              document.activeElement instanceof HTMLElement
                ? document.activeElement
                : null;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            if (returnFocus.current?.isConnected) returnFocus.current.focus();
          }}
          onPointerDownOutside={(event) => {
            if (busy) event.preventDefault();
          }}
          onEscapeKeyDown={(event) => {
            if (busy) event.preventDefault();
          }}
        >
          <Dialog.Title>{action} API Key</Dialog.Title>
          <Dialog.Description>
            {rotate
              ? t(
                  `轮换“${pending?.key.displayName ?? ""}”？旧密钥将立即失效。`,
                  `Rotate “${pending?.key.displayName ?? ""}”? The old key will stop working immediately.`,
                )
              : deleting
                ? t(
                    `删除“${pending?.key.displayName ?? ""}”？密钥将失效并从列表移除，无法恢复；历史现金账单保留。`,
                    `Delete “${pending?.key.displayName ?? ""}”? The key will stop working and be removed permanently. Historical cash bills are retained.`,
                  )
                : enabling
                  ? t(
                      `启用“${pending?.key.displayName ?? ""}”？恢复使用原密钥，有效期和额度保持不变。`,
                      `Enable “${pending?.key.displayName ?? ""}”? The same key can be used again; expiry and quota remain unchanged.`,
                    )
                  : t(
                      `停用“${pending?.key.displayName ?? ""}”？新请求将被拒绝，之后可重新启用。`,
                      `Disable “${pending?.key.displayName ?? ""}”? New requests will be rejected; you can enable it again later.`,
                    )}
          </Dialog.Description>
          {error ? (
            <div role="alert" className="nt-alert nt-alert--danger">
              {error}
            </div>
          ) : null}
          <div className="dialog-actions">
            <button
              type="button"
              className="nt-btn nt-btn--outline"
              disabled={busy}
              onClick={onClose}
            >
              {t("取消", "Cancel")}
            </button>
            <button
              type="button"
              className={`nt-btn nt-btn--${deleting ? "danger" : "primary"}`}
              disabled={busy}
              onClick={onConfirm}
            >
              {busy
                ? t("处理中…", "Working…")
                : t(`确认${action}`, `${action} key`)}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
