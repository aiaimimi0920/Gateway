import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent, useEffect, useState } from "react";

export type SecretConfirmDialogProps = {
  open: boolean;
  busy: boolean;
  error?: string | null;
  onOpenChange(open: boolean): void;
  onConfirm(token: string): Promise<void>;
};

export function SecretConfirmDialog({
  open,
  busy,
  error,
  onOpenChange,
  onConfirm,
}: SecretConfirmDialogProps) {
  const [token, setToken] = useState("");

  useEffect(() => {
    if (!open) {
      setToken("");
    }
  }, [open]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onConfirm(token);
      onOpenChange(false);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="secret-confirm-description">
          <Dialog.Title>Confirm secret access</Dialog.Title>
          <Dialog.Description id="secret-confirm-description">
            Re-enter the management token to receive a short-lived secret grant.
          </Dialog.Description>
          <form onSubmit={(event) => void submit(event)}>
            <label htmlFor="secret-confirm-token">Management token</label>
            <input
              id="secret-confirm-token"
              type="password"
              autoComplete="current-password"
              value={token}
              onChange={(event) => setToken(event.currentTarget.value)}
              required
              autoFocus
            />
            {error && <p role="alert">{error}</p>}
            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button" disabled={busy}>Cancel</button>
              </Dialog.Close>
              <button type="submit" disabled={busy}>
                {busy ? "Confirming..." : "Confirm"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
