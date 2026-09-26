import os from "node:os";
import path from "node:path";
import { mkdir, writeFile } from "node:fs/promises";

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export async function maybeWriteCredentialFile(input, result, deps) {
  const { parseBoolean } = deps;
  const shouldWrite =
    input?.writeCredentialFile === true ||
    typeof input?.credentialFilePath === "string" ||
    typeof input?.credentialRootDir === "string" ||
    parseBoolean(process.env.QWEN_WEB_WRITE_CREDENTIAL_FILE, false);
  if (!shouldWrite || !result?.ok) {
    return null;
  }

  const targetPath = resolveCredentialFilePath(input, result, deps);
  await mkdir(path.dirname(targetPath), { recursive: true });
  const payload = buildCredentialPayload(result);
  await writeFile(targetPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return targetPath;
}

function buildCredentialPayload(result) {
  const payload = {
    apiKey: result.authToken,
  };
  if (normalizeString(result.expiresAt)) {
    payload.expiresAt = result.expiresAt;
  }
  if (normalizeString(result.cookieHeader)) {
    payload.headers = {
      Cookie: result.cookieHeader,
    };
  }
  if (normalizeString(result.selectedModel)) {
    payload.supportedModels = [result.selectedModel];
  }
  if (normalizeString(result.selectedDisplayModel)) {
    payload.selectedDisplayModel = result.selectedDisplayModel;
  }
  const accountName =
    normalizeString(result?.authProbe?.email) ?? normalizeString(result?.authProbe?.userId);
  if (accountName) {
    payload.accountName = accountName;
  }
  const materialKey = normalizeString(result?.authProbe?.userId)
    ? `qwen-web-user:${result.authProbe.userId}`
    : null;
  if (materialKey) {
    payload.credentialMaterialKey = materialKey;
  }
  payload.rawSource = {
    tokenSource: result.tokenSource ?? null,
    authProbe: result.authProbe ?? null,
    modelProbe: result.modelProbe ?? null,
    createChatProbe: result.createChatProbe ?? null,
    profileSource: result.profileSource ?? null,
    localStorageKeys: Array.isArray(result.localStorageKeys) ? result.localStorageKeys : [],
  };
  return payload;
}

function resolveCredentialFilePath(input, result, deps) {
  const { defaultFamilyDir } = deps;
  const explicitPath = normalizeString(input?.credentialFilePath);
  if (explicitPath) {
    return ensureJsonExtension(explicitPath);
  }

  const rootDir =
    normalizeString(input?.credentialRootDir) ??
    normalizeString(process.env.NEURO_PROVIDER_CREDENTIAL_ROOT_DIR) ??
    path.join(os.homedir(), ".neuro");
  const familyDir =
    normalizeString(input?.credentialFamilyDir) ?? defaultFamilyDir;
  const explicitFileName = normalizeString(input?.credentialFileName);
  const accountHint =
    normalizeString(result?.authProbe?.email) ??
    normalizeString(result?.authProbe?.userId) ??
    normalizeString(result?.selectedModel) ??
    `qwen-web-${Date.now()}`;
  const inferredFileName =
    explicitFileName ??
    sanitizeFileNameComponent(accountHint) ??
    `qwen-web-${Date.now()}.json`;
  const fileName = ensureJsonExtension(inferredFileName);
  return path.join(rootDir, familyDir, fileName);
}

function ensureJsonExtension(filePath) {
  return filePath.toLowerCase().endsWith(".json") ? filePath : `${filePath}.json`;
}

function sanitizeFileNameComponent(value) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || null;
}
