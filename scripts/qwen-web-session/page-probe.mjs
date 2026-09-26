export async function probeQwenPage(page, baseUrl, preferredModels) {
  return page.evaluate(
    async ({ baseUrl, preferredModels }) => {
      const trimString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const decodeJwtExp = (token) => {
        try {
          const [, payload] = token.split(".");
          if (!payload) {
            return null;
          }
          const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
          const jsonText = atob(normalized);
          const decoded = JSON.parse(jsonText);
          return typeof decoded.exp === "number"
            ? new Date(decoded.exp * 1000).toISOString()
            : null;
        } catch {
          return null;
        }
      };
      const headersFor = (token) => ({
        Authorization: `Bearer ${token}`,
        Accept: "application/json, text/plain, */*",
        "Content-Type": "application/json",
      });
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
      const availableTokens = [
        { source: "active_token", token: trimString(localStorage.getItem("active_token")) },
        { source: "token", token: trimString(localStorage.getItem("token")) },
      ].filter((entry) => entry.token);

      let resolvedToken = null;
      let tokenSource = null;
      let authProbe = null;
      for (const entry of availableTokens) {
        const probe = await requestJson(`${baseUrl}/api/v1/auths/`, {
          method: "GET",
          headers: headersFor(entry.token),
        });
        if (probe.ok) {
          resolvedToken = entry.token;
          tokenSource = entry.source;
          authProbe = probe;
          break;
        }
        if (authProbe === null) {
          authProbe = probe;
        }
      }
      if (!resolvedToken) {
        return {
          ok: false,
          error: {
            code: "qwen_web_session_missing_active_token",
            message:
              "No valid Qwen Web browser token was available from localStorage. The gateway refresh worker only reuses an existing signed-in Qwen Web session; it does not register or sign in accounts.",
            status: authProbe?.status ?? 401,
          },
          authProbe,
          localStorageKeys: Object.keys(localStorage),
        };
      }

      const modelProbe = await requestJson(`${baseUrl}/api/models`, {
        method: "GET",
        headers: headersFor(resolvedToken),
      });
      const rawModels = Array.isArray(modelProbe?.json?.data)
        ? modelProbe.json.data
        : Array.isArray(modelProbe?.json)
          ? modelProbe.json
          : [];
      const normalizedModels = rawModels
        .map((entry) => {
          if (typeof entry === "string") {
            return { id: entry, label: entry };
          }
          if (!entry || typeof entry !== "object") {
            return null;
          }
          const id =
            trimString(entry.code) ??
            trimString(entry.id) ??
            trimString(entry.model) ??
            trimString(entry.name);
          if (!id) {
            return null;
          }
          return {
            id,
            label:
              trimString(entry.name) ??
              trimString(entry.display_name) ??
              trimString(entry.label) ??
              id,
          };
        })
        .filter(Boolean);
      const selectedModel =
        preferredModels.find((candidate) =>
          normalizedModels.some((entry) => entry.id === candidate),
        ) ?? normalizedModels[0]?.id ?? null;
      const selectedDisplayModel =
        normalizedModels.find((entry) => entry.id === selectedModel)?.label ?? selectedModel;

      let createChatProbe = null;
      if (selectedModel) {
        createChatProbe = await requestJson(`${baseUrl}/api/v2/chats/new`, {
          method: "POST",
          headers: headersFor(resolvedToken),
          body: JSON.stringify({
            title: `api_${Math.floor(Date.now() / 1000)}`,
            models: [selectedModel],
            chat_mode: "normal",
            chat_type: "t2t",
            timestamp: Math.floor(Date.now() / 1000),
          }),
        });
      }

      const expiresAt =
        trimString(authProbe?.json?.expires_at) ??
        trimString(authProbe?.json?.data?.expires_at) ??
        decodeJwtExp(resolvedToken);

      return {
        ok: true,
        authToken: resolvedToken,
        tokenSource,
        expiresAt,
        selectedModel,
        selectedDisplayModel,
        availableModels: normalizedModels,
        authProbe: {
          status: authProbe?.status ?? null,
          ok: authProbe?.ok ?? false,
          userId:
            trimString(authProbe?.json?.id) ??
            trimString(authProbe?.json?.data?.id) ??
            null,
          email:
            trimString(authProbe?.json?.email) ??
            trimString(authProbe?.json?.data?.email) ??
            null,
        },
        modelProbe: {
          status: modelProbe?.status ?? null,
          ok: modelProbe?.ok ?? false,
          count: normalizedModels.length,
        },
        createChatProbe: {
          status: createChatProbe?.status ?? null,
          ok: createChatProbe?.ok ?? false,
          chatId:
            trimString(createChatProbe?.json?.id) ??
            trimString(createChatProbe?.json?.data?.id) ??
            null,
        },
        localStorageKeys: Object.keys(localStorage),
      };
    },
    { baseUrl, preferredModels },
  );
}
