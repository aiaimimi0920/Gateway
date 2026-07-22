import { z } from "zod";

export const MANAGEMENT_SESSION_STORAGE_KEY = "neuro.gateway.console.management-session.v1";

const storedSessionSchema = z.object({
  version: z.literal(1),
  managementToken: z.string().min(1),
});

export function getManagementSessionStorage(): Storage | undefined {
  return typeof window === "undefined" ? undefined : window.sessionStorage;
}

function defaultSessionNamespace(): string {
  return typeof window === "undefined" ? "server" : window.location.origin;
}

function storageKey(namespace = defaultSessionNamespace()): string {
  return `${MANAGEMENT_SESSION_STORAGE_KEY}:${encodeURIComponent(namespace)}`;
}

export function readManagementSessionToken(
  storage: Storage | undefined = getManagementSessionStorage(),
  namespace?: string,
): string | null {
  if (!storage) {
    return null;
  }
  const key = storageKey(namespace);
  const raw = storage.getItem(key);
  if (!raw) {
    return null;
  }
  try {
    const parsed = storedSessionSchema.safeParse(JSON.parse(raw) as unknown);
    if (parsed.success) {
      return parsed.data.managementToken;
    }
  } catch {
    // Invalid session data is removed below.
  }
  storage.removeItem(key);
  return null;
}

export function writeManagementSessionToken(
  managementToken: string,
  storage: Storage | undefined = getManagementSessionStorage(),
  namespace?: string,
): void {
  if (!storage) {
    return;
  }
  const token = managementToken.trim();
  if (!token) {
    throw new Error("Management token cannot be empty.");
  }
  storage.setItem(
    storageKey(namespace),
    JSON.stringify({ version: 1, managementToken: token }),
  );
}

export function clearManagementSession(
  storage: Storage | undefined = getManagementSessionStorage(),
  namespace?: string,
): void {
  storage?.removeItem(storageKey(namespace));
}
