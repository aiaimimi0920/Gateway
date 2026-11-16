import type { GrokAuthSession } from "../../api/console/grok-auth";

// Owned by one console client/management identity, never localStorage or a module singleton.
// Only non-secret save receipts survive dialog unmounts. Unknown outcomes are not evicted by age.
export type GrokOAuthOperations = Map<string, { session: GrokAuthSession; uncertain: boolean; rejected?: boolean }>;
export const grokOperationKey = (providerId: string, credentialId?: string) => JSON.stringify([providerId, credentialId ?? null]);
