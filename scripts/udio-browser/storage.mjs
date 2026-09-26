import path from "node:path";
import process from "node:process";
import { GetObjectCommand, PutObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { normalizeString } from "./request.mjs";
import { readLocalState, writeLocalState } from "./local-state.mjs";
import { assertRuntimeStateSize, readRuntimeStateBody } from "./state-body.mjs";
import { scopeRuntimeState } from "./state-scope.mjs";

let objectStorageClient = null;
function resolveObjectStorageConfig() {
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
      (process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        "")
        .trim()
        .toLowerCase(),
    ),
  };
}

function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.resolve(process.cwd(), config.localDir);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("Udio worker object storage is not fully configured.");
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

async function maybeReadRuntimeState(objectKey) {
  const normalized = normalizeString(objectKey);
  if (!normalized) {
    return null;
  }

  try {
    const config = resolveObjectStorageConfig();
    let buffer;
    if (config.driver === "local") {
      buffer = await readLocalState(getStorageRoot(), normalized);
    } else {
      const client = getS3Client(config);
      const response = await client.send(
        new GetObjectCommand({
          Bucket: config.bucket,
          Key: normalized,
        }),
      );
      buffer = await readRuntimeStateBody(response.Body);
    }

    if (!buffer?.length) {
      return null;
    }
    return JSON.parse(buffer.toString("utf8"));
  } catch {
    return null;
  }
}

async function maybePersistRuntimeState(context, objectKey, existingState = null, baseUrl = "https://www.udio.com") {
  const normalized = normalizeString(objectKey);
  if (!normalized) {
    return;
  }

  const state = scopeRuntimeState(await context.storageState(), baseUrl);
  existingState = scopeRuntimeState(existingState, baseUrl);
  if (!hasPersistableRuntimeState(state) && !hasPersistableRuntimeState(existingState)) {
    return;
  }
  if (!shouldPersistRuntimeState(existingState, state)) {
    return;
  }
  const text = JSON.stringify(state, null, 2);
  assertRuntimeStateSize(Buffer.byteLength(text, "utf8"));
  const buffer = Buffer.from(text, "utf8");
  const config = resolveObjectStorageConfig();

  if (config.driver === "local") {
    await writeLocalState(getStorageRoot(), normalized, buffer);
    return;
  }

  const client = getS3Client(config);
  await client.send(
    new PutObjectCommand({
      Bucket: config.bucket,
      Key: normalized,
      Body: buffer,
      ContentType: "application/json",
    }),
  );
}

function shouldPersistRuntimeState(existingState, nextState) {
  if (hasPersistableRuntimeState(nextState)) {
    return true;
  }
  if (hasPersistableRuntimeState(existingState)) {
    return false;
  }
  return true;
}

function hasPersistableRuntimeState(state) {
  return hasUdioAuthCookies(state) || hasUdioAuthLocalStorage(state);
}

function hasUdioAuthCookies(state) {
  const cookies = Array.isArray(state?.cookies) ? state.cookies : [];
  return cookies.some(
    (cookie) =>
      typeof cookie?.name === "string" &&
      cookie.name.startsWith("sb-ssr-production-auth-token"),
  );
}

function hasUdioAuthLocalStorage(state) {
  const origins = Array.isArray(state?.origins) ? state.origins : [];
  for (const origin of origins) {
    const entries = Array.isArray(origin?.localStorage) ? origin.localStorage : [];
    for (const entry of entries) {
      const key = String(entry?.name ?? "");
      const raw = String(entry?.value ?? "");
      if (!key || !raw) {
        continue;
      }
      const lower = key.toLowerCase();
      if (!lower.includes("supabase") && !lower.includes("auth")) {
        continue;
      }
      try {
        const parsed = JSON.parse(raw);
        const token =
          parsed?.access_token ??
          parsed?.currentSession?.access_token ??
          parsed?.session?.access_token ??
          parsed?.data?.session?.access_token;
        if (typeof token === "string" && token.trim()) {
          return true;
        }
      } catch {
        // Ignore non-JSON cache entries.
      }
    }
  }
  return false;
}


export { maybeReadRuntimeState, maybePersistRuntimeState, hasPersistableRuntimeState, shouldPersistRuntimeState };

