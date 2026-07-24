import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent } from "react";

export type RestoreRevisionDialogProps = {
  open: boolean;
  busy: boolean;
  activeRevisionId: string;
  selectedRevisionId: string;
  commitMessage: string;
  secretPatchSummary: {
    keep: number;
    replace: number;
    clear: number;
  };
  aliasChangeLines: string[];
  providerChangeLines: string[];
  modelRouteChangeLines: string[];
  onOpenChange(open: boolean): void;
  onConfirm(): Promise<void>;
};

function renderChangeBlock(title: string, lines: string[], emptyMessage: string) {
  return (
    <div className="nt-validation-list nt-validation-list--warning">
      <strong>{title}</strong>
      <ul>
        {lines.length > 0 ? (
          lines.map((line) => <li key={`${title}:${line}`}>{line}</li>)
        ) : (
          <li>{emptyMessage}</li>
        )}
      </ul>
    </div>
  );
}

export function RestoreRevisionDialog({
  open,
  busy,
  activeRevisionId,
  selectedRevisionId,
  commitMessage,
  secretPatchSummary,
  aliasChangeLines,
  providerChangeLines,
  modelRouteChangeLines,
  onOpenChange,
  onConfirm,
}: RestoreRevisionDialogProps) {
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onConfirm();
      onOpenChange(false);
    } catch {
      // The caller surfaces the server-safe error message in the main console area.
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="restore-revision-description">
          <Dialog.Title>Review revision restore</Dialog.Title>
          <Dialog.Description id="restore-revision-description">
            Confirm that the selected archived revision should replace the current active route
            configuration.
          </Dialog.Description>
          <form onSubmit={(event) => void submit(event)} className="nt-stack">
            <dl className="nt-meta-list">
              <div>
                <dt>Current active revision</dt>
                <dd>{activeRevisionId}</dd>
              </div>
              <div>
                <dt>Revision to restore</dt>
                <dd>{selectedRevisionId}</dd>
              </div>
              <div>
                <dt>Commit message</dt>
                <dd>{commitMessage}</dd>
              </div>
            </dl>

            <div className="nt-validation-list nt-validation-list--warning">
              <strong>Secret patch plan</strong>
              <ul>
                <li>keep: {secretPatchSummary.keep}</li>
                <li>replace: {secretPatchSummary.replace}</li>
                <li>clear: {secretPatchSummary.clear}</li>
              </ul>
            </div>

            <div className="nt-stack">
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>Restore diff review</strong>
                <ul>
                  <li>Review each change block before promoting the archived revision.</li>
                </ul>
              </div>
              <div className="nt-grid nt-grid--2">
                {renderChangeBlock(
                  "Alias changes",
                  aliasChangeLines,
                  "No alias changes detected.",
                )}
                {renderChangeBlock(
                  "Provider changes",
                  providerChangeLines,
                  "No provider changes detected.",
                )}
                {renderChangeBlock(
                  "Model route changes",
                  modelRouteChangeLines,
                  "No model route changes detected.",
                )}
              </div>
            </div>

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button" disabled={busy}>
                  Cancel
                </button>
              </Dialog.Close>
              <button type="submit" disabled={busy}>
                {busy ? "Restoring..." : "Confirm restore"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
