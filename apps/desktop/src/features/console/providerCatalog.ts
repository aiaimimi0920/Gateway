import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { writeRequestClientProfile, type RequestClientProfile } from "./requestClientProfile";
import { writeCatalogWorkflow, type CatalogWorkflow } from "./providerCatalogWorkflow";
import { writeProviderCallFlow } from "./providerCallFlow";
import { WEB_CATALOG_TEMPLATES } from "./providerWebCatalog";
import { webCatalogAnchor } from "./providerSessionCatalog";
import { writeRecoveryWait, type RecoveryWaitChoice } from "./requestWait";
import { qwenCatalogBase, type QwenEndpointDraft } from "./qwenEndpoint";
import { VERTEX_EXPRESS_BASE } from "./vertexExpress";
import { VERTEX_OAUTH_TEMPLATE, vertexOAuthCatalogBase } from "./vertexOAuthCatalog";
import type { VertexTarget } from "./vertexOAuth";
import { validateLumaAgentsProvider } from "./lumaAgents";
import { LUMA_CATALOG_TEMPLATES, lumaMcpCatalogProvider } from "./lumaMcpCatalog";
import { KIRO_OAUTH_TEMPLATE, writeKiroOAuthCatalog } from "./kiroOAuthCatalog";
import { RIVER_TEMPLATE } from "./riverCatalog";
import { AGNES_TEMPLATE } from "./agnesCatalog";
import { MINIMAX_TEMPLATES } from "./minimaxCatalog";
import { SENSENOVA_TEMPLATE } from "./sensenovaCatalog";
import { COMMANDCODE_TEMPLATE } from "./commandcodeCatalog";
import { SAP_TEMPLATE, writeSapProvider, type SapTarget } from "./sapAiCore";
import { CUSTOM_API_TEMPLATE } from "./customApiCatalog";
import { CLOUDFLARE_TEMPLATE, requireCloudflareBase } from "./cloudflareWorkersAi";
import { DIM_TEMPLATES, writeDimProvider } from "./dimCatalog";
import { NOUS_TEMPLATE, writeNousProvider } from "./nousCatalog";
import { LOBSTERAI_TEMPLATE, writeLobsterAiProvider } from "./lobsteraiCatalog";
import { QUICK_OAUTH_TEMPLATE, writeQuickProvider } from "./quickCatalog";
import { SEARCH_CATALOG_TEMPLATES } from "./providerSearchCatalog";

export type ProviderCatalogCategory =
  | "mainstream"
  | "aggregator"
  | "search"
  | "third-party-compatible";

export type ProviderCompatibility = "auto" | "native" | "openai" | "anthropic" | "search";

export type ProviderCatalogTemplate = {
  id: string;
  providerId: string;
  labelZh: string;
  labelEn: string;
  descriptionZh: string;
  descriptionEn: string;
  category: ProviderCatalogCategory;
  compatibility: ProviderCompatibility;
  preset: string | null;
  adapter?: string;
  protocolProfile?: string;
  vendorKey: string;
  vendorName: string;
  baseUrl: string;
  supportedModels: readonly string[];
  custom?: boolean;
};

export type ProviderCatalogDraft = {
  sapTarget?: SapTarget;
  cloudflareAccountId?: string;
  kiroProfileArn?: string;
  vertexTarget?: VertexTarget;
  qwenEndpoint?: QwenEndpointDraft;
  recoveryWait?: RecoveryWaitChoice;
  workflow?: CatalogWorkflow;
  secrets?: Record<string, string>;
  requestClientProfile?: RequestClientProfile;
  siteHealthEnabled?: boolean;
  codexBasispointsEnabled?: boolean;
  discovery?: import("./accountDiscovery").AccountDiscovery;
  templateId: string;
  providerId: string;
  providerLabel: string;
  vendorKey: string;
  vendorName: string;
  baseUrl: string;
  supportedModels: string[];
  credentialId?: string;
  accountName?: string;
  apiKey?: string;
};

export type ProviderCatalogClassification = {
  category: ProviderCatalogCategory;
  compatibility: ProviderCompatibility;
};

const STANDARD_CATALOG_TEMPLATES = [
  {
    id: "bedrock-api", providerId: "bedrock-api", labelZh: "bedrock-api", labelEn: "bedrock-api",
    descriptionZh: "AWS Bedrock 原生 Converse；账号内明确 API Key / IAM / 临时 IAM，额度按实际主体和区域归属。",
    descriptionEn: "Native AWS Bedrock Converse; explicit API Key / IAM / temporary IAM per account, with actual principal and region quota scope.",
    category: "mainstream", compatibility: "native", preset: "bedrock-converse", vendorKey: "aws-bedrock", vendorName: "AWS Bedrock",
    baseUrl: "https://bedrock-runtime.us-east-1.amazonaws.com", supportedModels: [],
  },
  {
    id: "qwen-api", providerId: "qwen-api", labelZh: "qwen-api", labelEn: "qwen-api",
    descriptionZh: "Qwen 普通 API Key；明确选择地域与工作空间，独立于 Coding Plan 和 Web。",
    descriptionEn: "Ordinary Qwen API Key; explicitly select region and workspace, separate from Coding Plan and Web.",
    category: "mainstream", compatibility: "openai", preset: "qwen-official-api", vendorKey: "qwen", vendorName: "Qwen",
    baseUrl: "", supportedModels: [],
  },
  {
    id: "kiro-api-key", providerId: "kiro-api", labelZh: "kiro-api", labelEn: "kiro-api",
    descriptionZh: "Kiro CLI 原生 Key；固定 US East 兼容通道、手工精确模型，消耗同用户订阅 credits。",
    descriptionEn: "Native Kiro CLI Key; fixed US East compatibility channel and explicit model IDs, using the same user's subscription credits.",
    category: "mainstream", compatibility: "native", preset: "kiro-api-key", vendorKey: "kiro-platform", vendorName: "Kiro",
    baseUrl: "https://runtime.us-east-1.kiro.dev", supportedModels: [],
  },
  KIRO_OAUTH_TEMPLATE,
  {
    id: "vertex-express-api", providerId: "vertex-express-api", labelZh: "vertex-express-api", labelEn: "vertex-express-api",
    descriptionZh: "Vertex Express 原生 API Key；固定 global publisher-model 通道、手工精确模型，与 Full Vertex OAuth 分开。",
    descriptionEn: "Native Vertex Express API Key; fixed global publisher-model channel and exact manual models, separate from full Vertex OAuth.",
    category: "mainstream", compatibility: "native", preset: "vertex-express-api", vendorKey: "google-vertex-express", vendorName: "Google / Vertex Express",
    baseUrl: VERTEX_EXPRESS_BASE, supportedModels: [],
  },
  VERTEX_OAUTH_TEMPLATE,
  ...LUMA_CATALOG_TEMPLATES,
  {
    id: "chatgpt", providerId: "codex-oauth",
    labelZh: "codex-oauth", labelEn: "codex-oauth",
    descriptionZh: "ChatGPT Codex OAuth 账号，按订阅类型分组。",
    descriptionEn: "ChatGPT Codex OAuth accounts grouped by subscription.",
    category: "mainstream", compatibility: "native",
    preset: "chatgpt-codex-oauth-official-api",
    vendorKey: "chatgpt_platform", vendorName: "ChatGPT",
    baseUrl: "https://chatgpt.com/backend-api/codex",
    supportedModels: [],
  },
  {
    id: "openai",
    providerId: "openai-api",
    labelZh: "openai-api",
    labelEn: "openai-api",
    descriptionZh: "OpenAI Platform 的原生 Responses 与 Chat Completions 接口。",
    descriptionEn: "Native OpenAI Platform Responses and Chat Completions APIs.",
    category: "mainstream",
    compatibility: "native",
    preset: "openai",
    vendorKey: "openai",
    vendorName: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    supportedModels: [],
  },
  {
    id: "anthropic",
    providerId: "anthropic-api",
    labelZh: "anthropic-api",
    labelEn: "anthropic-api",
    descriptionZh: "Anthropic Messages 原生协议。",
    descriptionEn: "Native Anthropic Messages protocol.",
    category: "mainstream",
    compatibility: "anthropic",
    preset: "anthropic",
    vendorKey: "anthropic",
    vendorName: "Anthropic",
    baseUrl: "https://api.anthropic.com",
    supportedModels: [],
  },
  {
    id: "gemini-api",
    providerId: "gemini-api",
    labelZh: "gemini-api",
    labelEn: "gemini-api",
    descriptionZh: "Google Generative Language API；不同于 Gemini Web 与 Canvas 登录渠道。",
    descriptionEn: "Google Generative Language API, separate from Gemini Web and Canvas sign-in channels.",
    category: "mainstream",
    compatibility: "native",
    preset: "gemini-api",
    vendorKey: "google-gemini",
    vendorName: "Google / Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    supportedModels: [],
  },
  {
    id: "azure-api",
    providerId: "azure-api",
    labelZh: "azure-api",
    labelEn: "azure-api",
    descriptionZh: "Azure OpenAI v1 兼容接口；需要填写资源专属地址与部署模型名。",
    descriptionEn: "Azure OpenAI v1-compatible API; enter the resource URL and deployment model name.",
    category: "mainstream",
    compatibility: "openai",
    preset: "azure-api",
    vendorKey: "microsoft-azure",
    vendorName: "Microsoft / Azure OpenAI",
    baseUrl: "",
    supportedModels: [],
  },
  {
    id: "cohere-chat",
    providerId: "cohere-api",
    labelZh: "cohere-api",
    labelEn: "cohere-api",
    descriptionZh: "Cohere Chat 原生协议。",
    descriptionEn: "Native Cohere Chat protocol.",
    category: "mainstream",
    compatibility: "native",
    preset: "cohere-chat",
    vendorKey: "cohere",
    vendorName: "Cohere",
    baseUrl: "https://api.cohere.com",
    supportedModels: [],
  },
  {
    id: "groq-openai",
    providerId: "groq-api",
    labelZh: "groq-api",
    labelEn: "groq-api",
    descriptionZh: "Groq 的 OpenAI-compatible 官方接口。",
    descriptionEn: "Groq's official OpenAI-compatible API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "groq-openai",
    vendorKey: "groq",
    vendorName: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    supportedModels: [],
  },
  {
    id: "nvidia-openai",
    providerId: "nvidia-api",
    labelZh: "nvidia-api",
    labelEn: "nvidia-api",
    descriptionZh: "NVIDIA 的 OpenAI-compatible 推理接口。",
    descriptionEn: "NVIDIA's OpenAI-compatible inference API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "nvidia-openai",
    vendorKey: "nvidia",
    vendorName: "NVIDIA",
    baseUrl: "https://integrate.api.nvidia.com",
    supportedModels: ["openai/gpt-oss-20b"],
  },
  {
    id: "deepseek-api",
    providerId: "deepseek-api",
    labelZh: "deepseek-api",
    labelEn: "deepseek-api",
    descriptionZh: "DeepSeek 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "DeepSeek's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "deepseek-api",
    vendorKey: "deepseek",
    vendorName: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    supportedModels: [],
  },
  {
    id: "mistral-studio-api",
    providerId: "mistral-studio-api",
    labelZh: "mistral-studio-api",
    labelEn: "mistral-studio-api",
    descriptionZh: "使用 Studio API Key 调用 Mistral 标准模型接口。",
    descriptionEn: "Call the standard Mistral model API with a Studio API key.",
    category: "mainstream",
    compatibility: "openai",
    preset: "mistral-studio-api",
    vendorKey: "mistral",
    vendorName: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    supportedModels: [],
  },
  {
    id: "mistral-vibe-api",
    providerId: "mistral-vibe-api",
    labelZh: "mistral-vibe-api",
    labelEn: "mistral-vibe-api",
    descriptionZh: "使用 Vibe CLI 的 HTTP User-Agent；模型与额度由上游根据账号权限决定。",
    descriptionEn: "Uses the Vibe CLI HTTP User-Agent; models and quota depend on upstream account permissions.",
    category: "mainstream",
    compatibility: "openai",
    preset: "mistral-vibe-api",
    vendorKey: "mistral",
    vendorName: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    supportedModels: [],
  },
  {
    id: "xai-api",
    providerId: "xAI-api",
    labelZh: "xAI-api",
    labelEn: "xAI-api",
    descriptionZh: "xAI 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "xAI's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "xai-api",
    vendorKey: "xai",
    vendorName: "xAI",
    baseUrl: "https://api.x.ai/v1",
    supportedModels: [],
  },
  {
    id: "grok-oauth", providerId: "grok-oauth", labelZh: "grok-oauth", labelEn: "grok-oauth",
    descriptionZh: "Grok Build OAuth；需要可用于此集成的客户端，手动回调；与 API Key 和 Web 分池，尚待真实验收。",
    descriptionEn: "Grok Build OAuth with a permitted client and manual callback. Separate from API Key and Web; live acceptance pending.",
    category: "mainstream", compatibility: "openai", preset: "grok-oauth", vendorKey: "xai", vendorName: "xAI",
    baseUrl: "https://cli-chat-proxy.grok.com/v1", supportedModels: [],
  },
  {
    id: "perplexity-api",
    providerId: "perplexity-api",
    labelZh: "perplexity-api",
    labelEn: "perplexity-api",
    descriptionZh: "Perplexity Sonar 同步与流式兼容接口，不含 Agent 或 Search API。",
    descriptionEn: "Perplexity Sonar synchronous and streaming compatibility API; not Agent or Search API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "perplexity-api",
    vendorKey: "perplexity",
    vendorName: "Perplexity",
    baseUrl: "https://api.perplexity.ai",
    supportedModels: [],
  },
  {
    id: "openrouter-openai",
    providerId: "openrouter-api",
    labelZh: "openrouter-api",
    labelEn: "openrouter-api",
    descriptionZh: "多模型 OpenAI-compatible 聚合接口。",
    descriptionEn: "Multi-model OpenAI-compatible aggregation API.",
    category: "aggregator",
    compatibility: "openai",
    preset: "openrouter-openai",
    vendorKey: "openrouter",
    vendorName: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    supportedModels: ["openrouter/free"],
  },
  {
    id: "poe-openai",
    providerId: "poe-api",
    labelZh: "poe-api",
    labelEn: "poe-api",
    descriptionZh: "Poe 的 OpenAI-compatible 多 Bot 聚合接口。",
    descriptionEn: "Poe's OpenAI-compatible multi-bot aggregation API.",
    category: "aggregator",
    compatibility: "openai",
    preset: "poe-openai",
    vendorKey: "poe",
    vendorName: "Poe",
    baseUrl: "https://api.poe.com/v1",
    supportedModels: [],
  },
  ...SEARCH_CATALOG_TEMPLATES,
  RIVER_TEMPLATE,
  AGNES_TEMPLATE,
  ...MINIMAX_TEMPLATES,
  SENSENOVA_TEMPLATE,
  COMMANDCODE_TEMPLATE,
  SAP_TEMPLATE,
  CLOUDFLARE_TEMPLATE,
  ...DIM_TEMPLATES,
  NOUS_TEMPLATE,
  LOBSTERAI_TEMPLATE,
  QUICK_OAUTH_TEMPLATE,
  CUSTOM_API_TEMPLATE,
] as const satisfies readonly ProviderCatalogTemplate[];

export const PROVIDER_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = [
  ...STANDARD_CATALOG_TEMPLATES.flatMap((template) => [template, ...WEB_CATALOG_TEMPLATES.filter((web) => webCatalogAnchor(web.id) === template.id)]),
  ...WEB_CATALOG_TEMPLATES.filter((web) => !webCatalogAnchor(web.id)),
];

export function providerCatalogClassification(
  _providerId: string,
  preset: string | null | undefined,
  adapter?: string | null,
  protocolProfile?: string | null,
): ProviderCatalogClassification | null {
  const normalizedPreset = preset?.trim().toLowerCase() ?? "";
  if (
    normalizedPreset.length === 0 &&
    adapter?.trim().toLowerCase() === "openai_compatible" &&
    protocolProfile?.trim().toLowerCase() === "openai_compatible_generic"
  ) {
    return {
      category: "third-party-compatible",
      compatibility: "auto",
    };
  }
  return null;
}

export function providerDefinitionFromCatalogDraft(
  template: ProviderCatalogTemplate,
  draft: ProviderCatalogDraft,
): Record<string, unknown> {
  if (template.id === "lumalabs-mcp") return lumaMcpCatalogProvider(draft);
  if (template.id === "accio-oauth" && draft.baseUrl !== "https://phoenix-gw.alibaba.com") {
    throw new Error("Accio OAuth 仅支持固定的 https://phoenix-gw.alibaba.com 地址。");
  }
  if (template.id === "kiro-api-key" && draft.baseUrl !== template.baseUrl) {
    throw new Error("Kiro CLI API Key 仅支持固定的 US East runtime 地址。");
  }
  if (template.id === "vertex-express-api" && draft.baseUrl !== VERTEX_EXPRESS_BASE) {
    throw new Error("Vertex Express API Key 仅支持固定的 global publisher-model 地址。");
  }
  const provider: Record<string, unknown> = {
    id: draft.providerId,
    label: draft.providerLabel,
    vendor_key: draft.vendorKey,
    vendor_name: draft.vendorName,
    base_url: template.id === "cloudflare-workers-ai" ? requireCloudflareBase(draft.baseUrl) : template.id === "vertex-oauth" ? vertexOAuthCatalogBase(draft) : template.preset === "qwen-official-api" ? qwenCatalogBase(draft) : draft.baseUrl,
    supported_models: [...draft.supportedModels],
  };
  writeRequestClientProfile(provider, draft.requestClientProfile ?? "auto");
  writeRecoveryWait(provider, draft.recoveryWait);
  if (draft.siteHealthEnabled) provider.site_health = {};
  if (draft.codexBasispointsEnabled && template.id === "chatgpt") provider.codex_basispoints_enabled = true;
  if (template.preset) {
    provider.preset = template.preset;
  } else {
    provider.adapter = template.adapter ?? "openai_compatible";
    provider.protocol_profile = template.protocolProfile ?? "openai_compatible_generic";
  }
  if (template.id === "chatgpt") {
    provider.protocol_profile = "chatgpt_codex_backend";
    provider.credential_identity_categories = CHATGPT_POOL_CATEGORIES.map((category) => ({ ...category }));
  }
  if (template.id === "vertex-oauth") provider.auth_mode = "vertex_user_oauth";
  if (template.id === "kiro-oauth") writeKiroOAuthCatalog(provider, draft);
  if (template.id === "flowmusic-web") {
    provider.supported_protocol_families = ["producer_music"];
    provider.execution_mode = "direct_http";
  }
  if (draft.workflow) {
    writeCatalogWorkflow(provider, draft.workflow);
    writeProviderCallFlow(provider, template, draft);
  }
  if (template.id === "lumalabs-api") validateLumaAgentsProvider(provider);
  writeDimProvider(provider, template);
  writeNousProvider(provider, template);
  writeLobsterAiProvider(provider, template);
  writeQuickProvider(provider, template);
  writeSapProvider(provider, draft);
  return provider;
}
