import type { ConsoleApi } from "../api/console";
import type { ManagementSession } from "../api/contracts";
import { GatewayApiError } from "../api/errors";

export function isUsableManagementToken(value: string): boolean {
  return /^[\x21-\x7e]{1,4096}$/.test(value);
}

/** A lost write response is ambiguous: read back once, never repeat the rotation. */
export async function rotateManagementToken(
  api: Pick<ConsoleApi, "rotateSession" | "verifySession">,
  currentToken: string,
  newToken: string,
  isCurrent: () => boolean,
): Promise<ManagementSession | undefined> {
  // Validate before committing: the replacement must remain usable in HTTP headers.
  if (!isUsableManagementToken(newToken)) throw new Error("New management token must contain 1-4096 visible ASCII characters.");
  try {
    await api.rotateSession(currentToken, newToken);
    return undefined;
  } catch (cause) {
    if (!isCurrent() || (cause instanceof GatewayApiError && cause.status < 500)) throw cause;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5000);
    try {
      // A rejected candidate is not evidence that the still-current token is invalid.
      return await api.verifySession(newToken, {
        notifyAuthenticationFailure: false,
        signal: controller.signal,
      });
    } catch {
      throw cause;
    } finally {
      clearTimeout(timer);
    }
  }
}
