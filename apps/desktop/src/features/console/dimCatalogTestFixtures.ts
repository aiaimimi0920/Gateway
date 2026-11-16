export const model = (id = "opaque-model") => ({ id, name: "Opaque model", protocol: "responses" as const,
  context_window: null, max_output_tokens: null, tools: true, reasoning: null, reasoning_efforts: [],
  input_modalities: ["text"], output_modalities: ["text"], service_tiers: [], deprecated: false, zdr: false });
export const catalog = { site: "china" as const, client_id: "permitted-gateway", subject: "subject", models: [model()] };
export const document = { providers: [{ id: "dim", preset: "dim-cn-oauth", auth_mode: "dim_user_oauth", base_url: "https://dimagent.cn/v1",
  explicit_credentials_only: true, credentials: [{ id: "account", api_key: "", refresh_token: "", dim_oauth: { site: "china", client_id: "permitted-gateway", subject: "subject" },
    dim_catalog: { ...catalog, models: [model("old-fetched")] }, supported_models: ["old-fetched", "manual-model"] }] }], model_routes: [], aliases: {} };
