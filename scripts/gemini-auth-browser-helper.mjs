import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "..");

const GEMINI_CANVAS_CAPTURE_SCRIPT = path.join(__dirname, "export-gemini-canvas-storage-state.mjs");
const GEMINI_BUSINESS_CAPTURE_SCRIPT = path.join(__dirname, "export-gemini-business-runtime.mjs");
const GEMINI_WEB_CAPTURE_SCRIPT = path.join(__dirname, "export-gemini-web-runtime.mjs");
const DEFAULT_GEMINI_CANVAS_SHARE_ID = "fe24c455a570";

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function readJsonStdin() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
    process.stdin.on("end", () => {
      try {
        const text = Buffer.concat(chunks).toString("utf8").trim();
        resolve(text ? JSON.parse(text) : {});
      } catch (error) {
        reject(error);
      }
    });
    process.stdin.on("error", reject);
  });
}

function summarizeOutput(text) {
  const trimmed = String(text ?? "").trim();
  if (trimmed.length <= 400) {
    return trimmed;
  }
  return `${trimmed.slice(0, 400)}...(truncated)`;
}

function extractLastJsonObject(text) {
  const source = String(text ?? "");
  const starts = [];
  for (let index = source.indexOf("{"); index >= 0; index = source.indexOf("{", index + 1)) {
    starts.push(index);
  }
  for (let index = starts.length - 1; index >= 0; index -= 1) {
    const candidate = source.slice(starts[index]).trim();
    try {
      return JSON.parse(candidate);
    } catch {
      // Keep scanning backward until a valid JSON object is found.
    }
  }
  return null;
}

function buildFailure(code, message, body = null, status = 500) {
  return {
    ok: false,
    error: {
      code,
      message,
      status,
      body,
    },
  };
}

function buildCanvasSuccess(targetFamily, payload) {
  const runtimeStateObjectKey = normalizeString(payload?.runtimeStateObjectKey);
  if (!runtimeStateObjectKey) {
    throw new Error("Gemini Canvas helper did not return runtimeStateObjectKey.");
  }
  const browserRuntimeStateObjectKey =
    normalizeString(payload?.browserRuntimeStateObjectKey) ?? null;
  const apiKeys = Array.isArray(payload?.apiKeys)
    ? payload.apiKeys
        .map((candidate) => normalizeString(candidate))
        .filter(Boolean)
    : [];
  return {
    ok: true,
    result: {
      ok: true,
      targetFamily,
      runtimeStateObjectKey,
      browserRuntimeStateObjectKey,
      suggestedShareId:
        normalizeString(payload?.suggestedShareId) ?? DEFAULT_GEMINI_CANVAS_SHARE_ID,
      apiKeys,
      currentUrl: normalizeString(payload?.currentUrl),
      note:
        normalizeString(payload?.note)
        ?? "Captured Gemini Canvas browser storage state on the host browser executor.",
    },
  };
}

function buildBusinessSuccess(payload) {
  const jwt = normalizeString(payload?.jwt);
  const configId = normalizeString(payload?.configId);
  const session = normalizeString(payload?.session);
  if (!jwt || !configId || !session) {
    throw new Error("Gemini Business helper did not return jwt, configId, and session.");
  }
  return {
    ok: true,
    result: {
      ok: true,
      targetFamily: "gemini-business",
      jwt,
      configId,
      session,
      currentUrl: normalizeString(payload?.currentUrl),
      note:
        normalizeString(payload?.note)
        ?? "Captured Gemini Business runtime material on the host browser executor.",
    },
  };
}

function buildWebSuccess(payload) {
  const apiKey = normalizeString(payload?.apiKey);
  if (!apiKey) {
    throw new Error("Gemini Web helper did not return the primary session cookie.");
  }
  return {
    ok: true,
    result: {
      ok: true,
      targetFamily: "gemini-web",
      apiKey,
      authToken: normalizeString(payload?.authToken),
      accountIndex: normalizeString(payload?.accountIndex),
      accessToken: normalizeString(payload?.accessToken),
      buildLabel: normalizeString(payload?.buildLabel),
      sessionId: normalizeString(payload?.sessionId),
      language: normalizeString(payload?.language),
      appPagePath: normalizeString(payload?.appPagePath),
      endpointPath: normalizeString(payload?.endpointPath),
      referer: normalizeString(payload?.referer),
      modelHeaders: payload?.modelHeaders ?? {},
      requestContextHeader: normalizeString(payload?.requestContextHeader),
      responseStatus: Number.isInteger(payload?.responseStatus) ? payload.responseStatus : null,
      responseContainsParis: payload?.responseContainsParis === true,
      currentUrl: normalizeString(payload?.currentUrl),
      note:
        normalizeString(payload?.note)
        ?? "Captured Gemini Web runtime on the host browser executor.",
    },
  };
}

function spawnCaptureScript(scriptPath, envOverrides) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [scriptPath], {
      cwd: REPO_ROOT,
      env: {
        ...process.env,
        ...envOverrides,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code) => {
      const combined = stderr.trim()
        ? stdout.trim()
          ? `${stdout.trim()}\n${stderr.trim()}`
          : stderr.trim()
        : stdout.trim();
      const parsed = extractLastJsonObject(combined);
      if (!parsed) {
        reject(
          new Error(
            `Gemini auth capture helper returned no parseable JSON output (code=${code ?? "unknown"}). stdout=${summarizeOutput(stdout)} stderr=${summarizeOutput(stderr)}`,
          ),
        );
        return;
      }
      resolve(parsed);
    });
  });
}

async function main() {
  let input;
  try {
    input = await readJsonStdin();
  } catch (error) {
    process.stdout.write(
      `${JSON.stringify(buildFailure("gemini_auth_invalid_json", `Invalid JSON input: ${error.message}`, null, 400), null, 2)}\n`,
    );
    process.exit(1);
    return;
  }

  const targetFamily = normalizeString(input?.targetFamily ?? input?.target_family)?.toLowerCase();
  if (!targetFamily) {
    process.stdout.write(
      `${JSON.stringify(buildFailure("gemini_auth_missing_target_family", "Gemini auth helper requires targetFamily.", null, 400), null, 2)}\n`,
    );
    process.exit(1);
    return;
  }

  try {
    if (targetFamily === "gemini-canvas" || targetFamily === "gemini-canvas-chat") {
      const accountLabel = normalizeString(input?.accountLabel ?? input?.account_label);
      const forceCompleteSignalRelativePath = normalizeString(
        input?.forceCompleteSignalRelativePath ?? input?.force_complete_signal_relative_path,
      );
      const payload = await spawnCaptureScript(GEMINI_CANVAS_CAPTURE_SCRIPT, {
        GEMINI_CANVAS_CAPTURE_OUTPUT_MODE: "storage_state",
        ...(accountLabel ? { GEMINI_CANVAS_CAPTURE_ACCOUNT_LABEL: accountLabel } : {}),
        ...(forceCompleteSignalRelativePath
          ? { GEMINI_CANVAS_CAPTURE_FORCE_COMPLETE_FILE: forceCompleteSignalRelativePath }
          : {}),
      });
      if (!payload?.ok) {
        throw new Error(normalizeString(payload?.error) ?? "Gemini Canvas auth helper failed.");
      }
      process.stdout.write(`${JSON.stringify(buildCanvasSuccess(targetFamily, payload), null, 2)}\n`);
      return;
    }

    if (targetFamily === "gemini-business") {
      const payload = await spawnCaptureScript(GEMINI_BUSINESS_CAPTURE_SCRIPT, {});
      if (!payload?.ok) {
        throw new Error(normalizeString(payload?.error) ?? "Gemini Business auth helper failed.");
      }
      process.stdout.write(`${JSON.stringify(buildBusinessSuccess(payload), null, 2)}\n`);
      return;
    }

    if (targetFamily === "gemini-web") {
      const payload = await spawnCaptureScript(GEMINI_WEB_CAPTURE_SCRIPT, {});
      if (!payload?.ok) {
        throw new Error(normalizeString(payload?.error) ?? "Gemini Web auth helper failed.");
      }
      process.stdout.write(`${JSON.stringify(buildWebSuccess(payload), null, 2)}\n`);
      return;
    }

    process.stdout.write(
      `${JSON.stringify(buildFailure("gemini_auth_unsupported_target_family", `Unsupported Gemini targetFamily: ${targetFamily}`, null, 400), null, 2)}\n`,
    );
    process.exit(1);
  } catch (error) {
    process.stdout.write(
      `${JSON.stringify(buildFailure("gemini_auth_helper_failed", error instanceof Error ? error.message : String(error)), null, 2)}\n`,
    );
    process.exit(1);
  }
}

main().catch((error) => {
  process.stdout.write(
    `${JSON.stringify(buildFailure("gemini_auth_helper_failed", error instanceof Error ? error.message : String(error)), null, 2)}\n`,
  );
  process.exit(1);
});
