function responseHeadersToObject(response) {
  const headers = {};
  response.headers.forEach((value, name) => {
    headers[name] = value;
  });
  return headers;
}

function getSetCookieValues(response) {
  if (typeof response.headers.getSetCookie === "function") {
    return response.headers.getSetCookie();
  }
  const single = response.headers.get("set-cookie");
  return single ? [single] : [];
}

async function collectTextResponse(response) {
  const text = await response.text();
  return {
    status: response.status,
    ok: response.ok,
    finalUrl: response.url,
    headers: responseHeadersToObject(response),
    setCookie: getSetCookieValues(response),
    text,
  };
}

async function collectBytesResponse(response) {
  const bytes = Buffer.from(await response.arrayBuffer());
  return {
    status: response.status,
    ok: response.ok,
    finalUrl: response.url,
    headers: responseHeadersToObject(response),
    setCookie: getSetCookieValues(response),
    bytes,
  };
}

function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function textPreview(value, max = 800) {
  const text = String(value ?? "");
  return text.length > max ? `${text.slice(0, max)}...[truncated]` : text;
}

function redactHeaders(headers) {
  const cloned = { ...(headers || {}) };
  for (const key of Object.keys(cloned)) {
    const lowered = key.toLowerCase();
    if (["cookie", "authorization", "x-goog-api-key"].includes(lowered)) {
      cloned[key] = "<redacted>";
    }
  }
  return cloned;
}

export {
  responseHeadersToObject,
  getSetCookieValues,
  collectTextResponse,
  collectBytesResponse,
  tryParseJson,
  textPreview,
  redactHeaders,
};
