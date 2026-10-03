import type { ConsoleRouteConfigResponse, ConsoleRouteDocument, ConsoleSecretPatch } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { providerStoragePathError } from "./providerStoragePath";

export type StorageTarget = "storage" | "archive";
export type StorageSecretField = "password" | "access_key_id" | "secret_access_key" | "session_token";
export type StorageConnection = {
  type: "local" | "webdav" | "s3";
  path?: string;
  endpoint?: string;
  directory?: string;
  username?: string;
  bucket?: string;
  region?: string;
  prefix?: string;
  allow_insecure_http?: boolean;
};
export type StorageSecretChanges = Partial<Record<StorageSecretField, { operation: "replace" | "clear"; value?: string }>>;
export type StorageConnectionEdit = { providerId: string; target: StorageTarget; connectionIdentity: string; secrets: StorageSecretChanges };
export const storageConnectionField = (target: StorageTarget) => `credential_${target}_connection`;
export function storageSecretFields(type: StorageConnection["type"]): StorageSecretField[] {
  return type === "webdav" ? ["password"] : type === "s3" ? ["access_key_id", "secret_access_key", "session_token"] : [];
}
export function readStorageConnection(value: unknown): StorageConnection | undefined {
  if (!isRecord(value) || !["local", "webdav", "s3"].includes(String(value.type))) return undefined;
  const connection: StorageConnection = { type: value.type as StorageConnection["type"] };
  for (const key of ["path", "endpoint", "directory", "username", "bucket", "region", "prefix"] as const) {
    if (typeof value[key] === "string") connection[key] = value[key];
  }
  if (value.allow_insecure_http === true) connection.allow_insecure_http = true;
  return connection;
}
export function storageConnectionIdentity(connection: StorageConnection | undefined): string {
  return JSON.stringify([connection?.type, connection?.endpoint, connection?.username, connection?.bucket, connection?.region]);
}
export function storageConnectionLabel(connection: StorageConnection | undefined, fallback: string): string {
  if (!connection) return fallback;
  if (connection.type === "local") return connection.path || fallback;
  return connection.type === "s3" ? `S3 · ${connection.bucket}/${connection.prefix || ""}` : `WebDAV · ${connection.directory || "/"}`;
}
export function validateStorageConnection(connection: StorageConnection): string | null {
  if (connection.type === "local") return connection.path && providerStoragePathError(connection.path) ? "请输入绝对本地目录，不能含父目录跳转" : null;
  let url: URL;
  try { url = new URL(connection.endpoint || ""); } catch { return "请输入有效的云存储服务地址"; }
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.search || url.hash) return "云服务地址不能包含内嵌凭据、查询参数或片段";
  if (url.protocol !== "https:" && !connection.allow_insecure_http) return "HTTP 会明文发送凭据和数据；请改用 HTTPS 或明确允许此风险";
  const prefix = connection.type === "webdav" ? connection.directory || "" : connection.prefix || "";
  if (/^[\\/]|[\\\u0000-\u001f\u007f%]/.test(prefix) || prefix.split("/").some((part) => part === ".." || part === ".") || prefix.includes("://")) return "目录或对象前缀必须是安全的相对路径，不能含跳转或编码转义";
  if (connection.type === "s3" && (!connection.bucket?.trim() || !connection.region?.trim())) return "S3/R2 需要填写 Bucket 和 Region（R2 通常为 auto）";
  return null;
}

/** Keep patches follow provider identity; obsolete protocol fields must never be reused. */
export function buildStorageConnectionPatches(document: ConsoleRouteDocument, patches: ConsoleSecretPatch[], edits: StorageConnectionEdit[]): ConsoleSecretPatch[] {
  const result = new Map(patches.filter((patch) => {
    const match = /^\/providers\/(\d+)\/(credential_(?:storage|archive)_connection)\/([^/]+)$/.exec(patch.path);
    if (!match) return true;
    const provider = document.providers[Number(match[1])];
    const connection = isRecord(provider) ? readStorageConnection(provider[match[2]]) : undefined;
    return Boolean(connection && storageSecretFields(connection.type).includes(match[3] as StorageSecretField));
  }).map((patch) => [patch.path, patch]));
  for (const edit of edits) {
    const index = document.providers.findIndex((provider) => isRecord(provider) && provider.id === edit.providerId);
    if (index < 0) continue;
    const provider = document.providers[index];
    const connection = isRecord(provider) ? readStorageConnection(provider[storageConnectionField(edit.target)]) : undefined;
    if (!connection || storageConnectionIdentity(connection) !== edit.connectionIdentity) continue;
    for (const field of storageSecretFields(connection.type)) {
      const secret = edit.secrets[field];
      if (!secret) continue;
      const path = `/providers/${index}/${storageConnectionField(edit.target)}/${field}`;
      result.set(path, secret.operation === "clear" ? { path, operation: "clear" } : { path, operation: "replace", value: secret.value });
    }
  }
  return [...result.values()];
}


export const storageSecretIdentity = (providerId: string, target: StorageTarget) => JSON.stringify([providerId, target]);
export function configuredStorageSecrets(config: ConsoleRouteConfigResponse | null): ReadonlyMap<string, readonly string[]> {
  const result = new Map<string, string[]>();
  for (const secret of config?.routeConfig.secrets ?? []) {
    const match = /^\/providers\/(\d+)\/credential_(storage|archive)_connection\/([^/]+)$/.exec(secret.path);
    if (!match || !secret.configured) continue;
    const provider = config?.routeConfig.document.providers[Number(match[1])];
    if (!isRecord(provider) || typeof provider.id !== "string") continue;
    const key = storageSecretIdentity(provider.id, match[2] as StorageTarget);
    result.set(key, [...(result.get(key) ?? []), match[3]]);
  }
  return result;
}


export function storageConnectionEditError(document: ConsoleRouteDocument, edits: StorageConnectionEdit[]): string | null {
  for (const edit of edits) {
    if (Object.keys(edit.secrets).length === 0) continue;
    const provider = document.providers.find((entry) => isRecord(entry) && entry.id === edit.providerId);
    if (!isRecord(provider)) continue;
    if (storageConnectionIdentity(readStorageConnection(provider[storageConnectionField(edit.target)])) !== edit.connectionIdentity) {
      return "云存储连接目标已改变；请重新输入认证信息，不能将未提交的凭据移到新目标。";
    }
  }
  return null;
}
