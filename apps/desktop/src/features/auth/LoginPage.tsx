import { type FormEvent, useState } from "react";

export type LoginPageProps = {
  busy: boolean;
  error?: string | null;
  onSubmit(token: string): Promise<void>;
};

export function LoginPage({ busy, error, onSubmit }: LoginPageProps) {
  const [token, setToken] = useState("");

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onSubmit(token);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <main className="auth-page" aria-labelledby="login-title">
      <form className="auth-panel" onSubmit={(event) => void submit(event)}>
        <h1 id="login-title">Sign in</h1>
        <p>Use the management token configured for this Gateway.</p>
        <label htmlFor="login-token">Management token</label>
        <input
          id="login-token"
          name="management-token"
          type="password"
          autoComplete="current-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
          autoFocus
        />
        {error && <p role="alert">{error}</p>}
        <button type="submit" disabled={busy}>
          {busy ? "Signing in..." : "Sign in"}
        </button>
      </form>
    </main>
  );
}
