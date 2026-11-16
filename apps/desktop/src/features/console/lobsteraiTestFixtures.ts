import type { LobsterAiAuthSession } from "../../api/console/lobsterai-auth";
export const redirect = "http://127.0.0.1:43567/auth/callback";
export const authorization = `https://lobsterai.youdao.com/portal#/login?${new URLSearchParams({ source: "electron", state: "A".repeat(43), redirect_uri: redirect })}`;
export const lobsterSession = (status: LobsterAiAuthSession["status"]): LobsterAiAuthSession => ({
  id: "login", providerId: "renamed", expectedRevision: "r1", status, authorizationUrl: status === "waiting_user" ? authorization : null,
  redirectUri: redirect, credentialId: status === "succeeded" ? "stable" : null, targetCredentialId: null,
  savedRevision: status === "succeeded" ? "r2" : null, errorCode: null,
});
export const model = (modelId = "opaque/model") => ({ modelId, modelName: null, apiFormat: "openai", runtimeProfile: null,
  supportsToolCalling: null, supportsThinking: null, supportsImage: null, supportsVideo: null,
  contextWindow: null, maxTokens: null, requestCapabilities: null, accessible: null });
export const catalog = { models: [model()] };
export const provider = { id: "renamed", auth_mode: "lobsterai_session", adapter: "openai_compatible", explicit_credentials_only: true,
  base_url: "https://lobsterai-server.youdao.com/api/proxy/v1", credentials: [{ id: "stable", lobsterai: { installationId: "synthetic-installation", identity: { yid: "synthetic-user" } },
    lobsterai_catalog: { models: [model("old-fetched")] }, supported_models: ["old-fetched", "manual-model"] }] };
