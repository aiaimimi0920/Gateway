import type { ConsoleOperatorSummary } from "../../api/contracts";
import type { OperationsPanelState } from "./operations-contracts";

export type DatabaseOperationsAvailability =
  "available" | "unsupported" | "unknown";

// The persisted-analysis handlers require pg_pool, not merely a ready database.
// SQLite can be healthy while these server-only endpoints are unsupported.
export function databaseOperationsAvailability(
  summary: OperationsPanelState<ConsoleOperatorSummary>,
): DatabaseOperationsAvailability {
  if (summary.error || !summary.data) return "unknown";
  return summary.data.readiness.dependencies.postgresql.configured
    ? "available"
    : "unsupported";
}

export async function settleOperations<T>(
  run: (() => Promise<T>) | null,
): Promise<OperationsPanelState<T>> {
  if (!run) return { data: null, error: null, loading: false };
  try {
    return { data: await run(), error: null, loading: false };
  } catch (cause) {
    return {
      data: null,
      error: cause instanceof Error ? cause.message : String(cause),
      loading: false,
    };
  }
}

export async function settleDatabaseOperations<T>(
  summary: Promise<OperationsPanelState<ConsoleOperatorSummary>>,
  run: (() => Promise<T>) | null,
): Promise<OperationsPanelState<T>> {
  const availability = databaseOperationsAvailability(await summary);
  // Unsupported and unknown do not become fabricated successful empty results.
  return settleOperations(availability === "available" ? run : null);
}
