/** Browser-only session probe; keep self-contained for Playwright serialization. */
export async function probeSession({ preferredModels, cookieToken }) {
  const trimString = (value) =>
    typeof value === "string" && value.trim() ? value.trim() : null;
  const JWT_RE = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;
  const decodeJwt = (token) => {
    try {
      const [, payload] = token.split(".");
      if (!payload) {
        return null;
      }
      const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
      const jsonText = atob(normalized);
      return JSON.parse(jsonText);
    } catch {
      return null;
    }
  };
  const collectStorageModels = () => {
    const rawCandidates = [];
    for (const key of Object.keys(localStorage)) {
      const lowered = key.toLowerCase();
      if (!lowered.includes("model")) {
        continue;
      }
      const raw = localStorage.getItem(key);
      if (raw) {
        rawCandidates.push(raw);
      }
    }
    const models = new Set();
    const visit = (value) => {
      if (typeof value === "string") {
        if (
          ["qwen-lora", "google-nano-banana-2", "gpt-image-1.5"].includes(value.trim())
        ) {
          models.add(value.trim());
        }
        return;
      }
      if (Array.isArray(value)) {
        value.forEach(visit);
        return;
      }
      if (value && typeof value === "object") {
        Object.values(value).forEach(visit);
      }
    };
    for (const raw of rawCandidates) {
      try {
        visit(JSON.parse(raw));
      } catch {
        visit(raw);
      }
    }
    return Array.from(models);
  };
  const extractTokenFromStorage = () => {
    const preferredKeys = ["token", "active_token", "authToken"];
    for (const key of preferredKeys) {
      const value = trimString(localStorage.getItem(key));
      if (value && JWT_RE.test(value)) {
        return { source: `localStorage:${key}`, token: value };
      }
    }
    for (const key of Object.keys(localStorage)) {
      const value = trimString(localStorage.getItem(key));
      if (value && JWT_RE.test(value)) {
        return { source: `localStorage:${key}`, token: value };
      }
    }
    return null;
  };
  const collectJwtCandidates = (value, sink) => {
    if (!value) {
      return;
    }
    if (typeof value === "string") {
      const trimmed = value.trim();
      if (JWT_RE.test(trimmed)) {
        sink.add(trimmed);
      }
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((entry) => collectJwtCandidates(entry, sink));
      return;
    }
    if (typeof value === "object") {
      Object.values(value).forEach((entry) => collectJwtCandidates(entry, sink));
    }
  };
  const openDatabase = (name, version) =>
    new Promise((resolve, reject) => {
      const request = indexedDB.open(name, version);
      request.onerror = () => reject(request.error);
      request.onsuccess = () => resolve(request.result);
    });
  const readStorePreview = (db, storeName) =>
    new Promise((resolve, reject) => {
      const rows = [];
      const transaction = db.transaction(storeName, "readonly");
      const store = transaction.objectStore(storeName);
      const request = store.openCursor();
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor || rows.length >= 40) {
          resolve(rows);
          return;
        }
        rows.push(cursor.value);
        cursor.continue();
      };
    });
  const extractTokenFromIndexedDb = async () => {
    if (typeof indexedDB.databases !== "function") {
      return null;
    }
    const databases = await indexedDB.databases();
    const jwtCandidates = new Set();
    for (const info of databases) {
      if (!info?.name) {
        continue;
      }
      try {
        const db = await openDatabase(info.name, info.version);
        try {
          for (const storeName of Array.from(db.objectStoreNames)) {
            const previewRows = await readStorePreview(db, storeName);
            previewRows.forEach((row) => collectJwtCandidates(row, jwtCandidates));
          }
        } finally {
          db.close();
        }
      } catch {
        // Ignore inaccessible IndexedDB stores and keep scanning.
      }
    }
    const token = Array.from(jwtCandidates)[0] ?? null;
    return token ? { source: "indexedDB", token } : null;
  };
  const requestJson = async (url, init) => {
    const response = await fetch(url, init);
    const text = await response.text();
    let json = null;
    try {
      json = text ? JSON.parse(text) : null;
    } catch {
      json = null;
    }
    return {
      status: response.status,
      ok: response.ok,
      text,
      json,
    };
  };

  const tokenFromStorage = extractTokenFromStorage();
  const tokenFromIndexedDb = await extractTokenFromIndexedDb();
  const resolvedToken =
    trimString(cookieToken) ??
    tokenFromStorage?.token ??
    tokenFromIndexedDb?.token ??
    null;
  if (!resolvedToken) {
    return {
      ok: false,
      error: {
        status: 401,
        code: "chataibot_session_missing_token_cookie",
        message:
          "No valid ChatAIBot token cookie was available from the cloned browser profile. The gateway session worker only reuses an existing signed-in browser session; it does not register or sign in accounts.",
      },
      localStorageKeys: Object.keys(localStorage),
    };
  }

  const answersProbe = await requestJson("/api/user/answers-count/v2", {
    method: "GET",
    credentials: "include",
    headers: {
      Accept: "application/json",
    },
  });
  const tokenPayload = decodeJwt(resolvedToken) ?? {};
  const storageModels = collectStorageModels();
  const selectedModel =
    preferredModels.find((candidate) => storageModels.includes(candidate)) ??
    preferredModels[0] ??
    storageModels[0] ??
    null;
  return {
    ok: true,
    authToken: resolvedToken,
    tokenSource:
      (trimString(cookieToken) ? "cookie:token" : null) ??
      tokenFromStorage?.source ??
      tokenFromIndexedDb?.source ??
      null,
    expiresAt:
      typeof tokenPayload.exp === "number"
        ? new Date(tokenPayload.exp * 1000).toISOString()
        : null,
    userId: trimString(tokenPayload.userId) ?? trimString(tokenPayload.sub),
    accountName:
      trimString(tokenPayload.email) ??
      trimString(tokenPayload.username) ??
      trimString(tokenPayload.userId) ??
      null,
    selectedModel,
    availableModels: Array.from(new Set([...storageModels, ...preferredModels])),
    quotaProbe: {
      status: answersProbe.status,
      ok: answersProbe.ok,
      leftAnswersCount:
        answersProbe?.json?.leftAnswersCount ??
        answersProbe?.json?.data?.leftAnswersCount ??
        null,
      message:
        answersProbe?.json?.message ??
        answersProbe?.json?.error ??
        answersProbe?.text ??
        null,
    },
    localStorageKeys: Object.keys(localStorage),
  };
}
