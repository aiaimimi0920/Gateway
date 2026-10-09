import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";

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
  discovery?: import("./accountDiscovery").AccountDiscovery;
  templateId: string;
  providerId: string;
  providerLabel: string;
  vendorKey: string;
  vendorName: string;
  baseUrl: string;
  supportedModels: string[];
  credentialId: string;
  accountName: string;
  apiKey: string;
};

export type ProviderCatalogClassification = {
  category: ProviderCatalogCategory;
  compatibility: ProviderCompatibility;
};

export const PROVIDER_CATALOG_TEMPLATES = [
  {
    id: "chatgpt", providerId: "chatgpt",
    labelZh: "ChatGPT 官方凭证池", labelEn: "ChatGPT OAuth pool",
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
    providerId: "openai",
    labelZh: "OpenAI 官方 API",
    labelEn: "OpenAI official API",
    descriptionZh: "OpenAI Platform 的原生 Responses 与 Chat Completions 接口。",
    descriptionEn: "Native OpenAI Platform Responses and Chat Completions APIs.",
    category: "mainstream",
    compatibility: "native",
    preset: "openai",
    vendorKey: "openai",
    vendorName: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    supportedModels: ["gpt-4o"],
  },
  {
    id: "anthropic",
    providerId: "anthropic",
    labelZh: "Anthropic 官方 API",
    labelEn: "Anthropic official API",
    descriptionZh: "Anthropic Messages 原生协议。",
    descriptionEn: "Native Anthropic Messages protocol.",
    category: "mainstream",
    compatibility: "anthropic",
    preset: "anthropic",
    vendorKey: "anthropic",
    vendorName: "Anthropic",
    baseUrl: "https://api.anthropic.com",
    supportedModels: ["claude-sonnet-4-6"],
  },
  {
    id: "gemini-api",
    providerId: "gemini-api",
    labelZh: "Google Gemini API",
    labelEn: "Google Gemini API",
    descriptionZh: "Google Generative Language API；不同于 Gemini Web 与 Canvas 登录渠道。",
    descriptionEn: "Google Generative Language API, separate from Gemini Web and Canvas sign-in channels.",
    category: "mainstream",
    compatibility: "native",
    preset: "gemini-api",
    vendorKey: "google-gemini",
    vendorName: "Google / Gemini",
    baseUrl: "https://generativelanguage.googleapis.com",
    supportedModels: ["gemini-2.5-flash"],
  },
  {
    id: "azure-openai",
    providerId: "azure-openai",
    labelZh: "Azure OpenAI",
    labelEn: "Azure OpenAI",
    descriptionZh: "Azure OpenAI v1 兼容接口；需要填写资源专属地址与部署模型名。",
    descriptionEn: "Azure OpenAI v1-compatible API; enter the resource URL and deployment model name.",
    category: "mainstream",
    compatibility: "openai",
    preset: "azure-openai",
    vendorKey: "microsoft-azure",
    vendorName: "Microsoft / Azure OpenAI",
    baseUrl: "",
    supportedModels: [],
  },
  {
    id: "cohere-chat",
    providerId: "cohere",
    labelZh: "Cohere 官方 API",
    labelEn: "Cohere official API",
    descriptionZh: "Cohere Chat 原生协议。",
    descriptionEn: "Native Cohere Chat protocol.",
    category: "mainstream",
    compatibility: "native",
    preset: "cohere-chat",
    vendorKey: "cohere",
    vendorName: "Cohere",
    baseUrl: "https://api.cohere.com",
    supportedModels: ["command-a-03-2025"],
  },
  {
    id: "groq-openai",
    providerId: "groq",
    labelZh: "Groq",
    labelEn: "Groq",
    descriptionZh: "Groq 的 OpenAI-compatible 官方接口。",
    descriptionEn: "Groq's official OpenAI-compatible API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "groq-openai",
    vendorKey: "groq",
    vendorName: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    supportedModels: ["llama-3.3-70b-versatile"],
  },
  {
    id: "nvidia-openai",
    providerId: "nvidia",
    labelZh: "NVIDIA NIM",
    labelEn: "NVIDIA NIM",
    descriptionZh: "NVIDIA 的 OpenAI-compatible 推理接口。",
    descriptionEn: "NVIDIA's OpenAI-compatible inference API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "nvidia-openai",
    vendorKey: "nvidia",
    vendorName: "NVIDIA",
    baseUrl: "https://integrate.api.nvidia.com",
    supportedModels: ["deepseek-ai/deepseek-v3.2"],
  },
  {
    id: "together-openai",
    providerId: "together",
    labelZh: "Together AI",
    labelEn: "Together AI",
    descriptionZh: "Together AI 的 OpenAI-compatible 聚合接口。",
    descriptionEn: "Together AI's OpenAI-compatible aggregation API.",
    category: "aggregator",
    compatibility: "openai",
    preset: "together-openai",
    vendorKey: "together",
    vendorName: "Together AI",
    baseUrl: "https://api.together.xyz/v1",
    supportedModels: ["meta-llama/Llama-3.3-70B-Instruct-Turbo"],
  },
  {
    id: "deepseek-openai",
    providerId: "deepseek",
    labelZh: "DeepSeek 官方 API",
    labelEn: "DeepSeek official API",
    descriptionZh: "DeepSeek 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "DeepSeek's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "deepseek-openai",
    vendorKey: "deepseek",
    vendorName: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    supportedModels: ["deepseek-chat"],
  },
  {
    id: "mistral-openai",
    providerId: "mistral",
    labelZh: "Mistral AI 官方 API",
    labelEn: "Mistral AI official API",
    descriptionZh: "Mistral 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "Mistral's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "mistral-openai",
    vendorKey: "mistral",
    vendorName: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    supportedModels: ["mistral-large-latest"],
  },
  {
    id: "qwen-dashscope-openai",
    providerId: "qwen-dashscope-openai",
    labelZh: "阿里云百炼 / Qwen",
    labelEn: "Alibaba Cloud DashScope / Qwen",
    descriptionZh: "DashScope 的 OpenAI-compatible 通用模型接口。",
    descriptionEn: "DashScope's OpenAI-compatible general model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "qwen-dashscope-openai",
    vendorKey: "alibaba-qwen",
    vendorName: "Alibaba / Qwen",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    supportedModels: ["qwen3-coder-plus", "qwen-plus", "qwen-turbo"],
  },
  {
    id: "xai-openai",
    providerId: "xai",
    labelZh: "xAI 官方 API",
    labelEn: "xAI official API",
    descriptionZh: "xAI 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "xAI's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "xai-openai",
    vendorKey: "xai",
    vendorName: "xAI",
    baseUrl: "https://api.x.ai/v1",
    supportedModels: ["grok-4-latest"],
  },
  {
    id: "perplexity",
    providerId: "perplexity",
    labelZh: "Perplexity 官方 API",
    labelEn: "Perplexity official API",
    descriptionZh: "Perplexity Sonar 的 OpenAI-compatible 对话接口。",
    descriptionEn: "Perplexity Sonar's OpenAI-compatible conversation API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "perplexity",
    vendorKey: "perplexity",
    vendorName: "Perplexity",
    baseUrl: "https://api.perplexity.ai",
    supportedModels: ["sonar"],
  },
  {
    id: "openrouter-openai",
    providerId: "openrouter",
    labelZh: "OpenRouter",
    labelEn: "OpenRouter",
    descriptionZh: "多模型 OpenAI-compatible 聚合接口。",
    descriptionEn: "Multi-model OpenAI-compatible aggregation API.",
    category: "aggregator",
    compatibility: "openai",
    preset: "openrouter-openai",
    vendorKey: "openrouter",
    vendorName: "OpenRouter",
    baseUrl: "https://openrouter.ai/api",
    supportedModels: ["google/gemma-4-26b-a4b-it:free"],
  },
  {
    id: "poe-openai",
    providerId: "poe",
    labelZh: "Poe API",
    labelEn: "Poe API",
    descriptionZh: "Poe 的 OpenAI-compatible 多 Bot 聚合接口。",
    descriptionEn: "Poe's OpenAI-compatible multi-bot aggregation API.",
    category: "aggregator",
    compatibility: "openai",
    preset: "poe-openai",
    vendorKey: "poe",
    vendorName: "Poe",
    baseUrl: "https://api.poe.com",
    supportedModels: ["Claude-Sonnet-4.6"],
  },
  {
    id: "longcat-openai",
    providerId: "longcat",
    labelZh: "LongCat 官方 API",
    labelEn: "LongCat official API",
    descriptionZh: "LongCat 的 OpenAI-compatible 官方模型接口。",
    descriptionEn: "LongCat's official OpenAI-compatible model API.",
    category: "mainstream",
    compatibility: "openai",
    preset: "longcat-openai",
    vendorKey: "longcat",
    vendorName: "LongCat",
    baseUrl: "https://api.longcat.chat/openai",
    supportedModels: ["LongCat-2.0"],
  },
  {
    id: "linkup",
    providerId: "linkup",
    labelZh: "Linkup Search",
    labelEn: "Linkup Search",
    descriptionZh: "搜索、抓取、研究与余额查询接口。",
    descriptionEn: "Search, fetch, research, and balance APIs.",
    category: "search",
    compatibility: "search",
    preset: "linkup",
    vendorKey: "linkup",
    vendorName: "Linkup",
    baseUrl: "https://api.linkup.so",
    supportedModels: ["linkup-search", "linkup-fetch", "linkup-research", "linkup-balance"],
  },
  {
    id: "tavily",
    providerId: "tavily",
    labelZh: "Tavily Search",
    labelEn: "Tavily Search",
    descriptionZh: "Tavily 官方搜索接口。",
    descriptionEn: "Tavily's official search API.",
    category: "search",
    compatibility: "search",
    preset: "tavily",
    vendorKey: "tavily",
    vendorName: "Tavily",
    baseUrl: "https://api.tavily.com",
    supportedModels: ["tavily-search"],
  },
  {
    id: "you",
    providerId: "you",
    labelZh: "You.com Search",
    labelEn: "You.com Search",
    descriptionZh: "You.com 官方搜索接口。",
    descriptionEn: "You.com's official search API.",
    category: "search",
    compatibility: "search",
    preset: "you",
    vendorKey: "you",
    vendorName: "You.com",
    baseUrl: "https://ydc-index.io",
    supportedModels: ["you-search"],
  },
  {
    id: "exa",
    providerId: "exa",
    labelZh: "Exa Search",
    labelEn: "Exa Search",
    descriptionZh: "Exa 官方搜索与内容抓取接口。",
    descriptionEn: "Exa's official search and content fetch APIs.",
    category: "search",
    compatibility: "search",
    preset: "exa",
    vendorKey: "exa",
    vendorName: "Exa",
    baseUrl: "https://api.exa.ai",
    supportedModels: ["exa-search", "exa-fetch"],
  },
  {
    id: "jina-search",
    providerId: "jina-search",
    labelZh: "Jina AI Search",
    labelEn: "Jina AI Search",
    descriptionZh: "Jina AI 官方搜索接口。",
    descriptionEn: "Jina AI's official search API.",
    category: "search",
    compatibility: "search",
    preset: "jina-search",
    vendorKey: "jina-ai",
    vendorName: "Jina AI",
    baseUrl: "https://s.jina.ai",
    supportedModels: ["jina-search"],
  },
  {
    id: "jina-reader",
    providerId: "jina-reader",
    labelZh: "Jina AI Reader",
    labelEn: "Jina AI Reader",
    descriptionZh: "Jina AI 官方网页读取接口。",
    descriptionEn: "Jina AI's official web reader API.",
    category: "search",
    compatibility: "search",
    preset: "jina-reader",
    vendorKey: "jina-ai",
    vendorName: "Jina AI",
    baseUrl: "https://r.jina.ai",
    supportedModels: ["jina-fetch"],
  },
  {
    id: "websearchapi",
    providerId: "websearchapi",
    labelZh: "WebSearchAPI",
    labelEn: "WebSearchAPI",
    descriptionZh: "WebSearchAPI 官方搜索接口。",
    descriptionEn: "WebSearchAPI's official search API.",
    category: "search",
    compatibility: "search",
    preset: "websearchapi",
    vendorKey: "websearchapi",
    vendorName: "WebSearchAPI",
    baseUrl: "https://api.websearchapi.ai",
    supportedModels: ["websearchapi-search"],
  },
  {
    id: "muyuan-openai",
    providerId: "muyuan-openai",
    labelZh: "Muyuan · 第三方 OpenAI 兼容",
    labelEn: "Muyuan · Third-party OpenAI-compatible",
    descriptionZh: "预配置的第三方 OpenAI-compatible 服务；保留独立渠道 ID 与额度池。",
    descriptionEn: "Preconfigured third-party OpenAI-compatible service with an independent channel ID and quota pool.",
    category: "third-party-compatible",
    compatibility: "openai",
    preset: "muyuan-openai",
    vendorKey: "muyuan",
    vendorName: "Muyuan",
    baseUrl: "https://muyuan.do/v1",
    supportedModels: ["gpt-5.4"],
  },
  {
    id: "custom-api-provider",
    providerId: "custom-api-provider",
    labelZh: "自定义 API 服务商",
    labelEn: "Custom API provider",
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

export function providerCatalogClassification(
  providerId: string,
  preset: string | null | undefined,
  adapter?: string | null,
  protocolProfile?: string | null,
): ProviderCatalogClassification | null {
  const normalizedProviderId = providerId.trim().toLowerCase();
  const normalizedPreset = preset?.trim().toLowerCase() ?? "";
  if (normalizedProviderId === "muyuan-openai" || normalizedPreset === "muyuan-openai") {
    return {
      category: "third-party-compatible",
      compatibility: "openai",
    };
  }
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
  return provider;
}
