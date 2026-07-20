#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveGeminiCanvasHostExportStorageStatePath } from "./gemini-canvas-runtime-paths.mjs";

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const entry = argv[i];
    if (!entry.startsWith("--")) {
      continue;
    }
    const key = entry.slice(2);
    const next = argv[i + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
      continue;
    }
    args[key] = next;
    i += 1;
  }
  return args;
}

function nowStamp() {
  return new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
}

function ensureDir(dir) {
  return fs.mkdir(dir, { recursive: true });
}

async function readJson(filePath) {
  const raw = await fs.readFile(filePath, "utf8");
  return JSON.parse(raw);
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeText(filePath, value) {
  await fs.writeFile(filePath, value, "utf8");
}

function extractBootstrapString(html, key) {
  const patterns = [
    new RegExp(`"${key}"\\s*:\\s*"([^"]*)"`, "i"),
    new RegExp(`\\['${key}'\\]\\s*=\\s*"([^"]*)"`, "i"),
    new RegExp(`${key}\\\\u0022:\\\\u0022([^\\\\]+)`, "i"),
  ];
  for (const pattern of patterns) {
    const match = html.match(pattern);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

function normalizeAppPath(candidate) {
  if (!candidate || typeof candidate !== "string") {
    return null;
  }
  const trimmed = candidate.trim();
  if (!trimmed) {
    return null;
  }
  try {
    const url = new URL(trimmed);
    return normalizeAppPath(url.pathname);
  } catch {
    // ignore
  }
  const withoutQuery = trimmed.split(/[?#]/, 1)[0].trim();
  if (!withoutQuery) {
    return null;
  }
  if (withoutQuery === "/app") {
    return "/app";
  }
  if (withoutQuery.startsWith("/app/")) {
    return withoutQuery.replace(/\/+$/, "");
  }
  if (/^[A-Za-z0-9_-]+$/.test(withoutQuery)) {
    return `/app/${withoutQuery}`;
  }
  return null;
}

function extractAppPagePathFromHtml(html) {
  for (const key of ["appPagePath", "app_page_path", "pagePath", "page_path", "pageId", "page_id"]) {
    const value = extractBootstrapString(html, key);
    const normalized = normalizeAppPath(value);
    if (normalized) {
      return normalized;
    }
  }
  return null;
}

function parseBootstrapFromHtml(html, fallbackLanguage = "zh-CN") {
  return {
    accessToken: extractBootstrapString(html, "SNlM0e"),
    buildLabel: extractBootstrapString(html, "cfb2h"),
    sessionId: extractBootstrapString(html, "FdrFJe"),
    language: extractBootstrapString(html, "TuX5cc") || fallbackLanguage,
    pushId: extractBootstrapString(html, "qKIAYe"),
    clientPctx: extractBootstrapString(html, "Ylro7b"),
    appPagePath: extractAppPagePathFromHtml(html),
  };
}

function cookieMatchesUrl(cookie, url) {
  const hostname = url.hostname.toLowerCase();
  const pathname = url.pathname || "/";
  const cookieDomain = String(cookie.domain || "").trim().toLowerCase();
  if (!cookieDomain) {
    return false;
  }
  const domain = cookieDomain.startsWith(".") ? cookieDomain.slice(1) : cookieDomain;
  const domainMatch = hostname === domain || hostname.endsWith(`.${domain}`);
  if (!domainMatch) {
    return false;
  }
  const cookiePath = String(cookie.path || "/").trim() || "/";
  return pathname.startsWith(cookiePath);
}

function buildPureHttpSession(storageState, targetUrl, baseUrl, authUser = "0") {
  const target = new URL(targetUrl);
  const base = new URL(baseUrl);
  const cookies = Array.isArray(storageState.cookies) ? storageState.cookies : [];
  const matched = cookies
    .filter((cookie) => cookieMatchesUrl(cookie, target) || cookieMatchesUrl(cookie, base))
    .map((cookie, index) => ({
      name: String(cookie.name || "").trim(),
      value: String(cookie.value || "").trim(),
      path: String(cookie.path || "/").trim() || "/",
      domain: String(cookie.domain || "").trim(),
      index,
    }))
    .filter((cookie) => cookie.name && cookie.value);

  matched.sort((a, b) => {
    if (b.path.length !== a.path.length) {
      return b.path.length - a.path.length;
    }
    return a.index - b.index;
  });

  const sapisidCookie = matched.find((cookie) =>
    ["__Secure-1PAPISID", "__Secure-3PAPISID", "SAPISID"].includes(cookie.name),
  );
  if (!sapisidCookie) {
    throw new Error("missing SAPISID-compatible cookie in storage-state");
  }
  if (matched.length === 0) {
    throw new Error("no usable Google cookies matched the target URL");
  }
  const cookieHeader = matched.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  return {
    cookieHeader,
    sapisid: sapisidCookie.value,
    authUser: String(authUser || "0").trim() || "0",
  };
}

function buildSapisidAuthorization(sapisid, origin) {
  const timestampSecs = Math.floor(Date.now() / 1000);
  const digest = crypto
    .createHash("sha1")
    .update(`${timestampSecs} ${sapisid} ${origin}`)
    .digest("hex");
  const hash = `SAPISIDHASH ${timestampSecs}_${digest} SAPISID1PHASH ${timestampSecs}_${digest} SAPISID3PHASH ${timestampSecs}_${digest}`;
  return { timestampSecs, value: hash };
}

function buildBrowserLikeHeaders(session, origin, referer, locale, authorization, contentType) {
  const headers = {
    "accept": "*/*",
    "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
    "authorization": authorization,
    "content-type": contentType,
    "cookie": session.cookieHeader,
    "origin": origin,
    "referer": referer,
    "sec-ch-ua": "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not A(Brand\";v=\"24\"",
    "sec-ch-ua-arch": "\"x86\"",
    "sec-ch-ua-bitness": "\"64\"",
    "sec-ch-ua-form-factors": "\"Desktop\"",
    "sec-ch-ua-full-version": "\"143.0.0.0\"",
    "sec-ch-ua-full-version-list": "\"Microsoft Edge\";v=\"143.0.0.0\", \"Chromium\";v=\"143.0.0.0\", \"Not A(Brand\";v=\"24.0.0.0\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-model": "\"\"",
    "sec-ch-ua-platform": "\"Windows\"",
    "sec-ch-ua-platform-version": "\"10.0.0\"",
    "sec-ch-ua-wow64": "?0",
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "cors",
    "sec-fetch-site": "same-origin",
    "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    "x-goog-authuser": session.authUser,
    "x-origin": origin,
    "x-same-domain": "1",
  };
  return headers;
}

function mergeSetCookie(session, setCookieValues) {
  if (!setCookieValues?.length) {
    return session;
  }
  const parsed = new Map();
  for (const segment of String(session.cookieHeader || "").split(";")) {
    const [name, ...rest] = segment.split("=");
    const trimmedName = name?.trim();
    if (!trimmedName) {
      continue;
    }
    parsed.set(trimmedName, rest.join("=").trim());
  }
  let nextSapisid = session.sapisid;
  for (const setCookie of setCookieValues) {
    const first = String(setCookie).split(";", 1)[0];
    const eqIndex = first.indexOf("=");
    if (eqIndex <= 0) {
      continue;
    }
    const name = first.slice(0, eqIndex).trim();
    const value = first.slice(eqIndex + 1).trim();
    if (!name) {
      continue;
    }
    parsed.set(name, value);
    if (["__Secure-1PAPISID", "__Secure-3PAPISID", "SAPISID"].includes(name) && value) {
      nextSapisid = value;
    }
  }
  return {
    ...session,
    sapisid: nextSapisid,
    cookieHeader: Array.from(parsed.entries()).map(([name, value]) => `${name}=${value}`).join("; "),
  };
}

async function fetchText(url, options) {
  const response = await fetch(url, options);
  const text = await response.text();
  const headers = {};
  response.headers.forEach((value, name) => {
    headers[name] = value;
  });
  return {
    status: response.status,
    ok: response.ok,
    url: response.url,
    headers,
    text,
    setCookie: response.headers.getSetCookie ? response.headers.getSetCookie() : [],
  };
}

function parseUrlEncoded(body) {
  const params = new URLSearchParams(body);
  return Array.from(params.entries());
}

function serializeUrlEncoded(entries) {
  const params = new URLSearchParams();
  for (const [key, value] of entries) {
    params.append(key, value);
  }
  return params.toString();
}

function refreshRequestUrl(originalUrl, bootstrap, reqid) {
  const url = new URL(originalUrl);
  if (bootstrap.buildLabel) {
    url.searchParams.set("bl", bootstrap.buildLabel);
  }
  if (bootstrap.sessionId) {
    url.searchParams.set("f.sid", bootstrap.sessionId);
  }
  if (bootstrap.language) {
    url.searchParams.set("hl", bootstrap.language);
  }
  url.searchParams.set("_reqid", String(reqid));
  url.searchParams.set("rt", "c");
  return url.toString();
}

function refreshRequestBody(postData, bootstrap) {
  const entries = parseUrlEncoded(postData);
  const refreshed = entries.filter(([key]) => key !== "at");
  if (bootstrap.accessToken) {
    refreshed.push(["at", bootstrap.accessToken]);
  }
  return serializeUrlEncoded(refreshed);
}

function extractXsrfToken(bodyText) {
  const patterns = [
    /\["xsrf","([^"]+)"\]/i,
    /"xsrf"\s*,\s*"([^"]+)"/i,
  ];
  for (const pattern of patterns) {
    const match = bodyText.match(pattern);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

function extractImageSignals(bodyText) {
  const urls = Array.from(bodyText.matchAll(/https:\/\/lh3\.googleusercontent\.com\/gg-dl\/[^"\\\s]+/g)).map((match) => match[0]);
  const contentSignals = Array.from(bodyText.matchAll(/http:\/\/googleusercontent\.com\/image_generation_content\/\d+/g)).map((match) => match[0]);
  const conversationIds = Array.from(bodyText.matchAll(/c_[0-9a-f]{16}/g)).map((match) => match[0]);
  const responseIds = Array.from(bodyText.matchAll(/r_[0-9a-f]{16}/g)).map((match) => match[0]);
  return {
    imageUrls: Array.from(new Set(urls)),
    contentSignals: Array.from(new Set(contentSignals)),
    conversationIds: Array.from(new Set(conversationIds)),
    responseIds: Array.from(new Set(responseIds)),
  };
}

function minimalRequestSequence(captureJson) {
  const source = Array.isArray(captureJson.rpcCaptures)
    ? captureJson.rpcCaptures
    : Array.isArray(captureJson.result?.rpcCaptures)
      ? captureJson.result.rpcCaptures
      : [];
  const wanted = ["ESY5D", "L5adhe", "XhaU0b", "StreamGenerate"];
  const requests = source.filter((entry) => entry.type === "request");
  const seen = new Set();
  const filtered = [];
  for (const entry of requests) {
    if (!wanted.includes(entry.label) || seen.has(entry.label)) {
      continue;
    }
    seen.add(entry.label);
    filtered.push(entry);
    if (filtered.length === wanted.length) {
      break;
    }
  }
  return filtered;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const capturePath = path.resolve(repoRoot, args.capture || ".runtime/canvas-program-rpc-capture-curl-postpatch2/response.json");
  const requestPath = path.resolve(repoRoot, args.request || ".runtime/canvas-program-rpc-image-request.json");
  const runtimePath = path.resolve(
    repoRoot,
    args.runtime || resolveGeminiCanvasHostExportStorageStatePath(),
  );
  const outDir = path.resolve(
    repoRoot,
    args.outDir || path.join(".runtime", "gemini-canvas-program-pure-http-probe", nowStamp()),
  );
  await ensureDir(outDir);

  const capture = await readJson(capturePath);
  const requestJson = await readJson(requestPath);
  const storageState = await readJson(runtimePath);
  const baseUrl = String(requestJson.baseUrl || "https://gemini.google.com").trim().replace(/\/+$/, "");
  const baseOrigin = new URL(baseUrl).origin;
  const programUrl = String(requestJson.canvasProgramUrl || `${baseUrl}/app`).trim();
  const authUser = String(requestJson.authUser || "0");
  let session = buildPureHttpSession(storageState, programUrl, baseUrl, authUser);
  await writeJson(path.join(outDir, "session.initial.json"), {
    authUser: session.authUser,
    sapisid: session.sapisid,
    cookieHeaderLength: session.cookieHeader.length,
  });

  const pageFetches = [];
  let bootstrap = null;
  for (const url of [programUrl, `${baseUrl}/app`]) {
    const response = await fetchText(url, {
      method: "GET",
      headers: {
        "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8",
        "cookie": session.cookieHeader,
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
      },
      redirect: "follow",
    });
    session = mergeSetCookie(session, response.setCookie);
    const parsed = parseBootstrapFromHtml(response.text, requestJson.locale || "zh-CN");
    pageFetches.push({
      url,
      finalUrl: response.url,
      status: response.status,
      parsedBootstrap: parsed,
      contentType: response.headers["content-type"] || null,
    });
    const fileSafe = url.endsWith("/app") ? "app-bootstrap.html" : "program-bootstrap.html";
    await writeText(path.join(outDir, fileSafe), response.text);
    if (parsed.accessToken || parsed.buildLabel || parsed.sessionId) {
      bootstrap = parsed;
      break;
    }
  }
  if (!bootstrap) {
    bootstrap = pageFetches.at(-1)?.parsedBootstrap || {};
  }
  await writeJson(path.join(outDir, "bootstrap.fetches.json"), pageFetches);
  await writeJson(path.join(outDir, "bootstrap.selected.json"), bootstrap);

  const sequence = minimalRequestSequence(capture);
  const reqidBase = 2900000 + Math.floor(Math.random() * 100000);
  const executed = [];
  let streamGenerateSignals = null;

  for (let index = 0; index < sequence.length; index += 1) {
    const step = sequence[index];
    const reqid = reqidBase + (index * 100000);
    const requestUrl = refreshRequestUrl(step.url, bootstrap, reqid);
    let requestBody = refreshRequestBody(step.bodyText || step.postData || "", bootstrap);
    const referer = step.headers?.referer || `${baseUrl}/`;
    let authorization = buildSapisidAuthorization(session.sapisid, baseOrigin);
    let response = await fetchText(requestUrl, {
      method: "POST",
      headers: buildBrowserLikeHeaders(
        session,
        baseOrigin,
        referer,
        bootstrap.language || requestJson.locale || "zh-CN",
        authorization.value,
        "application/x-www-form-urlencoded;charset=UTF-8",
      ),
      body: requestBody,
      redirect: "follow",
    });

    if (response.status === 400) {
      const xsrf = extractXsrfToken(response.text);
      if (xsrf) {
        bootstrap.accessToken = xsrf;
        requestBody = refreshRequestBody(step.bodyText || step.postData || "", bootstrap);
        authorization = buildSapisidAuthorization(session.sapisid, baseOrigin);
        response = await fetchText(requestUrl, {
          method: "POST",
          headers: buildBrowserLikeHeaders(
            session,
            baseOrigin,
            referer,
            bootstrap.language || requestJson.locale || "zh-CN",
            authorization.value,
            "application/x-www-form-urlencoded;charset=UTF-8",
          ),
          body: requestBody,
          redirect: "follow",
        });
      }
    }

    session = mergeSetCookie(session, response.setCookie);
    const stepDir = path.join(outDir, `${String(index + 1).padStart(2, "0")}-${step.label}`);
    await ensureDir(stepDir);
    await writeJson(path.join(stepDir, "request.json"), {
      label: step.label,
      requestUrl,
      headers: buildBrowserLikeHeaders(
        session,
        baseOrigin,
        referer,
        bootstrap.language || requestJson.locale || "zh-CN",
        authorization.value,
        "application/x-www-form-urlencoded;charset=UTF-8",
      ),
      body: requestBody,
    });
    await writeText(path.join(stepDir, "response.body.txt"), response.text);
    await writeJson(path.join(stepDir, "response.meta.json"), {
      status: response.status,
      finalUrl: response.url,
      headers: response.headers,
    });

    const signals = extractImageSignals(response.text);
    if (step.label === "StreamGenerate") {
      streamGenerateSignals = signals;
    }
    executed.push({
      label: step.label,
      requestUrl,
      status: response.status,
      finalUrl: response.url,
      xsrfRetryApplied: response.status !== 400 && bootstrap.accessToken !== null,
      signals,
    });
  }

  const summary = {
    ok: Boolean(streamGenerateSignals?.imageUrls?.length || streamGenerateSignals?.contentSignals?.length),
    capturePath,
    requestPath,
    runtimePath,
    outDir,
    baseUrl,
    programUrl,
    bootstrap,
    executed,
    streamGenerateSignals,
    note: "This probe replays the third-line program-owned success sample via direct HTTP only. It does not invoke the browser pool.",
  };
  await writeJson(path.join(outDir, "summary.json"), summary);
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error?.stack || error}\n`);
  process.exitCode = 1;
});
