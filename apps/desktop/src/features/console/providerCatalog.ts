import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { writeRequestClientProfile, type RequestClientProfile } from "./requestClientProfile";
import { writeCatalogWorkflow, type CatalogWorkflow } from "./providerCatalogWorkflow";
import { writeProviderCallFlow } from "./providerCallFlow";
import { WEB_CATALOG_TEMPLATES } from "./providerWebCatalog";
import { writeRecoveryWait, type RecoveryWaitChoice } from "./requestWait";

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
    id: "mistral-api",
    providerId: "mistral-api",
    labelZh: "mistral-api",
    labelEn: "mistral-api",
    descriptionZh: "Mistral 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "Mistral's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "mistral-api",
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
  {
    id: "linkup",
    providerId: "linkup-api",
    labelZh: "linkup-api",
    labelEn: "linkup-api",
    descriptionZh: "搜索、抓取与研究接口。",
    descriptionEn: "Search, fetch, and research APIs.",
    category: "search",
    compatibility: "search",
    preset: "linkup",
    vendorKey: "linkup",
    vendorName: "Linkup",
    baseUrl: "https://api.linkup.so",
    supportedModels: ["linkup-search", "linkup-fetch", "linkup-research", "search", "fetch"],
  },
  {
    id: "tavily",
    providerId: "tavily-api",
    labelZh: "tavily-api",
    labelEn: "tavily-api",
    descriptionZh: "Tavily 官方搜索与正文提取接口。",
    descriptionEn: "Tavily's official search and extract APIs.",
    category: "search",
    compatibility: "search",
    preset: "tavily",
    vendorKey: "tavily",
    vendorName: "Tavily",
    baseUrl: "https://api.tavily.com",
    supportedModels: ["tavily-search", "tavily-fetch", "search", "fetch"],
  },
  {
    id: "you",
    providerId: "you-api",
    labelZh: "you-api",
    labelEn: "you-api",
    descriptionZh: "You.com 官方搜索与网页读取接口。",
    descriptionEn: "You.com's official search and contents APIs.",
    category: "search",
    compatibility: "search",
    preset: "you",
    vendorKey: "you",
    vendorName: "You.com",
    baseUrl: "https://ydc-index.io",
    supportedModels: ["you-search", "you-fetch", "search", "fetch"],
  },
  {
    id: "exa",
    providerId: "exa-api",
    labelZh: "exa-api",
    labelEn: "exa-api",
    descriptionZh: "Exa 官方搜索与内容抓取接口。",
    descriptionEn: "Exa's official search and content fetch APIs.",
    category: "search",
    compatibility: "search",
    preset: "exa",
    vendorKey: "exa",
    vendorName: "Exa",
    baseUrl: "https://api.exa.ai",
    supportedModels: ["exa-search", "exa-fetch", "search", "fetch"],
  },
  {
    id: "jina",
    providerId: "jina-api",
    labelZh: "jina-api",
    labelEn: "jina-api",
    descriptionZh: "Jina AI 官方搜索与网页读取接口，共用一个账号。",
    descriptionEn: "Jina AI web search and page reading with one shared account.",
    category: "search",
    compatibility: "search",
    preset: "jina",
    vendorKey: "jina-ai",
    vendorName: "Jina AI",
    baseUrl: "https://s.jina.ai",
    supportedModels: ["jina-search", "jina-fetch", "search", "fetch"],
  },
  {
    id: "websearchapi",
    providerId: "websearchapi-api",
    labelZh: "websearchapi-api",
    labelEn: "websearchapi-api",
    descriptionZh: "WebSearchAPI 官方搜索与网页抓取接口。",
    descriptionEn: "WebSearchAPI's official search and scraper APIs.",
    category: "search",
    compatibility: "search",
    preset: "websearchapi",
    vendorKey: "websearchapi",
    vendorName: "WebSearchAPI",
    baseUrl: "https://api.websearchapi.ai",
    supportedModels: ["websearchapi-search", "websearchapi-fetch", "search", "fetch"],
  },
  {
    id: "custom-api-provider",
    providerId: "custom-api",
    labelZh: "custom-api",
    labelEn: "custom-api",
    descriptionZh: "自动识别模型与调用协议。",
    descriptionEn: "Discover models and API protocol automatically.",
    category: "third-party-compatible",
    compatibility: "auto",
    preset: null,
    adapter: "openai_compatible",
    protocolProfile: "openai_compatible_generic",
    vendorKey: "third-party",
    vendorName: "Custom API provider",
    baseUrl: "",
    supportedModels: [],
    custom: true,
  },
] as const satisfies readonly ProviderCatalogTemplate[];

export const PROVIDER_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = STANDARD_CATALOG_TEMPLATES.flatMap((template) => [
  template, ...WEB_CATALOG_TEMPLATES.filter((web) => web.id === "chatgpt-web" ? template.id === "chatgpt" : template.id === "gemini-api"),
]);

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
  const provider: Record<string, unknown> = {
    id: draft.providerId,
    label: draft.providerLabel,
    vendor_key: draft.vendorKey,
    vendor_name: draft.vendorName,
    base_url: draft.baseUrl,
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
  if (draft.workflow) {
    writeCatalogWorkflow(provider, draft.workflow);
    writeProviderCallFlow(provider, template, draft);
  }
  return provider;
}
