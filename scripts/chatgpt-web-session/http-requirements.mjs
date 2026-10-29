// Same-origin handshake only; rejected phases never trigger another endpoint or UI submission.
import { createWorkerError } from "./errors.mjs";
import { buildProofToken } from "./sentinel-pow.mjs";

const requirementsPath = "/backend-api/sentinel/chat-requirements";
function invalid(code, stage) {
  return createWorkerError(503, code, `ChatGPT Web browser requirements failed: stage=${stage}.`);
}
function token(value, stage, optional = false) {
  if (optional && (value == null || typeof value === "string" && !value.trim())) return null;
  if (typeof value !== "string" || !value.trim() || Buffer.byteLength(value) > 32768 || /[\p{Cc}]/u.test(value)) {
    throw invalid("chatgpt_web_requirements_invalid_token", stage);
  }
  return value.trim();
}

async function exchange(page, input, headers, path, body, stage) {
  let result;
  try {
    result = await page.evaluate(async ({ url, headers, body, timeoutMs }) => {
      const response = await fetch(url, { method: "POST", credentials: "same-origin", redirect: "error",
        headers, body: JSON.stringify(body), signal: AbortSignal.timeout(timeoutMs) });
      const result = { status: response.status, contentType: response.headers.get("content-type"),
        challenge: response.headers.get("cf-mitigated") === "challenge" };
      if (!response.ok || !response.body) { await response.body?.cancel(); return result; }
      const reader = response.body.getReader();
      const chunks = [];
      let size = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          size += value.byteLength;
          if (size > 1024 * 1024) return { ...result, tooLarge: true };
          chunks.push(value);
        }
      } finally { await reader.cancel().catch(() => {}); }
      const bytes = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
      return { ...result, text: new TextDecoder().decode(bytes) };
    }, { url: `${input.baseUrl}${path}`, headers: { ...headers,
      "X-OpenAI-Target-Path": path, "X-OpenAI-Target-Route": path }, body,
    timeoutMs: Math.max(1000, Math.min(Number(input.timeoutMs) || 30000, 60000)) });
  } catch { throw invalid("chatgpt_web_requirements_network", stage); }
  if (!(result?.status >= 200 && result.status < 300)) {
    const code = result?.challenge ? "chatgpt_web_browser_challenge_required"
      : result?.status === 401 ? "chatgpt_web_session_invalid" : "chatgpt_web_requirements_http";
    throw createWorkerError(503, code,
      `ChatGPT Web browser upstream failure: stage=${stage} upstream_status=${Number(result?.status) || 0}.`);
  }
  if (result.tooLarge) throw invalid("chatgpt_web_requirements_too_large", stage);
  if (result.contentType?.split(";")[0].trim().toLowerCase() !== "application/json") {
    throw invalid("chatgpt_web_requirements_not_json", stage);
  }
  let data;
  try { data = JSON.parse(result.text); }
  catch { throw invalid("chatgpt_web_requirements_decode_failed", stage); }
  if (!data || typeof data !== "object" || Array.isArray(data)) throw invalid("chatgpt_web_requirements_invalid_shape", stage);
  return { ...result, data };
}

export async function browserRequirements(page, input, bootstrap, source, headers) {
  const mode = input.requirementsMode ?? "prepare_finalize";
  if (!["prepare_finalize", "legacy"].includes(mode)) {
    throw createWorkerError(400, "chatgpt_web_requirements_mode_invalid", "ChatGPT Web requirements mode must be prepare_finalize or legacy.");
  }
  const twoStage = mode === "prepare_finalize";
  const stage = twoStage ? "requirements_prepare" : "requirements";
  let response = await exchange(page, input, headers, twoStage ? `${requirementsPath}/prepare` : requirementsPath, { p: source }, stage);
  const prepared = token(response.data[twoStage ? "prepare_token" : "token"], stage);
  const challenge = response.data;
  if (challenge.arkose?.required === true) throw invalid("chatgpt_web_arkose_required", stage);
  const turnstileToken = token(challenge.turnstile?.token, stage, true);
  if (challenge.turnstile?.required === true && !turnstileToken) throw invalid("chatgpt_web_turnstile_required", stage);
  let proofToken = null;
  if (challenge.proofofwork?.required === true) {
    const { seed, difficulty } = challenge.proofofwork;
    if (typeof seed !== "string" || !seed || Buffer.byteLength(seed) > 1024 ||
        typeof difficulty !== "string" || !/^(?:[a-fA-F0-9]{2}){1,32}$/.test(difficulty)) {
      throw invalid("chatgpt_web_proof_material_missing", stage);
    }
    // Reuse the existing bounded handler; no new challenge algorithm is introduced here.
    proofToken = buildProofToken(bootstrap, input.userAgent, seed, difficulty);
    proofToken = token(proofToken, stage);
  }
  if (twoStage) response = await exchange(page, input, headers, `${requirementsPath}/finalize`, {
    prepare_token: prepared, proof_token: proofToken ?? "", turnstile_token: turnstileToken ?? "",
  }, "requirements_finalize");
  const finalStage = twoStage ? "requirements_finalize" : stage;
  return { token: token(response.data.token, finalStage), proofToken, turnstileToken,
    soToken: token(response.data.so_token, finalStage, true), status: response.status, contentType: response.contentType };
}
