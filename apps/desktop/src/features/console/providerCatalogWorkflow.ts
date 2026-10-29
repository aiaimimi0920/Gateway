// Creation drafts compose existing runtime owners; no credentials or upstream I/O here.
import type { ConsoleCredentialPoolAutomationDriver, CredentialTestPolicy } from "../../api/contracts";
import type { ProviderCatalogTemplate } from "./providerCatalog";
import { defaultTestPolicy, testPolicyError } from "./credentialTestPolicyDocument";
import { siteHealthConfigSchema, type SiteHealthConfig } from "./siteHealth";
import { validateStorageConnection, type StorageConnection } from "./providerStorageConnection";
import { validateSiteHealthPlans } from "./siteHealthValidation";

export const CALL_ADAPTERS = {
  auto: ["自动", "Automatic", "openai_compatible", "openai_compatible_generic"],
  openai: ["OpenAI（自动）", "OpenAI (automatic)", "openai_compatible", "openai_compatible_generic"],
  anthropic: ["Anthropic Messages", "Anthropic Messages", "anthropic_compatible", "anthropic"],
  gemini: ["Gemini API", "Gemini API", "gemini_api_compatible", "gemini_api"],
  cohere: ["Cohere Chat", "Cohere Chat", "cohere_compatible", "cohere"],
} as const;
export type CallAdapter = keyof typeof CALL_ADAPTERS;
export const SPECIAL_ADAPTERS = {
  none: ["无（标准 API）", "None (standard API)", ""],
  basispoints: ["Basispoints", "Basispoints", "chatgpt-codex-oauth-official-api"],
  gemini_web: ["Gemini Web", "Gemini Web", "gemini-web-chat"],
  chatgpt_web: ["ChatGPT Web", "ChatGPT Web", "chatgpt-web-reverse"],
  aistudio_web: ["AI Studio Web", "AI Studio Web", "aistudio-web-reverse"],
} as const;
export type SpecialAdapter = keyof typeof SPECIAL_ADAPTERS;
export type CatalogWorkflow = {
  callAdapter: CallAdapter;
  specialAdapter: SpecialAdapter;
  mappings: Record<string, string[]>;
  testPolicy: CredentialTestPolicy;
  poolMin: number;
  poolMax: number;
  driverId: string;
  autoRefill: boolean;
  autoPrune: boolean;
  permanentDelete: boolean;
  storage?: StorageConnection;
  archive?: StorageConnection;
  health: SiteHealthConfig;
};

// Keys are runtime presets, not brands: the OpenAI reader covers Codex, not Platform.
const HEALTH_ADAPTERS: Record<string, SiteHealthConfig["adapter"]> = {
  "chatgpt-codex-oauth-official-api": "openai-status", anthropic: "anthropic-status", "gemini-api": "gemini-status",
  "azure-api": "azure-status", "cohere-chat": "cohere-status", "groq-openai": "groq-status",
  "nvidia-openai": "nvidia-status", "together-openai": "together-status", "deepseek-api": "deepseek-status",
  "mistral-api": "mistral-status", "xai-api": "xai-status",
  "perplexity-api": "perplexity-status", "openrouter-openai": "openrouter-status", "poe-openai": "poe-status",
  linkup: "linkup-status", tavily: "tavily-status", you: "you-status", exa: "exa-status",
  jina: "jina-status",
};
export function catalogHealthAdapter(template: ProviderCatalogTemplate) {
  return template.custom ? "auto" : HEALTH_ADAPTERS[template.preset ?? ""] ?? null;
}
export function compatibleCatalogDrivers(drivers: readonly ConsoleCredentialPoolAutomationDriver[], providerId: string) {
  return drivers.filter((driver) => driver.providerIds.includes(providerId.trim()));
}
export function defaultCatalogWorkflow(template: ProviderCatalogTemplate): CatalogWorkflow {
  const special = Object.entries(SPECIAL_ADAPTERS).find(([, value]) => value[2] === template.preset)?.[0];
  const testPolicy = defaultTestPolicy();
  // 仅保留文档示例作为用户主动测试的单模型目标，不批量探测目录。
  if (["mistral-api", "xai-api", "deepseek-api", "perplexity-api"].includes(template.id)) {
    testPolicy.modelSelection = "selected"; testPolicy.models = [template.id === "perplexity-api" ? "sonar" : template.id === "deepseek-api" ? "deepseek-flash" : template.id === "xai-api" ? "grok-4.7" : "mistral-large-latest"];
    testPolicy.cases = [{ id: "connection", name: "连通", prompt: "Reply with only OK.", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  // Catalog access does not imply credit or model access; never probe the full directory.
  if (["openai", "anthropic", "gemini-api"].includes(template.id)) {
    testPolicy.modelSelection = "selected"; testPolicy.models = [template.id === "gemini-api" ? "gemini-3.5-flash-lite" : template.id === "anthropic" ? "claude-sonnet-4-6" : "gpt-4o-mini"];
    testPolicy.cases = [{ id: "connection", name: "连通", prompt: "Reply with only OK.", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  // Keep the opt-in probe to one observed zero-price model, not a paid catalog sweep.
  if (template.id === "poe-openai") {
    testPolicy.modelSelection = "selected"; testPolicy.models = ["gemma-4-31b"];
    testPolicy.cases = [{ id: "connection", name: "连通", prompt: "Reply with only OK.", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  // Directory visibility is not callable evidence; opt in to one current chat model only.
  if (template.id === "groq-openai") {
    testPolicy.modelSelection = "selected"; testPolicy.models = ["openai/gpt-oss-20b"];
    testPolicy.cases = [{ id: "connection", name: "连通", prompt: "Reply with only OK.", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  if (template.id === "cohere-chat") {
    testPolicy.modelSelection = "selected"; testPolicy.models = ["command-r7b-12-2024"];
    testPolicy.cases = [{ id: "connection", name: "连通", prompt: "Reply with only OK.", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  // Search verifies returned web results, not chat instructions or an invented IQ score.
  if (["you", "exa", "tavily", "linkup", "websearchapi"].includes(template.id)) testPolicy.cases = [{ id: "search", name: "搜索连通",
    prompt: "Rust programming language official documentation", expectedAnswer: null, difficulty: 1, enabled: true }];
  // Fetch is a different operation; never report it as verified by a search query.
  if (["you", "exa", "tavily", "linkup", "websearchapi"].includes(template.id)) { testPolicy.modelSelection = "selected"; testPolicy.models = [`${template.id}-search`]; }
  // One public URL is a valid search query and an explicit Reader target, not a chat question.
  if (template.id === "jina") {
    testPolicy.modelSelection = "selected"; testPolicy.models = ["jina-search", "jina-fetch"];
    testPolicy.cases = [{ id: "web", name: "搜索与读取连通", prompt: "https://example.com/", expectedAnswer: null, difficulty: 1, enabled: true }];
  }
  return {
    callAdapter: template.compatibility === "anthropic" ? "anthropic" : template.id === "gemini-api" ? "gemini"
      : template.preset === "cohere-chat" ? "cohere" : "auto",
    specialAdapter: special && special !== "basispoints" ? special as SpecialAdapter : "none",
    mappings: {}, testPolicy, poolMin: 1, poolMax: 100,
    driverId: "", autoRefill: false, autoPrune: false, permanentDelete: false,
    // Public health reads are independent of paid account tests; unmatched products stay unsupported.
    health: siteHealthConfigSchema.parse({ enabled: Boolean(catalogHealthAdapter(template)), adapter: catalogHealthAdapter(template) ?? "auto" }),
  };
}
export function catalogWorkflowError(workflow: CatalogWorkflow, secrets: Record<string, string> = {}): string | null {
  if (!Number.isSafeInteger(workflow.poolMin) || !Number.isSafeInteger(workflow.poolMax)
    || workflow.poolMin < 0 || workflow.poolMax < 1 || workflow.poolMin > workflow.poolMax) return "凭据池大小须为整数，且 0 ≤ 最小值 ≤ 最大值。";
  const health = siteHealthConfigSchema.safeParse(workflow.health);
  if (!health.success) return "站点健康配置不合法，请检查成功率、间隔和有效期。";
  try { validateSiteHealthPlans(workflow.health, new Set()); } catch (error) { return String(error instanceof Error ? error.message : error); }
  for (const target of ["storage", "archive"] as const) {
    const connection = workflow[target];
    if (connection) { const error = validateStorageConnection(connection); if (error) return error; }
    if (connection?.type === "webdav" && secrets[`credential_${target}_connection/password`] && !connection.username?.trim()) return "WebDAV 密码需要同时填写用户名。";
  }
  return testPolicyError(workflow.testPolicy);
}
export function writeCatalogWorkflow(provider: Record<string, unknown>, workflow: CatalogWorkflow) {
  provider.model_map_targets = Object.fromEntries(Object.entries(workflow.mappings).filter(([, targets]) => targets.length).map(([source, targets]) => [source, [...targets]]));
  provider.test_policy = structuredClone(workflow.testPolicy);
  provider.pool_min_size = workflow.poolMin;
  provider.pool_target_size = workflow.poolMax;
  provider.auto_refill_enabled = workflow.autoRefill;
  provider.auto_prune_enabled = workflow.autoPrune;
  provider.credential_permanent_delete_enabled = workflow.permanentDelete;
  if (workflow.driverId) provider.credential_automation_driver_id = workflow.driverId;
  if (workflow.storage) provider.credential_storage_connection = structuredClone(workflow.storage);
  if (workflow.archive) provider.credential_archive_connection = structuredClone(workflow.archive);
  if (workflow.health.enabled) provider.site_health = siteHealthConfigSchema.parse(workflow.health);
  else delete provider.site_health;
}
