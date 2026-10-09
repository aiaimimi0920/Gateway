import * as Dialog from "@radix-ui/react-dialog";
import { useRef, useState } from "react";
import { X } from "lucide-react";
import type {
  ConsoleAccessKey,
  ConsoleAccessCatalog,
} from "../../api/contracts";
import { AccessKeyCreateForm } from "./AccessKeyCreateForm";
import { editAccessKeyDraft } from "./accessKeyEditing";
import type { AccessKeyDraft, TranslateFn } from "./accessKeysTypes";

export function AccessKeyEditDialog({
  accessKey,
  catalog,
  busy,
  error,
  onClose,
  onSave,
  t,
}: {
  accessKey: ConsoleAccessKey;
  catalog: ConsoleAccessCatalog;
  busy: boolean;
  error: string | null;
  onClose(): void;
  onSave(key: ConsoleAccessKey, draft: AccessKeyDraft): Promise<boolean>;
  t: TranslateFn;
}) {
  const [draft, setDraft] = useState(() =>
    editAccessKeyDraft(accessKey, catalog),
  );
  // Capture before the form's autoFocus moves focus into this portal.
  const trigger = useRef<HTMLElement | null>(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-key-dialog"
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            if (trigger.current?.isConnected) trigger.current.focus();
          }}
          onEscapeKeyDown={(event) => {
            if (busy) event.preventDefault();
          }}
          onPointerDownOutside={(event) => {
            if (busy) event.preventDefault();
          }}
        >
          <div className="nt-key-dialog__head">
            <Dialog.Title>{t("编辑 API Key", "Edit API Key")}</Dialog.Title>
            <button
              type="button"
              className="nt-icon-action"
              aria-label={t("关闭", "Close")}
              disabled={busy}
              onClick={onClose}
            >
              <X size={17} aria-hidden="true" />
            </button>
          </div>
          <Dialog.Description className="nt-visually-hidden">
            {accessKey.displayName}
          </Dialog.Description>
          <AccessKeyCreateForm
            editing
            draft={draft}
            onChange={setDraft}
            busy={busy}
            locked={false}
            local={catalog.storageMode === "local"}
            cashQuotaSupported={catalog.cashQuotaSupported}
            bundles={catalog.bundles}
            groups={catalog.accountGroups}
            balance={catalog.balances.find(
              (balance) => balance.accessKeyId === accessKey.id,
            )}
            error={error}
            t={t}
            onCancel={onClose}
            onSubmit={() => {
              void onSave(accessKey, draft).then((saved) => {
                if (saved) onClose();
              });
            }}
          />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
