import { existsSync, lstatSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { S3Client, GetObjectCommand } from "@aws-sdk/client-s3";
import { normalizeString } from "./settings.mjs";

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

async function toBuffer(stream) {
  if (!stream) return Buffer.alloc(0);
  if (Buffer.isBuffer(stream)) return stream;
  if (typeof stream === "object" && typeof stream.transformToByteArray === "function") {
    return Buffer.from(await stream.transformToByteArray());
  }
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("AI Studio browser worker object storage is not fully configured.");
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

async function mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath) {
  const client = getS3Client(config);
  const response = await client.send(
    new GetObjectCommand({
      Bucket: config.bucket,
      Key: runtimeStateObjectKey,
    }),
  );
  const bytes = await toBuffer(response.Body);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, bytes);
  return absolutePath;
}

export async function resolveRuntimeStateSource(runtimeStateObjectKey) {
  const config = resolveObjectStorageConfig();
  const absolutePath = path.join(getStorageRoot(), ...runtimeStateObjectKey.split("/"));

  if (!existsSync(absolutePath) && config.driver !== "local") {
    await mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath);
  }

  if (!existsSync(absolutePath)) {
    throw Object.assign(
      new Error(
        `AI Studio runtimeStateObjectKey '${runtimeStateObjectKey}' could not be resolved to a local file or directory.`,
      ),
      {
        status: 500,
        code: "aistudio_runtime_state_unavailable",
      },
    );
  }

  const stat = lstatSync(absolutePath);
  if (stat.isDirectory()) {
    return { mode: "profile_dir", absolutePath };
  }
  if (stat.isFile() && absolutePath.toLowerCase().endsWith(".json")) {
    return { mode: "storage_state_file", absolutePath };
  }

  throw Object.assign(
    new Error(
      `AI Studio runtimeStateObjectKey must point to a browser profile directory or Playwright storageState JSON file, but '${absolutePath}' is neither.`,
    ),
    {
      status: 400,
      code: "aistudio_runtime_state_invalid_path",
    },
  );
}
