// Experimental, one-shot HTTP/2 transport. No redirects, retries, identity spoofing or TLS bypass.
import http2 from "node:http2";
import net from "node:net";
import tls from "node:tls";
import { once } from "node:events";
import { gunzipSync } from "node:zlib";
import { uniqueJson } from "./json_wire.mjs";
import { duplexWriter } from "./duplex_writer.mjs";

export class ProbeError extends Error {
  constructor(code, status) {
    super(code);
    this.code = code;
    if (Number.isInteger(status)) this.status = status;
  }
}
const transportCodes = new Set(["ECONNRESET", "ECONNREFUSED", "ETIMEDOUT", "EPIPE",
  "ENETUNREACH", "EHOSTUNREACH", "ERR_HTTP2_ERROR", "ERR_HTTP2_STREAM_ERROR",
  "ERR_HTTP2_INVALID_SESSION", "ERR_HTTP2_GOAWAY_SESSION", "ERR_TLS_CERT_ALTNAME_INVALID",
  "CERT_HAS_EXPIRED", "UNABLE_TO_VERIFY_LEAF_SIGNATURE", "DEPTH_ZERO_SELF_SIGNED_CERT",
  "SELF_SIGNED_CERT_IN_CHAIN", "ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE",
  "ERR_SSL_TLSV1_ALERT_INTERNAL_ERROR"]);

// Do not release an operation's ownership while sibling requests still run their cleanup.
export async function settledRequests(requests) {
  const results = await Promise.allSettled(requests);
  const failed = results.find((result) => result.status === "rejected");
  if (failed) throw failed.reason;
  return results.map((result) => result.value);
}

export const endpoints = Object.freeze({
  poll: "https://api2.cursor.sh/auth/poll",
  me: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetMe",
  plan: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetPlanInfo",
  hardLimit: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetHardLimit",
  currentPeriod: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
  usage: "https://cursor.com/api/usage-summary",
  grants: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCreditGrantsBalance",
  stripe: "https://cursor.com/api/auth/stripe",
  teams: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetTeams",
  models: "https://api2.cursor.sh/agent.v1.AgentService/GetUsableModels",
  defaultModel: "https://api2.cursor.sh/agent.v1.AgentService/GetDefaultModelForCli",
  serverConfig: "https://api2.cursor.sh/aiserver.v1.ServerConfigService/GetServerConfig",
  cliModels: "https://api2.cursor.sh/aiserver.v1.AiService/GetUsableModels",
  cliDefaultModel: "https://api2.cursor.sh/aiserver.v1.AiService/GetDefaultModelForCli",
  parameterModels: "https://api2.cursor.sh/aiserver.v1.AiService/AvailableModels",
  run: "https://agentn.global.api5.cursor.sh/agent.v1.AgentService/Run",
  network: "https://api2.cursor.sh/",
});

export function validateTarget(target) {
  let url;
  try { url = new URL(target); } catch { throw new ProbeError("invalid_target"); }
  if (url.username || url.password || url.hash ||
      !Object.values(endpoints).includes(url.origin + url.pathname)) {
    throw new ProbeError("target_not_allowed");
  }
  if (url.search) {
    if (url.origin + url.pathname !== endpoints.poll ||
        [...url.searchParams.keys()].sort().join(",") !== "uuid,verifier" ||
        !/^[0-9a-f-]{36}$/.test(url.searchParams.get("uuid")) ||
        !/^[A-Za-z0-9_-]{43}$/.test(url.searchParams.get("verifier"))) {
      throw new ProbeError("invalid_poll_query");
    }
  }
  return url;
}

function proxyTunnel(socket, hostname, signal) {
  return new Promise((resolve, reject) => {
    let buffer = Buffer.alloc(0);
    const cleanup = () => {
      socket.off("data", data);
      socket.off("error", error);
      socket.off("end", ended);
      signal.removeEventListener("abort", abort);
    };
    const fail = (code) => { cleanup(); reject(new ProbeError(code)); };
    const error = () => fail("proxy_connection_failed");
    const ended = () => fail("proxy_truncated");
    const abort = () => fail("request_cancelled");
    const data = (chunk) => {
      if (buffer.length + chunk.length > 4096) return fail("proxy_headers_too_large");
      buffer = Buffer.concat([buffer, chunk]);
      const end = buffer.indexOf("\r\n\r\n");
      if (end < 0) return;
      if (!/^HTTP\/1\.[01] 200(?: |\r\n)/.test(buffer.toString("ascii")) ||
          end + 4 !== buffer.length) return fail("proxy_connect_rejected");
      cleanup();
      socket.pause();
      resolve();
    };
    socket.on("data", data);
    socket.on("error", error);
    socket.on("end", ended);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) return abort();
    // Never send bearer, cookies or query parameters to the local CONNECT proxy.
    socket.write(`CONNECT ${hostname}:443 HTTP/1.1\r\nHost: ${hostname}:443\r\n\r\n`);
  });
}

export async function request(target, options = {}) {
  const url = validateTarget(target);
  const { proxyPort, signal: external, onChunk, duplex = false } = options;
  if (!Number.isInteger(proxyPort) || proxyPort < 1 || proxyPort > 65535) {
    throw new ProbeError("explicit_loopback_proxy_required");
  }
  // Destination validation must also own HTTP routing, not just CONNECT and SNI.
  for (const key of Object.keys(options.headers ?? {})) {
    if (key.startsWith(":") || ["host", "user-agent"].includes(key.toLowerCase())) {
      throw new ProbeError("reserved_request_header");
    }
  }
  const timeoutMs = options.timeoutMs ?? 15000;
  const toolSession = duplex && typeof options.onWritable === "function";
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > (toolSession ? 300000 : 60000)) {
    throw new ProbeError("invalid_request_timeout");
  }
  const controller = new AbortController();
  const signal = controller.signal;
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const abort = () => controller.abort();
  external?.addEventListener("abort", abort, { once: true });
  if (external?.aborted) controller.abort();
  let socket, secure, session, stream, phase = "tcp_connect";
  const destroy = () => {
    stream?.destroy();
    session?.destroy();
    secure?.destroy();
    socket?.destroy();
  };
  signal.addEventListener("abort", destroy, { once: true });
  try {
    signal.throwIfAborted();
    socket = net.connect({ host: "127.0.0.1", port: proxyPort });
    // Keep late socket errors handled while TLS takes ownership.
    socket.on("error", () => {});
    await once(socket, "connect", { signal });
    phase = "proxy_connect";
    await proxyTunnel(socket, url.hostname, signal);
    phase = "tls_handshake";
    secure = tls.connect({
      socket, servername: url.hostname, ALPNProtocols: ["h2"], rejectUnauthorized: true,
    });
    secure.on("error", () => {});
    await once(secure, "secureConnect", { signal });
    if (!secure.authorized || secure.alpnProtocol !== "h2") {
      throw new ProbeError("verified_http2_required");
    }
    phase = "http2_connect";
    session = http2.connect(url.origin, { createConnection: () => secure });
    session.on("error", () => {});
    await once(session, "connect", { signal });
    await options.beforeSend?.();
    signal.throwIfAborted();
    phase = "request_headers";
    stream = session.request({
      ...options.headers,
      ":method": options.method ?? "POST",
      ":path": url.pathname + url.search,
      ":scheme": "https",
      ":authority": url.host,
      "user-agent": "Neuro-Gateway-Cursor-Experimental/0.1",
      "accept-encoding": onChunk ? "identity" : "gzip",
    });
    stream.on("error", () => {});
    options.onSent?.();
    phase = "response_stream";
    const result = await new Promise((resolve, reject) => {
      let status, contentType, contentEncoding, size = 0, ended = false;
      const chunks = [];
      const fail = (error) => reject(error instanceof ProbeError
        ? error : new ProbeError("http2_request_failed"));
      const writer = duplexWriter(stream, signal, (error) => { fail(error); stream.destroy(); });
      stream.once("close", () => writer.close());
      stream.on("response", (headers) => {
        status = headers[":status"];
        contentType = String(headers["content-type"] ?? "").split(";")[0].trim().toLowerCase();
        contentEncoding = String(headers["content-encoding"] ?? "identity").trim().toLowerCase();
        if (!["identity", "gzip"].includes(contentEncoding) || (onChunk && contentEncoding !== "identity")) {
          fail(new ProbeError("unsupported_http_content_encoding"));
          stream.destroy();
          return;
        }
        if (onChunk && status === 200 && contentType !== "application/connect+proto") {
          fail(new ProbeError("unexpected_stream_content_type"));
          stream.destroy();
        }
      });
      stream.on("error", fail);
      stream.on("aborted", () => fail(new ProbeError("http2_stream_aborted")));
      stream.on("close", () => {
        if (!ended) fail(new ProbeError("http2_stream_closed"));
      });
      stream.on("data", (chunk) => {
        size += chunk.length;
        if (size > 512 * 1024) {
          fail(new ProbeError("response_too_large"));
          return stream.destroy();
        }
        try {
          if (onChunk && status === 200) {
            const replies = onChunk(chunk);
            if (replies !== undefined) {
              if (!duplex || !Array.isArray(replies) || signal.aborted) {
                throw new ProbeError("remote_client_action_required");
              }
              for (const reply of replies) {
                writer.send(reply).catch(() => {});
              }
            }
          }
          else chunks.push(chunk);
        } catch (error) {
          fail(error);
          stream.destroy();
        }
      });
      stream.on("end", () => {
        ended = true;
        writer.close();
        if (stream.rstCode !== 0) return fail(new ProbeError("http2_reset"));
        let body = Buffer.concat(chunks);
        if (contentEncoding === "gzip") {
          try { body = gunzipSync(body, { maxOutputLength: 512 * 1024 }); }
          catch { return fail(new ProbeError("invalid_compressed_response")); }
        }
        resolve({ status, contentType, body, contentEncoding });
      });
      if (duplex) stream.write(options.body ?? Buffer.alloc(0));
      else stream.end(options.body);
      if (options.onWritable) {
        try { options.onWritable((reply) => writer.send(reply)); }
        catch (error) { fail(error); stream.destroy(); }
      }
    });
    return result;
  } catch (error) {
    const safe = signal.aborted ? new ProbeError(external?.aborted ? "cancelled" : "request_timeout")
      : error instanceof ProbeError ? error : new ProbeError("transport_failed");
    // Only local phases and known Node codes; messages, hostnames and stacks may contain secrets.
    safe.transportPhase = phase;
    safe.transportCode = transportCodes.has(error?.code) ? error.code : "other";
    throw safe;
  } finally {
    clearTimeout(timer);
    external?.removeEventListener("abort", abort);
    signal.removeEventListener("abort", destroy);
    destroy();
  }
}

export function parseJson(response) {
  if (response.status !== 200) throw new ProbeError("upstream_http_error", response.status);
  if (response.contentType !== "application/json") throw new ProbeError("unexpected_json_content_type");
  try {
    const value = uniqueJson(response.body);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("json_object_required");
    return value;
  } catch (cause) {
    const error = new ProbeError("invalid_json_response");
    const reasons = new Set(["json_size_limit", "json_depth_limit", "json_duplicate_key",
      "json_nonfinite_number", "json_integer_form", "json_object_required"]);
    // Fixed local parser labels only. Native exception text may contain upstream content.
    error.jsonReason = reasons.has(cause?.message) ? cause.message : "json_syntax_or_encoding";
    throw error;
  }
}

export const connectJsonHeaders = Object.freeze({
  accept: "application/json",
  "content-type": "application/json",
  "connect-protocol-version": "1",
});
