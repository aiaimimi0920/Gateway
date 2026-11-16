import { z } from "zod";
import { createGatewayApiClient } from "../../api/client";
import type { GatewayHostAdapter } from "../../platform/types";

export type BackupDestination = { kind: "webdav" | "s3" | "github" } & Record<string, string>;
const destination = z.record(z.string(), z.string()).and(z.object({ kind: z.enum(["webdav", "s3", "github"]) }));
const preview = z.object({ createdAt: z.string(), files: z.number(), providers: z.number(), accounts: z.number(), replacesAccessKeysAndBalances: z.literal(true) });
export type BackupPreview = z.infer<typeof preview>;
const result = z.object({
  destination: destination.nullable().optional(), pending: z.boolean().optional(), revision: z.string().optional(),
  preview: preview.optional(), content: z.string().max(45 * 1024 * 1024).optional(),
  fileName: z.literal("gateway-backup.ngbk").optional(), remoteRevision: z.string().optional(),
  saved: z.boolean().optional(), uploaded: z.boolean().optional(),
});
export function gatewayBackupApi(host: GatewayHostAdapter) {
  const client = createGatewayApiClient({ host });
  return async (action: Record<string, unknown>, managementToken: string, secretGrant: string, signal: AbortSignal) => {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal.aborted) abort(); else signal.addEventListener("abort", abort, { once: true });
    const timer = setTimeout(abort, 180_000);
    try {
      return await client.request("/v1/internal/gateway/console/backup", result, {
        method: "POST", body: action, managementToken, secretGrant, signal: controller.signal,
      });
    } finally { clearTimeout(timer); signal.removeEventListener("abort", abort); }
  };
}

export async function readBackupFile(file: File): Promise<string> {
  if (file.size > 32 * 1024 * 1024 + 52) throw new Error("备份文件超过 32 MiB 限制。");
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
  return btoa(binary);
}
export function saveBackupFile(content: string) {
  const bytes = Uint8Array.from(atob(content), value => value.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  const anchor = document.createElement("a"); anchor.href = url; anchor.download = "gateway-backup.ngbk";
  document.body.appendChild(anchor); anchor.click(); anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
