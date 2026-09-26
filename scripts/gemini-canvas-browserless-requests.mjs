import { collectTextResponse, collectBytesResponse, tryParseJson, redactHeaders } from "./gemini-canvas-browserless-response.mjs";

export function createBrowserlessRequestOwner({
  DEFAULT_BROWSERLESS_USER_AGENT,
  appendApiKeyQueryIfMissing,
  buildAuthAttempts,
  buildJsonHeaders,
  buildJsonHeadersWithOptions,
  buildFormHeaders,
  mergeSetCookie,
  originFromUrl,
  archiveAttempt,
  outDirFromContext,
  extractXsrfToken,
  normalizeString,
}) {
  async function sendExactMinimalApiKeyOnlyJson(context, prefix, requestUrl, requestBody, timeoutMs) {
    const url = appendApiKeyQueryIfMissing(requestUrl, context.apiKey);
    const headers = {
      Accept: "application/json",
      "Content-Type": "application/json",
      "X-Goog-AuthUser": context.session.authUser,
      "x-goog-api-key": context.apiKey,
      Origin: context.pageOrigin,
      Referer: context.pageReferer,
      "User-Agent": DEFAULT_BROWSERLESS_USER_AGENT,
    };
    const requestRecord = {
      label: "exact_minimal_api_key_only",
      kind: "api_key_only_exact",
      method: "POST",
      url,
      headers,
      headersRedacted: redactHeaders(headers),
      body: requestBody,
    };
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(requestBody),
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const responseRecord = await collectTextResponse(response);
    await archiveAttempt(outDirFromContext(context), prefix, requestRecord, responseRecord);
    return {
      ok: responseRecord.ok,
      request: requestRecord,
      response: responseRecord,
      responseJson: tryParseJson(responseRecord.text),
    };
  }

  async function sendJsonWithAuthAttempts(context, prefix, requestUrl, requestBody, timeoutMs) {
    const attempts = buildAuthAttempts(requestUrl, context.apiKey);
    const results = [];
    for (let index = 0; index < attempts.length; index += 1) {
      const attempt = attempts[index];
      const url =
        attempt.apiKeyPlacement === "query"
          ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
          : requestUrl;
      const headers = buildJsonHeaders({
        session: context.session,
        pageOrigin: context.pageOrigin,
        pageReferer: context.pageReferer,
        targetOrigin: originFromUrl(url) ?? context.pageOrigin,
        locale: context.locale,
        apiKey: context.apiKey,
        authMode: attempt,
        includeBody: true,
      });
      const requestRecord = {
        label: attempt.label,
        kind: attempt.kind,
        apiKeyPlacement: attempt.apiKeyPlacement,
        method: "POST",
        url,
        headers,
        body: requestBody,
        headersRedacted: redactHeaders(headers),
      };
      const response = await fetch(url, {
        method: "POST",
        headers,
        body: JSON.stringify(requestBody),
        redirect: "follow",
        signal: AbortSignal.timeout(timeoutMs),
      });
      const responseRecord = await collectTextResponse(response);
      context.session = mergeSetCookie(context.session, responseRecord.setCookie);
      results.push({
        request: requestRecord,
        response: responseRecord,
      });
      await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
      if (responseRecord.ok) {
        return {
          ok: true,
          url,
          attempt,
          request: requestRecord,
          response: responseRecord,
          responseJson: tryParseJson(responseRecord.text),
          attempts: results,
        };
      }
    }
    const last = results[results.length - 1];
    return {
      ok: false,
      request: last?.request ?? null,
      response: last?.response ?? null,
      responseJson: tryParseJson(last?.response?.text),
      attempts: results,
    };
  }

  async function sendJsonWithCustomAttempts(context, prefix, attempts, timeoutMs) {
    const results = [];
    for (let index = 0; index < attempts.length; index += 1) {
      const attempt = attempts[index];
      const candidateKeys = [];
      if (attempt.includeSignedHeaders && normalizeString(context.apiKey)) {
        candidateKeys.push(null, context.apiKey);
      } else if (normalizeString(context.apiKey)) {
        candidateKeys.push(context.apiKey);
      } else {
        candidateKeys.push(null);
      }
      const uniqueKeys = [];
      for (const candidateKey of candidateKeys) {
        const keyTag = candidateKey ?? "__none__";
        if (!uniqueKeys.includes(keyTag)) {
          uniqueKeys.push(keyTag);
        }
      }

      for (const candidateKeyTag of uniqueKeys) {
        const candidateKey = candidateKeyTag === "__none__" ? null : candidateKeyTag;
        const apiKeyPlacements =
          candidateKey && /generativelanguage\.googleapis\.com|clients6\.google\.com/i.test(attempt.requestUrl)
            ? ["header", "query"]
            : ["none"];
        if (candidateKey && attempt.includeApiKey !== false && !apiKeyPlacements.includes("header")) {
          apiKeyPlacements.unshift("header");
        }
        for (const apiKeyPlacement of apiKeyPlacements) {
          const url =
            candidateKey && apiKeyPlacement === "query"
              ? appendApiKeyQueryIfMissing(attempt.requestUrl, candidateKey)
              : attempt.requestUrl;
          const headers = buildJsonHeadersWithOptions({
            session: context.session,
            pageOrigin: context.pageOrigin,
            pageReferer: context.pageReferer,
            targetOrigin: originFromUrl(url) ?? context.pageOrigin,
            locale: context.locale,
            apiKey: candidateKey,
            apiKeyPlacement,
            includeSignedHeaders: attempt.includeSignedHeaders,
            preserveCrossOriginOrigin: attempt.preserveCrossOriginOrigin,
            preserveCrossOriginReferer: attempt.preserveCrossOriginReferer,
            signedOriginOverride: attempt.signedOriginOverride,
            refererOverride: attempt.refererOverride,
            includeBody: true,
          });
          const requestRecord = {
            label: attempt.label,
            kind: attempt.kind,
            requestVariant: attempt.requestVariant,
            apiKeyPlacement,
            apiKeyPresent: Boolean(candidateKey),
            method: "POST",
            url,
            headers,
            headersRedacted: redactHeaders(headers),
            body: attempt.requestBody,
          };
          const response = await fetch(url, {
            method: "POST",
            headers,
            body: JSON.stringify(attempt.requestBody),
            redirect: "follow",
            signal: AbortSignal.timeout(timeoutMs),
          });
          const responseRecord = await collectTextResponse(response);
          context.session = mergeSetCookie(context.session, responseRecord.setCookie);
          results.push({
            request: requestRecord,
            response: responseRecord,
          });
          await archiveAttempt(
            outDirFromContext(context),
            `${prefix}.attempt-${String(results.length).padStart(2, "0")}`,
            requestRecord,
            responseRecord,
          );
          const responseJson = tryParseJson(responseRecord.text);
          if (responseRecord.ok) {
            return {
              ok: true,
              request: requestRecord,
              response: responseRecord,
              responseJson,
              attempts: results,
            };
          }
        }
      }
    }
    const last = results[results.length - 1];
    return {
      ok: false,
      request: last?.request ?? null,
      response: last?.response ?? null,
      responseJson: tryParseJson(last?.response?.text),
      attempts: results,
    };
  }

  async function sendGetBytesWithAuthAttempts(context, prefix, requestUrl, timeoutMs) {
    const attempts = buildAuthAttempts(requestUrl, context.apiKey);
    const results = [];
    for (let index = 0; index < attempts.length; index += 1) {
      const attempt = attempts[index];
      const url =
        attempt.apiKeyPlacement === "query"
          ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
          : requestUrl;
      const headers = buildJsonHeaders({
        session: context.session,
        pageOrigin: context.pageOrigin,
        pageReferer: context.pageReferer,
        targetOrigin: originFromUrl(url) ?? context.pageOrigin,
        locale: context.locale,
        apiKey: context.apiKey,
        authMode: attempt,
        includeBody: false,
      });
      headers.accept = "*/*";
      delete headers["content-type"];
      const requestRecord = {
        label: attempt.label,
        kind: attempt.kind,
        apiKeyPlacement: attempt.apiKeyPlacement,
        method: "GET",
        url,
        headers,
        headersRedacted: redactHeaders(headers),
      };
      const response = await fetch(url, {
        method: "GET",
        headers,
        redirect: "follow",
        signal: AbortSignal.timeout(timeoutMs),
      });
      const responseRecord = await collectBytesResponse(response);
      context.session = mergeSetCookie(context.session, responseRecord.setCookie);
      results.push({
        request: requestRecord,
        response: responseRecord,
      });
      await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
      if (responseRecord.ok) {
        return {
          ok: true,
          url,
          attempt,
          request: requestRecord,
          response: responseRecord,
          attempts: results,
        };
      }
    }
    const last = results[results.length - 1];
    return {
      ok: false,
      request: last?.request ?? null,
      response: last?.response ?? null,
      attempts: results,
    };
  }

  async function sendFormRequest(context, prefix, requestUrl, formBody, timeoutMs, allowXsrfRetry = true) {
    let currentBody = formBody;
    let attemptIndex = 0;
    while (true) {
      attemptIndex += 1;
      const headers = buildFormHeaders({
        session: context.session,
        pageOrigin: context.pageOrigin,
        pageReferer: context.pageReferer,
        locale: context.locale,
      });
      const requestRecord = {
        method: "POST",
        url: requestUrl,
        headers,
        body: currentBody,
        headersRedacted: redactHeaders(headers),
      };
      const response = await fetch(requestUrl, {
        method: "POST",
        headers,
        body: currentBody,
        redirect: "follow",
        signal: AbortSignal.timeout(timeoutMs),
      });
      const responseRecord = await collectTextResponse(response);
      context.session = mergeSetCookie(context.session, responseRecord.setCookie);
      await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(attemptIndex).padStart(2, "0")}`, requestRecord, responseRecord);
      if (responseRecord.ok) {
        return {
          ok: true,
          request: requestRecord,
          response: responseRecord,
          responseJson: tryParseJson(responseRecord.text),
        };
      }
      if (allowXsrfRetry) {
        const xsrfToken = extractXsrfToken(responseRecord.text);
        if (xsrfToken && !currentBody.includes(`at=${encodeURIComponent(xsrfToken)}`)) {
          const params = new URLSearchParams(currentBody.endsWith("&") ? currentBody.slice(0, -1) : currentBody);
          params.set("at", xsrfToken);
          currentBody = `${params.toString()}&`;
          allowXsrfRetry = false;
          continue;
        }
      }
      return {
        ok: false,
        request: requestRecord,
        response: responseRecord,
        responseJson: tryParseJson(responseRecord.text),
      };
    }
  }

  async function sendGetJsonWithAuthAttempts(context, prefix, requestUrl, timeoutMs) {
    const attempts = buildAuthAttempts(requestUrl, context.apiKey);
    const results = [];
    for (let index = 0; index < attempts.length; index += 1) {
      const attempt = attempts[index];
      const url =
        attempt.apiKeyPlacement === "query"
          ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
          : requestUrl;
      const headers = buildJsonHeaders({
        session: context.session,
        pageOrigin: context.pageOrigin,
        pageReferer: context.pageReferer,
        targetOrigin: originFromUrl(url) ?? context.pageOrigin,
        locale: context.locale,
        apiKey: context.apiKey,
        authMode: attempt,
        includeBody: false,
      });
      headers.accept = "application/json";
      delete headers["content-type"];
      const requestRecord = {
        label: attempt.label,
        kind: attempt.kind,
        apiKeyPlacement: attempt.apiKeyPlacement,
        method: "GET",
        url,
        headers,
        headersRedacted: redactHeaders(headers),
      };
      const response = await fetch(url, {
        method: "GET",
        headers,
        redirect: "follow",
        signal: AbortSignal.timeout(timeoutMs),
      });
      const responseRecord = await collectTextResponse(response);
      context.session = mergeSetCookie(context.session, responseRecord.setCookie);
      results.push({
        request: requestRecord,
        response: responseRecord,
      });
      await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
      if (responseRecord.ok) {
        return {
          ok: true,
          request: requestRecord,
          response: responseRecord,
          responseJson: tryParseJson(responseRecord.text),
          attempts: results,
        };
      }
    }
    const last = results[results.length - 1];
    return {
      ok: false,
      request: last?.request ?? null,
      response: last?.response ?? null,
      responseJson: tryParseJson(last?.response?.text),
      attempts: results,
    };
  }

  return {
    sendExactMinimalApiKeyOnlyJson,
    sendJsonWithAuthAttempts,
    sendJsonWithCustomAttempts,
    sendGetBytesWithAuthAttempts,
    sendFormRequest,
    sendGetJsonWithAuthAttempts,
  };
}
