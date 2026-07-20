import http from "node:http";
import { spawn } from "node:child_process";
import { readFileSync, existsSync, unlinkSync, rmSync } from "node:fs";
import { readFile as readFileAsync, rm as rmAsync, unlink as unlinkAsync, writeFile as writeFileAsync } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import crypto from "node:crypto";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const PORT = Number.parseInt(process.env.LOCAL_BROWSER_EXECUTOR_PORT || "42341", 10);
const BEARER_TOKEN =
  typeof process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN === "string" &&
  process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN.trim()
    ? process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN.trim()
    : null;

const PROVIDER_SCRIPTS = {
  aistudio: path.join(__dirname, "aistudio-web-browser-worker.mjs"),
  udio: path.join(__dirname, "udio-browser-worker.mjs"),
  suno: path.join(__dirname, "suno-browser-worker.mjs"),
  producer: path.join(__dirname, "producer-browser-worker.mjs"),
  lumalabs: path.join(__dirname, "lumalabs-browser-worker.mjs"),
};

function writeJson(res, status, payload) {
  res.statusCode = status;
  res.setHeader("content-type", "application/json; charset=utf-8");
  res.end(JSON.stringify(payload));
}

function unauthorized(res) {
  writeJson(res, 401, {
    ok: false,
    error: {
      code: "browser_executor_unauthorized",
      message: "Browser executor token is required.",
      status: 401,
    },
  });
}

function authorized(req) {
  if (!BEARER_TOKEN) {
    return true;
  }
  const authHeader = req.headers.authorization;
  const internalApiKey = req.headers["x-internal-api-key"];
  const provided =
    (typeof authHeader === "string" && authHeader.replace(/^Bearer\s+/i, "").trim()) ||
    (typeof internalApiKey === "string" && internalApiKey.trim()) ||
    null;
  return provided === BEARER_TOKEN;
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
    req.on("end", () => {
      try {
        const text = Buffer.concat(chunks).toString("utf8");
        resolve(text ? JSON.parse(text) : {});
      } catch (error) {
        reject(error);
      }
    });
    req.on("error", reject);
  });
}

function browserExecutionStatusFromError(error) {
  const message = String(error?.message || error?.body || "").toLowerCase();
  if (message.includes("challenge")) {
    return "challenge_required";
  }
  if (message.includes("auth") || message.includes("unauthorized")) {
    return "auth_required";
  }
  return "failed";
}

async function readResultFilePayload(filePath) {
  const serialized = await readFileAsync(filePath, "utf8");
  try {
    await unlinkAsync(filePath);
  } catch (_) {}
  return JSON.parse(serialized);
}

function createWorkerResultFilePath(provider) {
  const safeProvider = String(provider || "worker").replace(/[^a-z0-9._-]+/gi, "-");
  return path.join(
    os.tmpdir(),
    `browser-executor-${safeProvider}-${crypto.randomUUID()}.json`,
  );
}

function spawnWorker(scriptPath, input, provider = "worker") {
  return new Promise((resolve, reject) => {
    const resultFilePath =
      typeof input?.resultFilePath === "string" && input.resultFilePath.trim()
        ? input.resultFilePath.trim()
        : createWorkerResultFilePath(provider);
    const workerInput = {
      ...(input && typeof input === "object" ? input : {}),
      resultFilePath,
    };
    const child = spawn(process.execPath, [scriptPath], {
      cwd: REPO_ROOT,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    let settled = false;

    const finishResolve = async (payload) => {
      if (settled) {
        return;
      }
      settled = true;
      try {
        child.kill();
      } catch (_) {}
      resolve(payload);
    };

    const finishReject = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      try {
        child.kill();
      } catch (_) {}
      reject(error);
    };

    const pollResultFile = async () => {
      if (settled) {
        return;
      }
      try {
        if (existsSync(resultFilePath)) {
          const payload = await readResultFilePayload(resultFilePath);
          await finishResolve(payload);
          return;
        }
      } catch (error) {
        finishReject(
          new Error(`Browser worker result-file parse failed: ${error.message}`),
        );
        return;
      }
      setTimeout(pollResultFile, 50);
    };

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", finishReject);
    child.on("close", async (code) => {
      if (settled) {
        return;
      }
      try {
        if (existsSync(resultFilePath)) {
          const payload = await readResultFilePayload(resultFilePath);
          await finishResolve(payload);
          return;
        }
      } catch (error) {
        finishReject(
          new Error(`Browser worker result-file parse failed: ${error.message}`),
        );
        return;
      }
      if (!stdout.trim()) {
        finishReject(
          new Error(
            `Browser worker exited without JSON output (code=${code ?? "unknown"}). ${stderr}`.trim(),
          ),
        );
        return;
      }
      try {
        await finishResolve(JSON.parse(stdout));
      } catch (error) {
        finishReject(
          new Error(
            `Browser worker returned invalid JSON: ${error.message}. stderr=${stderr}`.trim(),
          ),
        );
      }
    });

    pollResultFile().catch(finishReject);

    child.stdin.write(JSON.stringify(workerInput));
    child.stdin.end();
  });
}

function normalizeWorkerResponse(workerResponse) {
  if (
    !workerResponse ||
    typeof workerResponse !== "object" ||
    typeof workerResponse.bodyFilePath !== "string" ||
    !workerResponse.bodyFilePath.trim()
  ) {
    return workerResponse;
  }
  const filePath = workerResponse.bodyFilePath.trim();
  const bodyText = readFileSync(filePath, "utf8");
  try {
    unlinkSync(filePath);
  } catch (_) {}
  try {
    rmSync(path.dirname(filePath), { recursive: true, force: true });
  } catch (_) {}
  return {
    ...workerResponse,
    bodyFilePath: null,
    bodyText,
  };
}

async function handleHealth(_req, res) {
  writeJson(res, 200, {
    ok: true,
    enabled: true,
    mode: "local_node_browser_executor_service",
    remoteBaseUrl: null,
    aistudioScriptPath: PROVIDER_SCRIPTS.aistudio,
    lumalabsScriptPath: PROVIDER_SCRIPTS.lumalabs,
    producerScriptPath: PROVIDER_SCRIPTS.producer,
    sunoScriptPath: PROVIDER_SCRIPTS.suno,
    udioScriptPath: PROVIDER_SCRIPTS.udio,
    geminiCanvasPoolScriptPath: "",
  });
}

async function handleExecute(req, res) {
  let body;
  try {
    body = await readJsonBody(req);
  } catch (error) {
    writeJson(res, 400, {
      ok: false,
      status: 400,
      error: {
        code: "browser_executor_invalid_json",
        message: `Invalid request body: ${error.message}`,
        status: 400,
      },
    });
    return;
  }

  const provider = String(body?.provider || "").trim().toLowerCase();
  const scriptPath = PROVIDER_SCRIPTS[provider];
  if (!scriptPath || !existsSync(scriptPath)) {
    writeJson(res, 400, {
      ok: false,
      provider,
      status: 400,
      error: {
        code: "browser_executor_unknown_provider",
        message: `Unsupported browser executor provider: ${provider || "unknown"}`,
        status: 400,
      },
      lease: null,
      browserExecutionStatus: "failed",
    });
    return;
  }

  try {
    const workerResponse = normalizeWorkerResponse(
      await spawnWorker(scriptPath, body.input ?? {}, provider),
    );
    if (workerResponse?.ok) {
      const resultPayload =
        provider === "aistudio" ? workerResponse : workerResponse.result ?? null;
      writeJson(res, 200, {
        ok: true,
        provider,
        status: workerResponse.status ?? 200,
        result: resultPayload,
        error: null,
        lease: null,
        browserExecutionStatus: "completed",
      });
      return;
    }

    const error = workerResponse?.error ?? {};
    writeJson(res, 200, {
      ok: false,
      provider,
      status: error.status ?? workerResponse?.status ?? 500,
      result: null,
      error: {
        code: error.code ?? "browser_executor_worker_failed",
        message: error.message ?? "Browser worker failed.",
        status: error.status ?? workerResponse?.status ?? 500,
        body: error.body ?? null,
      },
      lease: null,
      browserExecutionStatus: browserExecutionStatusFromError(error),
    });
  } catch (error) {
    writeJson(res, 200, {
      ok: false,
      provider,
      status: 500,
      result: null,
      error: {
        code: "browser_executor_spawn_failed",
        message: error.message,
        status: 500,
        body: null,
      },
      lease: null,
      browserExecutionStatus: "failed",
    });
  }
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url || "/", `http://${req.headers.host || "127.0.0.1"}`);
    if (!authorized(req)) {
      unauthorized(res);
      return;
    }
    if (req.method === "GET" && url.pathname === "/v1/internal/browser-executor/health") {
      await handleHealth(req, res);
      return;
    }
    if (req.method === "POST" && url.pathname === "/v1/internal/browser-executor/execute") {
      await handleExecute(req, res);
      return;
    }
    writeJson(res, 404, {
      ok: false,
      error: {
        code: "browser_executor_not_found",
        message: "Not found.",
        status: 404,
      },
    });
  } catch (error) {
    writeJson(res, 500, {
      ok: false,
      error: {
        code: "browser_executor_service_failed",
        message: error.message,
        status: 500,
      },
    });
  }
});

server.listen(PORT, "127.0.0.1", () => {
  process.stdout.write(
    JSON.stringify({
      ok: true,
      port: PORT,
      mode: "local_node_browser_executor_service",
      providerScripts: PROVIDER_SCRIPTS,
    }) + "\n",
  );
});

process.on("SIGINT", () => server.close(() => process.exit(0)));
process.on("SIGTERM", () => server.close(() => process.exit(0)));
