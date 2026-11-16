import { ConfirmDialog, Dialog, Notice } from "@neuro/ui";
import { useEffect, useRef, type RefObject } from "react";
import type { ConsoleAccessKey } from "../../api/contracts";
import { AccessKeyCreateForm } from "./AccessKeyCreateForm";
import { AccessKeySecretResult } from "./AccessKeySecretResult";
import "./accessKeyDialogs.css";
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
  returnFocusRef: RefObject<HTMLElement | null>;
  apiBaseUrl: string;
};

export function AccessKeyDialog(props: KeyDialogProps) {
  const { t, open, onClose, revealedSecret, creatingKey, catalog, apiBaseUrl } =
    props;
  const secret = revealedSecret?.kind === "access-key" ? revealedSecret : null;
  const secretInput = useRef<HTMLTextAreaElement>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (secret) secretInput.current?.focus();
  }, [secret]);
  return (
    <Dialog
      title={secret ? t("API Key 已生成", "API Key generated") : t("新建密钥", "New Key")}
      className="nt-key-dialog nt-key-dialog--structured"
      closeLabel={t("关闭", "Close")}
      busy={creatingKey}
      initialFocusRef={secret ? secretInput : nameInput}
      returnFocusRef={props.returnFocusRef}
      open={open || Boolean(secret)}
      onOpenChange={(next) => {
        if (!next && !creatingKey) onClose();
      }}
    >
          {secret ? (
            <AccessKeySecretResult secret={secret} apiBaseUrl={apiBaseUrl} inputRef={secretInput} onClose={onClose} t={t} />
          ) : (
            <AccessKeyCreateForm
              nameInputRef={nameInput}
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
              onCancel={onClose}
              t={t}
            />
          )}
    </Dialog>
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
  const rotate = pending?.kind === "rotate";
  const deleting = pending?.kind === "delete";
  const enabling = pending?.kind === "enable";
  const action = rotate
    ? t("轮转", "Rotate")
    : deleting
      ? t("删除", "Delete")
      : enabling
        ? t("启用", "Enable")
        : t("停用", "Disable");
  return (
    <ConfirmDialog
      title={`${action} API Key`}
      busy={busy}
      danger={deleting}
      confirmLabel={t(`确认${action}`, `${action} key`)}
      cancelLabel={t("取消", "Cancel")}
      onConfirm={onConfirm}
      dismissOnOutside
      focusResetKey={pending ? `${pending.kind}:${pending.key.id}` : ""}
      description={rotate
        ? t(`轮转“${pending?.key.displayName ?? ""}”？旧密钥将立即失效，请同步更新客户端。`, `Rotate “${pending?.key.displayName ?? ""}”? The old key will stop working immediately.`)
        : deleting
          ? t(`删除“${pending?.key.displayName ?? ""}”？密钥将失效并从列表移除，无法恢复；历史现金账单保留。`, `Delete “${pending?.key.displayName ?? ""}”? The key will stop working and be removed permanently. Historical cash bills are retained.`)
          : enabling
            ? t(`启用“${pending?.key.displayName ?? ""}”？恢复使用原密钥，有效期和额度保持不变。`, `Enable “${pending?.key.displayName ?? ""}”? The same key can be used again; expiry and quota remain unchanged.`)
            : t(`停用“${pending?.key.displayName ?? ""}”？新请求将被拒绝，之后可重新启用。`, `Disable “${pending?.key.displayName ?? ""}”? New requests will be rejected; you can enable it again later.`)}
      open={Boolean(pending)}
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
          <div className="nt-key-confirm-target"><strong>{pending?.key.displayName}</strong>
            <code>{pending?.key.publicKeyPrefix}••••••</code>
          </div>
          {error ? (
            <Notice tone="danger">
              {error}
            </Notice>
          ) : null}
    </ConfirmDialog>
  );
}
