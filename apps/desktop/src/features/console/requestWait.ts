// Provider settings only; never merge these values into upstream request parameters.
export type RecoveryWaitChoice = "auto" | "disabled" | "300" | "600" | "900" | "client_connected";
export function writeRecoveryWait(provider: Record<string, unknown>, choice: RecoveryWaitChoice = "auto") {
  if (choice === "auto") { delete provider.request_wait; return; }
  provider.request_wait = choice === "client_connected" || choice === "disabled"
    ? { mode: choice } : { mode: "bounded", total_timeout_seconds: Number(choice) };
}
