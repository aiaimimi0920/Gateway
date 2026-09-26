import crypto from "node:crypto";

export function createBrowserlessAuthOwner({
  DEFAULT_AUTH_USER,
  DEFAULT_BROWSERLESS_USER_AGENT,
  normalizeString,
}) {
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

  function buildPureHttpSession(storageState, requestUrl, baseUrl, authUser = DEFAULT_AUTH_USER) {
    const targetUrl = new URL(requestUrl);
    const base = new URL(baseUrl);
    const cookies = Array.isArray(storageState?.cookies) ? storageState.cookies : [];
    const matched = cookies
      .filter((cookie) => cookieMatchesUrl(cookie, targetUrl) || cookieMatchesUrl(cookie, base))
      .map((cookie, index) => ({
        name: String(cookie.name || "").trim(),
        value: String(cookie.value || "").trim(),
        path: String(cookie.path || "/").trim() || "/",
        domain: String(cookie.domain || "").trim(),
        index,
      }))
      .filter((cookie) => cookie.name && cookie.value);

    matched.sort((left, right) => {
      if (right.path.length !== left.path.length) {
        return right.path.length - left.path.length;
      }
      return left.index - right.index;
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
      authUser: String(authUser || DEFAULT_AUTH_USER).trim() || DEFAULT_AUTH_USER,
    };
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
      const lines = String(setCookie || "")
        .split(/\r?\n/)
        .map((value) => value.trim())
        .filter(Boolean);
      for (const line of lines) {
        const cookiePair = line.split(";", 1)[0];
        const eqIndex = cookiePair.indexOf("=");
        if (eqIndex <= 0) {
          continue;
        }
        const name = cookiePair.slice(0, eqIndex).trim();
        const value = cookiePair.slice(eqIndex + 1).trim();
        if (!name) {
          continue;
        }
        parsed.set(name, value);
        if (["__Secure-1PAPISID", "__Secure-3PAPISID", "SAPISID"].includes(name) && value) {
          nextSapisid = value;
        }
      }
    }
    return {
      ...session,
      sapisid: nextSapisid,
      cookieHeader: Array.from(parsed.entries())
        .map(([name, value]) => `${name}=${value}`)
        .join("; "),
    };
  }

  function buildSapisidAuthorization(sapisid, origin, timestampSecs = null) {
    const ts = Number.isFinite(timestampSecs) ? Number(timestampSecs) : Math.floor(Date.now() / 1000);
    const digest = crypto
      .createHash("sha1")
      .update(`${ts} ${sapisid} ${origin}`)
      .digest("hex");
    return `SAPISIDHASH ${ts}_${digest} SAPISID1PHASH ${ts}_${digest} SAPISID3PHASH ${ts}_${digest}`;
  }

  function buildBrowserClientHints(headers) {
    headers["sec-ch-ua"] =
      headers["sec-ch-ua"] ??
      "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not A(Brand\";v=\"24\"";
    headers["sec-ch-ua-mobile"] = headers["sec-ch-ua-mobile"] ?? "?0";
    headers["sec-ch-ua-platform"] = headers["sec-ch-ua-platform"] ?? "\"Windows\"";
    headers["sec-ch-ua-platform-version"] =
      headers["sec-ch-ua-platform-version"] ?? "\"10.0.0\"";
    headers["sec-ch-ua-arch"] = headers["sec-ch-ua-arch"] ?? "\"x86\"";
    headers["sec-ch-ua-bitness"] = headers["sec-ch-ua-bitness"] ?? "\"64\"";
    headers["sec-ch-ua-model"] = headers["sec-ch-ua-model"] ?? "\"\"";
    headers["sec-ch-ua-wow64"] = headers["sec-ch-ua-wow64"] ?? "?0";
    headers["sec-ch-ua-form-factors"] =
      headers["sec-ch-ua-form-factors"] ?? "\"Desktop\"";
    headers["sec-fetch-dest"] = headers["sec-fetch-dest"] ?? "empty";
    headers["sec-fetch-mode"] = headers["sec-fetch-mode"] ?? "cors";
    headers["sec-fetch-site"] = headers["sec-fetch-site"] ?? "cross-site";
  }

  function buildJsonHeaders({
    session,
    pageOrigin,
    pageReferer,
    targetOrigin,
    locale,
    apiKey,
    authMode,
    includeBody = true,
  }) {
    if (authMode.kind === "api_key_only") {
      const headers = {
        accept: "application/json",
        "user-agent": DEFAULT_BROWSERLESS_USER_AGENT,
        origin: pageOrigin,
        referer: pageReferer,
        "x-goog-authuser": session.authUser,
      };
      if (includeBody) {
        headers["content-type"] = "application/json";
      }
      if (apiKey && authMode.apiKeyPlacement !== "none") {
        headers["x-goog-api-key"] = apiKey;
      }
      return headers;
    }

    const headers = {
      accept: "application/json",
      "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
      "user-agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
      origin: pageOrigin,
      referer: pageReferer,
      "x-same-domain": "1",
    };
    if (includeBody) {
      headers["content-type"] = "application/json";
    }
    if (apiKey && authMode.apiKeyPlacement !== "none") {
      headers["x-goog-api-key"] = apiKey;
    }

    buildBrowserClientHints(headers);
    headers["sec-fetch-site"] =
      String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase()
        ? "same-origin"
        : "cross-site";

    const authorization = buildSapisidAuthorization(session.sapisid, pageOrigin);
    headers.cookie = session.cookieHeader;
    headers.authorization = authorization;
    headers["x-origin"] = pageOrigin;
    headers["x-goog-authuser"] = session.authUser;
    return headers;
  }

  function buildJsonHeadersWithOptions({
    session,
    pageOrigin,
    pageReferer,
    targetOrigin,
    locale,
    apiKey,
    apiKeyPlacement = "none",
    includeSignedHeaders = true,
    preserveCrossOriginOrigin = true,
    preserveCrossOriginReferer = true,
    signedOriginOverride = null,
    refererOverride = null,
    includeBody = true,
  }) {
    const referer = normalizeString(refererOverride) ?? pageReferer;
    const signedOrigin = normalizeString(signedOriginOverride) ?? pageOrigin;
    if (!includeSignedHeaders) {
      const headers = {
        accept: "application/json",
        "user-agent": DEFAULT_BROWSERLESS_USER_AGENT,
      };
      if (includeBody) {
        headers["content-type"] = "application/json";
      }
      if (apiKey && apiKeyPlacement !== "none") {
        headers["x-goog-api-key"] = apiKey;
        headers["x-goog-authuser"] = session.authUser;
      }
      const includePageContext =
        String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase() ||
        preserveCrossOriginOrigin;
      if (includePageContext) {
        headers.origin = pageOrigin;
        headers.referer = referer;
      } else if (preserveCrossOriginReferer) {
        headers.referer = referer;
      }
      return headers;
    }

    const headers = {
      accept: "application/json",
      "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
      "user-agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
      "x-same-domain": "1",
    };
    if (includeBody) {
      headers["content-type"] = "application/json";
    }
    if (apiKey && apiKeyPlacement !== "none") {
      headers["x-goog-api-key"] = apiKey;
    }
    const includePageContext =
      String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase() ||
      preserveCrossOriginOrigin;
    if (includePageContext) {
      headers.origin = pageOrigin;
      headers.referer = referer;
    }

    buildBrowserClientHints(headers);
    headers["sec-fetch-site"] =
      String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase()
        ? "same-origin"
        : "cross-site";

    headers.cookie = session.cookieHeader;
    headers.authorization = buildSapisidAuthorization(session.sapisid, signedOrigin);
    headers["x-origin"] = signedOrigin;
    headers["x-goog-authuser"] = session.authUser;
    headers.referer = referer;

    if (!includePageContext) {
      delete headers.origin;
      if (preserveCrossOriginReferer) {
        headers.referer = referer;
      } else {
        delete headers.referer;
      }
    }

    return headers;
  }

  function buildFormHeaders({
    session,
    pageOrigin,
    pageReferer,
    locale,
  }) {
    const headers = {
      accept: "*/*",
      "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
      "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
      "user-agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
      origin: pageOrigin,
      referer: pageReferer,
      "x-same-domain": "1",
      cookie: session.cookieHeader,
      authorization: buildSapisidAuthorization(session.sapisid, pageOrigin),
      "x-origin": pageOrigin,
      "x-goog-authuser": session.authUser,
    };
    buildBrowserClientHints(headers);
    headers["sec-fetch-site"] = "same-origin";
    return headers;
  }

  function appendApiKeyQueryIfMissing(requestUrl, apiKey) {
    const trimmedKey = normalizeString(apiKey);
    if (!trimmedKey) {
      return requestUrl;
    }
    const parsed = new URL(requestUrl);
    const existingKey = parsed.searchParams.get("key");
    if (!existingKey) {
      parsed.searchParams.set("key", trimmedKey);
    }
    return parsed.toString();
  }

  function buildAuthAttempts(requestUrl, apiKey) {
    const parsed = new URL(requestUrl);
    const host = parsed.host.toLowerCase();
    const pathname = parsed.pathname.toLowerCase();
    const isApiHost =
      host.includes("generativelanguage.googleapis.com") || host.includes("clients6.google.com");
    if (apiKey && isApiHost) {
      const preferQueryFirst =
        pathname.includes(":predict") ||
        pathname.includes("generatecontent") && pathname.includes("image");
      if (preferQueryFirst) {
        return [
          { label: "api_key_only_query", kind: "api_key_only", apiKeyPlacement: "query" },
          { label: "api_key_only_header", kind: "api_key_only", apiKeyPlacement: "header" },
          { label: "signed_session_query", kind: "signed_session", apiKeyPlacement: "query" },
          { label: "signed_session_header", kind: "signed_session", apiKeyPlacement: "header" },
        ];
      }
      return [
        { label: "api_key_only_header", kind: "api_key_only", apiKeyPlacement: "header" },
        { label: "signed_session_header", kind: "signed_session", apiKeyPlacement: "header" },
        { label: "signed_session_query", kind: "signed_session", apiKeyPlacement: "query" },
      ];
    }
    if (apiKey && host.includes("googleusercontent.com")) {
      return [{ label: "signed_session_download", kind: "signed_session", apiKeyPlacement: "none" }];
    }
    return [{ label: "signed_session", kind: "signed_session", apiKeyPlacement: "none" }];
  }

  return {
    cookieMatchesUrl,
    buildPureHttpSession,
    mergeSetCookie,
    buildSapisidAuthorization,
    buildBrowserClientHints,
    buildJsonHeaders,
    buildJsonHeadersWithOptions,
    buildFormHeaders,
    appendApiKeyQueryIfMissing,
    buildAuthAttempts,
  };
}
