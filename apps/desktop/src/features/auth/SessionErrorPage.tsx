export type SessionErrorPageProps = {
  busy: boolean;
  error?: string | null;
  onRetry(): Promise<void>;
};

export function SessionErrorPage({ busy, error, onRetry }: SessionErrorPageProps) {
  return (
    <main className="auth-page" aria-labelledby="session-error-title">
      <section className="auth-panel">
        <h1 id="session-error-title">Session unavailable</h1>
        <p role="alert">{error ?? "Gateway administrator status could not be verified."}</p>
        <button type="button" disabled={busy} onClick={() => void onRetry()}>
          {busy ? "Retrying..." : "Retry initialization"}
        </button>
      </section>
    </main>
  );
}
