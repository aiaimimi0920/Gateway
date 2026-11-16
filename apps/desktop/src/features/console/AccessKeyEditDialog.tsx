import { Dialog } from "@neuro/ui";
import { useRef, useState } from "react";
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
  const nameInput = useRef<HTMLInputElement>(null);
  return (
    <Dialog
      title={t("编辑 API Key", "Edit API Key")}
      className="nt-key-dialog nt-key-dialog--structured"
      closeLabel={t("关闭", "Close")}
      busy={busy}
      initialFocusRef={nameInput}
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
          <AccessKeyCreateForm
            nameInputRef={nameInput}
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
    </Dialog>
  );
}
