import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { DEFAULT_TARGET_URL, normalizeString } from "./settings.mjs";

// Preserve one object-storage client for the lifetime of this helper process.
let objectStorageClient = null;

export function resolveObjectStorageConfig() {
  const driver =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER) ??
    normalizeString(process.env.OBJECT_STORAGE_DRIVER) ??
    "local";
  const localDir =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.OBJECT_STORAGE_LOCAL_DIR) ??
    ".runtime/ai-gateway-objects";

  return {
    driver,
    localDir,
    bucket:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET) ??
      normalizeString(process.env.OBJECT_STORAGE_BUCKET),
    region:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_REGION) ??
      normalizeString(process.env.OBJECT_STORAGE_REGION) ??
      "auto",
    endpoint:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT) ??
      normalizeString(process.env.OBJECT_STORAGE_ENDPOINT),
    accessKeyId:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID) ??
      normalizeString(process.env.OBJECT_STORAGE_ACCESS_KEY_ID),
    secretAccessKey:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY) ??
      normalizeString(process.env.OBJECT_STORAGE_SECRET_ACCESS_KEY),
    forcePathStyle: ["1", "true", "yes", "on"].includes(
      String(
        process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
          process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
          "",
      )
        .trim()
        .toLowerCase(),
    ),
  };
}

export function getStorageRoot(config) {
  return path.resolve(process.cwd(), config.localDir);
}

export function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("Remote object storage is not fully configured.");
  }
  objectStorageClient = new S3Client({
    region: config.region,
    endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle,
    credentials: {
      accessKeyId: config.accessKeyId,
      secretAccessKey: config.secretAccessKey,
    },
  });
  return objectStorageClient;
}

export async function putObject(objectKey, body) {
  const config = resolveObjectStorageConfig();
  if (config.driver === "local") {
    const absolutePath = path.join(getStorageRoot(config), ...objectKey.split("/"));
    await mkdir(path.dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, body);
    return absolutePath;
  }

  const client = getS3Client(config);
  await client.send(
    new PutObjectCommand({
      Bucket: config.bucket,
      Key: objectKey,
      Body: body,
      ContentType: "application/json",
    }),
  );
  return null;
}

export async function ensureParentDir(filePath) {
  await mkdir(path.dirname(path.resolve(process.cwd(), filePath)), { recursive: true });
}

export async function writeJsonFile(filePath, value) {
  await ensureParentDir(filePath);
  await writeFile(path.resolve(process.cwd(), filePath), JSON.stringify(value, null, 2), "utf8");
}

export async function writeStatus(statusPath, partial) {
  await writeJsonFile(statusPath, {
    ok: true,
    pid: process.pid,
    updatedAt: new Date().toISOString(),
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT ?? new Date().toISOString(),
    targetUrl: globalThis.__UDIO_MANUAL_HELPER_TARGET_URL ?? DEFAULT_TARGET_URL,
    phase: globalThis.__UDIO_MANUAL_HELPER_PHASE ?? "starting",
    ...partial,
  });
}

export async function maybeReadJsonFile(filePath) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return null;
  }
  const absolutePath = path.resolve(process.cwd(), normalized);
  if (!existsSync(absolutePath)) {
    return null;
  }
  return JSON.parse(await readFile(absolutePath, "utf8"));
}
