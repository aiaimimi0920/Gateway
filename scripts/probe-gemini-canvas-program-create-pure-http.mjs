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

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeText(filePath, value) {
  await fs.writeFile(filePath, value, "utf8");
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
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

function parseBootstrapFromHtml(html, fallbackLanguage = "zh-CN") {
  return {
    accessToken: extractBootstrapString(html, "SNlM0e"),
    buildLabel: extractBootstrapString(html, "cfb2h"),
    sessionId: extractBootstrapString(html, "FdrFJe"),
    language: extractBootstrapString(html, "TuX5cc") || fallbackLanguage,
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
  return {
    cookieHeader: matched.map((cookie) => `${cookie.name}=${cookie.value}`).join("; "),
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
  return `SAPISIDHASH ${timestampSecs}_${digest} SAPISID1PHASH ${timestampSecs}_${digest} SAPISID3PHASH ${timestampSecs}_${digest}`;
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

function buildBrowserLikeHeaders(session, origin, referer, locale, authorization) {
  return {
    "accept": "*/*",
    "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
    "authorization": authorization,
    "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
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
}

function extractXsrfToken(bodyText) {
  const patterns = [
    /\["xsrf","([^"]+)"\]/i,
    /"xsrf"\s*,\s*"([^"]+)"/i,
  ];
  for (const pattern of patterns) {
    const match = String(bodyText || "").match(pattern);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

function uniqueStrings(values) {
  return Array.from(new Set((values || []).map((value) => String(value || "").trim()).filter(Boolean)));
}

function extractAll(pattern, text) {
  return uniqueStrings(Array.from(String(text || "").matchAll(pattern)).map((match) => match[0]));
}

function deriveProgramHandle(bodyText, baseUrl) {
  const appPaths = extractAll(/\/app\/[0-9a-f]{16}/gi, bodyText);
  const conversationIds = extractAll(/\bc_[0-9a-f]{16}\b/gi, bodyText);
  const responseIds = extractAll(/\br_[0-9a-f]{16}\b/gi, bodyText);
  const hasCanvasProxyClient = /Browser API Proxy Client/i.test(bodyText);

  let appPath = appPaths[0] ?? null;
  const matchedConversation = conversationIds
    .map((value) => value.replace(/^c_/i, ""))
    .find((suffix) => appPaths.includes(`/app/${suffix}`));
  if (matchedConversation) {
    appPath = `/app/${matchedConversation}`;
  }

  const conversationId =
    matchedConversation ? `c_${matchedConversation}` : conversationIds[0] ?? null;
  if (!appPath && conversationId) {
    appPath = `/app/${conversationId.replace(/^c_/i, "")}`;
  }
  const responseId = responseIds[0] ?? null;
  const programUrl = appPath ? `${baseUrl.replace(/\/+$/, "")}${appPath}` : null;

  return {
    appPaths,
    conversationIds,
    responseIds,
    appPath,
    programUrl,
    conversationId,
    responseId,
    sourceSurface: hasCanvasProxyClient ? "canvas_proxy_client" : null,
  };
}

function buildUjx1BfUrl(baseUrl, shareId, bootstrap, reqid) {
  const url = new URL(`${baseUrl.replace(/\/+$/, "")}/_/BardChatUi/data/batchexecute`);
  url.searchParams.set("rpcids", "ujx1Bf");
  url.searchParams.set("source-path", `/share/${shareId}`);
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

function buildUjx1BfBody(shareId, accessToken) {
  const inner = `[[["ujx1Bf","[null,\\\"${shareId}\\\",[4]]",null,"generic"]]]`;
  const params = new URLSearchParams();
  params.set("f.req", inner);
  if (normalizeString(accessToken)) {
    params.set("at", accessToken);
  }
  return `${params.toString()}&`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const shareId = normalizeString(args.shareId || process.env.GEMINI_CANVAS_SHARE_ID || "fe24c455a570");
  const baseUrl = normalizeString(args.baseUrl || process.env.GEMINI_CANVAS_BASE_URL || "https://gemini.google.com");
  const shareUrl = normalizeString(args.shareUrl || process.env.GEMINI_CANVAS_SHARE_URL || `${baseUrl}/share/${shareId}`);
  const runtimePath = path.resolve(
    repoRoot,
    args.runtime || resolveGeminiCanvasHostExportStorageStatePath(),
  );
  const outDir = path.resolve(
    repoRoot,
    args.outDir || path.join("output", `gemini_canvas_program_create_pure_http_${nowStamp()}`),
  );
  await ensureDir(outDir);

  const storageState = await readJson(runtimePath);
  let session = buildPureHttpSession(storageState, shareUrl, baseUrl, args.authUser || "0");
  const origin = new URL(baseUrl).origin;

  const shareResponse = await fetchText(shareUrl, {
    method: "GET",
    headers: {
      "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      "accept-language": "zh-CN,zh;q=0.9,en;q=0.8",
      "cookie": session.cookieHeader,
      "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    },
    redirect: "follow",
  });
  session = mergeSetCookie(session, shareResponse.setCookie);
  await writeText(path.join(outDir, "share.html"), shareResponse.text);
  const bootstrap = parseBootstrapFromHtml(shareResponse.text, "zh-CN");
  await writeJson(path.join(outDir, "bootstrap.json"), bootstrap);

  const reqid = 360000 + Math.floor(Math.random() * 100000);
  const requestUrl = buildUjx1BfUrl(baseUrl, shareId, bootstrap, reqid);
  let requestBody = buildUjx1BfBody(shareId, bootstrap.accessToken);
  let authorization = buildSapisidAuthorization(session.sapisid, origin);
  let response = await fetchText(requestUrl, {
    method: "POST",
    headers: buildBrowserLikeHeaders(
      session,
      origin,
      shareUrl,
      bootstrap.language || "zh-CN",
      authorization,
    ),
    body: requestBody,
    redirect: "follow",
  });

  if (response.status === 400) {
    const xsrf = extractXsrfToken(response.text);
    if (xsrf) {
      bootstrap.accessToken = xsrf;
      requestBody = buildUjx1BfBody(shareId, bootstrap.accessToken);
      authorization = buildSapisidAuthorization(session.sapisid, origin);
      response = await fetchText(requestUrl, {
        method: "POST",
        headers: buildBrowserLikeHeaders(
          session,
          origin,
          shareUrl,
          bootstrap.language || "zh-CN",
          authorization,
        ),
        body: requestBody,
        redirect: "follow",
      });
    }
  }

  session = mergeSetCookie(session, response.setCookie);
  await writeJson(path.join(outDir, "request.json"), {
    url: requestUrl,
    body: requestBody,
    headers: buildBrowserLikeHeaders(
      session,
      origin,
      shareUrl,
      bootstrap.language || "zh-CN",
      authorization,
    ),
  });
  await writeText(path.join(outDir, "response.body.txt"), response.text);
  await writeJson(path.join(outDir, "response.meta.json"), {
    status: response.status,
    finalUrl: response.url,
    headers: response.headers,
  });

  const handle = deriveProgramHandle(response.text, baseUrl);
  const summary = {
    ok: response.status === 200 && Boolean(handle.programUrl && handle.conversationId),
    mode: "create_canvas_app_pure_http",
    baseUrl,
    shareId,
    shareUrl,
    runtimePath,
    outDir,
    bootstrap,
    requestUrl,
    status: response.status,
    finalUrl: response.url,
    handle,
    note: "This probe only verifies share -> ujx1Bf -> concrete canvas app handle over pure HTTP. It does not invoke the later websocket execution lane.",
  };
  await writeJson(path.join(outDir, "summary.json"), summary);
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error?.stack || error}\n`);
  process.exitCode = 1;
});
