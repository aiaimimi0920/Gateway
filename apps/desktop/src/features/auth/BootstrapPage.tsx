import { type FormEvent, useState } from "react";

export type BootstrapPageProps = {
  busy: boolean;
  error?: string | null;
  onSubmit(token: string): Promise<void>;
};

export function BootstrapPage({ busy, error, onSubmit }: BootstrapPageProps) {
  const [token, setToken] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (token.trim() !== confirmation.trim()) {
      setLocalError("The management token confirmation does not match.");
      return;
    }
    setLocalError(null);
    try {
      await onSubmit(token);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <main className="auth-page" aria-labelledby="bootstrap-title">
      <form className="auth-panel" onSubmit={(event) => void submit(event)}>
        <h1 id="bootstrap-title">Set up administrator</h1>
        <p>Create the management token for this local Gateway instance.</p>
        <label htmlFor="bootstrap-token">Management token</label>
        <input
          id="bootstrap-token"
          name="management-token"
          type="password"
          autoComplete="new-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
        />
        <label htmlFor="bootstrap-confirmation">Confirm management token</label>
        <input
          id="bootstrap-confirmation"
          name="management-token-confirmation"
          type="password"
          autoComplete="new-password"
          value={confirmation}
          onChange={(event) => setConfirmation(event.currentTarget.value)}
          required
        />
        {(localError || error) && <p role="alert">{localError ?? error}</p>}
        <button type="submit" disabled={busy}>
          {busy ? "Creating administrator..." : "Create administrator"}
        </button>
      </form>
    </main>
  );
}
