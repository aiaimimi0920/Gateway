import { existsSync, lstatSync, mkdirSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { GetObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

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
      (
        process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        ""
      )
        .trim()
        .toLowerCase(),
    ),
  };
}

export function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.isAbsolute(config.localDir)
    ? config.localDir
    : path.resolve(process.cwd(), config.localDir);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw Object.assign(new Error("Remote object storage is not fully configured."), {
      status: 500,
      code: "gemini_canvas_object_storage_not_configured",
    });
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
  const body = response.Body;
  if (!body || typeof body.transformToByteArray !== "function") {
    throw Object.assign(
      new Error("Gemini Canvas runtime-state download did not return a readable object body."),
      {
        status: 500,
        code: "gemini_canvas_remote_profile_body_missing",
      },
    );
  }
  const bytes = await body.transformToByteArray();
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, Buffer.from(bytes));
  return absolutePath;
}

function looksLikePersistentBrowserProfileDir(absolutePath) {
  const defaultProfileDir = path.join(absolutePath, "Default");
  const localStatePath = path.join(absolutePath, "Local State");
  return (
    existsSync(defaultProfileDir) &&
    lstatSync(defaultProfileDir).isDirectory() &&
    existsSync(localStatePath) &&
    lstatSync(localStatePath).isFile()
  );
}

export async function resolveRuntimeStateSource(runtimeStateObjectKey, options = {}) {
  const { allowFixtureEmptyProfile = false } = options;
  const config = resolveObjectStorageConfig();
  const normalizedKey = normalizeString(runtimeStateObjectKey);
  const absolutePath =
    normalizedKey && path.isAbsolute(normalizedKey)
      ? normalizedKey
      : path.join(getStorageRoot(), ...String(runtimeStateObjectKey || "").split("/"));
  if (!existsSync(absolutePath) && config.driver !== "local") {
    try {
      await mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath);
    } catch (error) {
      if (allowFixtureEmptyProfile) {
        mkdirSync(absolutePath, { recursive: true });
        return {
          mode: "profile_dir",
          absolutePath,
        };
      }
      const message = error instanceof Error ? error.message : String(error);
      throw Object.assign(
        new Error(
          `Gemini Canvas browser profiles currently require a locally mirrored browser profile directory or storageState JSON. Automatic mirror for '${runtimeStateObjectKey}' failed: ${message}`,
        ),
        {
          status: 500,
          code: "gemini_canvas_remote_profile_not_supported",
        },
      );
    }
  }
  if (existsSync(absolutePath)) {
    const stat = lstatSync(absolutePath);
    if (stat.isDirectory()) {
      if (looksLikePersistentBrowserProfileDir(absolutePath)) {
        return {
          mode: "profile_dir",
          absolutePath,
        };
      }
      const embeddedStorageStatePath = path.join(absolutePath, "storage-state.json");
      if (existsSync(embeddedStorageStatePath) && lstatSync(embeddedStorageStatePath).isFile()) {
        return {
          mode: "storage_state_file",
          absolutePath: embeddedStorageStatePath,
        };
      }
      return {
        mode: "profile_dir",
        absolutePath,
      };
    }
    if (stat.isFile() && absolutePath.toLowerCase().endsWith(".json")) {
      return {
        mode: "storage_state_file",
        absolutePath,
      };
    }
    throw Object.assign(
      new Error(
        `Gemini Canvas runtimeStateObjectKey must point to either a persistent browser profile directory or a Playwright storageState JSON file, but ${absolutePath} is neither.`,
      ),
      {
        status: 400,
        code: "gemini_canvas_runtime_state_invalid_path",
      },
    );
  }

  if (config.driver !== "local") {
    throw Object.assign(
      new Error(
        "Gemini Canvas browser profiles currently require a locally mirrored browser profile directory or storageState JSON. Remote object storage bundles without a local mirror are not supported yet.",
      ),
      {
        status: 500,
        code: "gemini_canvas_remote_profile_not_supported",
      },
    );
  }

  mkdirSync(absolutePath, { recursive: true });

  return {
    mode: "profile_dir",
    absolutePath,
  };
}
