import { existsSync, lstatSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { S3Client, GetObjectCommand, PutObjectCommand } from "@aws-sdk/client-s3";
import { normalizeString } from "./input-text.mjs";
import { resolveStorageObjectPath } from "./storage-path.mjs";
import { writeAtomicCapture as writeFile } from "./atomic-capture-file.mjs";
import { withObjectStorageRequest, readRuntimeStateBody, MAX_RUNTIME_STATE_BYTES,
  runtimeStateTooLarge } from "./object-storage-request.mjs";

const AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME = "aistudio-target-rpc-contract.json";
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

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("AI Studio live probe object storage is not fully configured.");
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
  const bytes = await withObjectStorageRequest(async (signal) => {
    const response = await client.send(new GetObjectCommand({
      Bucket: config.bucket, Key: runtimeStateObjectKey,
    }), { abortSignal: signal });
    return readRuntimeStateBody(response.Body, signal, response.ContentLength);
  });
  await mkdir(path.dirname(absolutePath), { recursive: true });
  resolveStorageObjectPath(path.resolve(process.cwd(), config.localDir), runtimeStateObjectKey);
  await writeFile(absolutePath, bytes);
  return absolutePath;
}

function buildTargetRpcContractObjectKey(runtimeStateObjectKey) {
  const normalized = normalizeString(runtimeStateObjectKey);
  if (!normalized) {
    return null;
  }
  if (normalized.endsWith("/storage-state.json")) {
    return `${normalized.slice(0, -"/storage-state.json".length)}/${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
  }
  if (normalized.endsWith("\\storage-state.json")) {
    return `${normalized.slice(0, -"\\storage-state.json".length)}/${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
  }
  return `${normalized}.${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
}

async function persistJsonObjectMirror(objectKey, payload) {
  const normalizedKey = normalizeString(objectKey);
  if (!normalizedKey) {
    return null;
  }
  const config = resolveObjectStorageConfig();
  const root = path.resolve(process.cwd(), config.localDir);
  const absolutePath = resolveStorageObjectPath(root, normalizedKey);
  const text = `${JSON.stringify(payload, null, 2)}\n`;
  if (Buffer.byteLength(text, "utf8") > MAX_RUNTIME_STATE_BYTES) throw runtimeStateTooLarge();
  const serialized = Buffer.from(text, "utf8");
  await mkdir(path.dirname(absolutePath), { recursive: true });
  resolveStorageObjectPath(root, normalizedKey);
  await writeFile(absolutePath, serialized);
  if (config.driver === "s3-compatible") {
    const client = getS3Client(config);
    await withObjectStorageRequest((signal) => client.send(
      new PutObjectCommand({
        Bucket: config.bucket,
        Key: normalizedKey,
        ContentType: "application/json",
        Body: serialized,
      }), { abortSignal: signal },
    ));
  }
  return absolutePath;
}

async function resolveRuntimeStateSource(runtimeStateObjectKey) {
  const config = resolveObjectStorageConfig();
  const absolutePath = resolveStorageObjectPath(path.resolve(process.cwd(), config.localDir), runtimeStateObjectKey);

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

function closeObjectStorageClient() {
  objectStorageClient?.destroy();
  objectStorageClient = null;
}

export { resolveRuntimeStateSource, buildTargetRpcContractObjectKey, persistJsonObjectMirror, closeObjectStorageClient };
