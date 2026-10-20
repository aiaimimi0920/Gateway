import { z } from "zod";

// A generated reply is independent of whether its exact routing-health row changed.
export const modelHealthRecoverySchema = z.enum([
  "applied", "stale_health", "stale_configuration", "configuration_changed_after_write",
  "storage_failed", "unavailable", "protected", "failure_recorded", "failure_not_penalized",
]);
