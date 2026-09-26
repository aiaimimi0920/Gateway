import type { GatewayDesktopNotice } from "../lib/types";

export function notice(tone: GatewayDesktopNotice["tone"], message: string): GatewayDesktopNotice {
  return { tone, message };
}

export function toMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
