/**
 * One display name per provider, resolved from the recognizable brand of the
 * upstream site instead of whatever slug or free-text label the config happens
 * to carry.
 *
 * Raw configs mix conventions: some providers only have an id (`chataibot`,
 * `jina-reader`), some carry a hand-typed label with routing details baked in
 * (`Gemini Web /u/1/`), and some repeat their protocol suffix
 * (`qwen-dashscope-openai`). Every console surface — provider cards, account
 * cards, entitlement scope tabs, member rows — reads the same resolved label, so
 * the normalization lives here rather than at each render site.
 */

/**
 * Authored brand per provider id. Ids win over presets because a provider can
 * reuse a generic preset (`openrouter` and `xfyun-maas` both run on the `openai`
 * preset) while belonging to a completely different brand.
 */
const BRAND_BY_PROVIDER_ID: Record<string, string> = {
  // Catalog templates.
  openai: "OpenAI",
  anthropic: "Anthropic",
  "gemini-api": "Google Gemini",
  "azure-openai": "Azure OpenAI",
  "cohere-chat": "Cohere",
  "groq-openai": "Groq",
  "nvidia-openai": "NVIDIA NIM",
  "together-openai": "Together AI",
  "deepseek-openai": "DeepSeek",
  "mistral-openai": "Mistral AI",
  "qwen-dashscope-openai": "Qwen DashScope",
  "xai-openai": "xAI",
  perplexity: "Perplexity",
  "openrouter-openai": "OpenRouter",
  "poe-openai": "Poe",
  "longcat-openai": "LongCat",
  "jina-search": "Jina Search",
  "jina-reader": "Jina Reader",
  "muyuan-openai": "Muyuan",

  // Deployed providers whose id is the only stable handle we get.
  accio: "Accio",
  chataibot: "ChatAIBot",
  codex: "OpenAI Codex",
  exa: "Exa",
  "gemini-business": "Gemini Business",
  "gemini-canvas": "Gemini Canvas",
  "gemini-canvas-chat": "Gemini Canvas Chat",
  linkup: "Linkup",
  longcat: "LongCat",
  lumalabs: "Luma AI",
  nvidia: "NVIDIA NIM",
  openrouter: "OpenRouter",
  poe: "Poe",
  producer: "Producer",
  "qwen-coding-plan-anthropic": "Qwen Coding Plan · Anthropic",
  "qwen-coding-plan-openai": "Qwen Coding Plan · OpenAI",
  "qwen-web-chat": "Qwen Chat",
  suno: "Suno",
  tavily: "Tavily",
  udio: "Udio",
  websearchapi: "WebSearchAPI",
  "xfyun-maas": "iFlytek MaaS",
  you: "You.com",
};

/**
 * Brand per preset, for providers the deployment named itself. A preset match
 * keeps whatever the id adds on top as a qualifier, so two Gemini Web providers
 * stay distinguishable.
 */
const BRAND_BY_PRESET: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  "gemini-api": "Google Gemini",
  "gemini-business": "Gemini Business",
  "gemini-web-chat": "Gemini Web",
  "gemini-web-chat-modular": "Gemini Web",
  "gemini-canvas-program-relay": "Gemini Canvas",
  "gemini-canvas-chat": "Gemini Canvas Chat",
  "azure-openai": "Azure OpenAI",
  "cohere-chat": "Cohere",
  "groq-openai": "Groq",
  "nvidia-openai": "NVIDIA NIM",
  "together-openai": "Together AI",
  "deepseek-openai": "DeepSeek",
  "mistral-openai": "Mistral AI",
  "qwen-dashscope-openai": "Qwen DashScope",
  "qwen-coding-plan-openai": "Qwen Coding Plan · OpenAI",
  "qwen-coding-plan-anthropic": "Qwen Coding Plan · Anthropic",
  "qwen-web-chat": "Qwen Chat",
  "xai-openai": "xAI",
  perplexity: "Perplexity",
  "openrouter-openai": "OpenRouter",
  "poe-openai": "Poe",
  "longcat-openai": "LongCat",
  linkup: "Linkup",
  tavily: "Tavily",
  you: "You.com",
  exa: "Exa",
  "jina-search": "Jina Search",
  "jina-reader": "Jina Reader",
  websearchapi: "WebSearchAPI",
  accio: "Accio",
  chataibot: "ChatAIBot",
  codex: "OpenAI Codex",
  suno: "Suno",
  udio: "Udio",
  lumalabs: "Luma AI",
  producer: "Producer",
  "muyuan-openai": "Muyuan",
};

/** Tokens whose brand casing is not "capitalize the first letter". */
const TOKEN_CASING: Record<string, string> = {
  ai: "AI",
  api: "API",
  asr: "ASR",
  cn: "CN",
  eu: "EU",
  gpt: "GPT",
  http: "HTTP",
  id: "ID",
  llm: "LLM",
  maas: "MaaS",
  nim: "NIM",
  ocr: "OCR",
  openai: "OpenAI",
  sdk: "SDK",
  tts: "TTS",
  us: "US",
  url: "URL",
  vip: "VIP",
  xai: "xAI",
};

function splitTokens(value: string): string[] {
  return value
    .split(/[^\p{L}\p{N}]+/u)
    .map((token) => token.trim())
    .filter((token) => token.length > 0);
}

function titleCaseToken(token: string): string {
  const casing = TOKEN_CASING[token.toLowerCase()];
  if (casing) {
    return casing;
  }
  // Leave anything the config already mixed-cased alone (`DashScope`, `xAI`).
  if (/[a-z]/.test(token) && /[A-Z]/.test(token)) {
    return token;
  }
  if (/^\p{Lu}/u.test(token) && token.length > 1) {
    return token;
  }
  return token.charAt(0).toUpperCase() + token.slice(1);
}

function prettifyTokens(tokens: readonly string[]): string {
  return tokens.map(titleCaseToken).join(" ");
}

/**
 * Whatever the id says on top of the brand, e.g. `gemini-web-secondary` against
 * the `gemini-web-chat-modular` preset keeps "Secondary". Tokens the brand or
 * the preset already covers drop out.
 */
function idQualifier(providerId: string, brand: string, preset: string | null): string {
  const covered = new Set(
    [...splitTokens(brand), ...splitTokens(preset ?? "")].map((token) => token.toLowerCase()),
  );
  const leftover = splitTokens(providerId).filter((token) => !covered.has(token.toLowerCase()));
  return leftover.length > 0 ? prettifyTokens(leftover) : "";
}

function withQualifier(brand: string, qualifier: string): string {
  return qualifier.length > 0 ? `${brand} · ${qualifier}` : brand;
}

/** "Google / Gemini" and "Alibaba / Qwen" read as one brand in the console. */
function prettifyVendorName(vendorName: string): string {
  return prettifyTokens(splitTokens(vendorName));
}

export type ProviderBrandInput = {
  providerId: string;
  providerPreset?: string | null;
  /** The label the config carries, used only when nothing else resolves. */
  providerLabel?: string | null;
  /** Vendor grouping from the backend, e.g. "Google / Gemini". */
  vendorName?: string | null;
};

/**
 * The display name for a provider: an authored brand when we recognize the id or
 * preset, then the backend vendor name, then a tidied id. Never empty as long as
 * the provider has an id.
 */
export function providerBrandLabel({
  providerId,
  providerPreset = null,
  providerLabel = null,
  vendorName = null,
}: ProviderBrandInput): string {
  const id = providerId.trim();
  const preset = providerPreset?.trim() ? providerPreset.trim() : null;
  const label = providerLabel?.trim() ? providerLabel.trim() : null;

  if (id.length === 0) {
    return label ?? prettifyVendorName(vendorName ?? "");
  }

  const byId = BRAND_BY_PROVIDER_ID[id.toLowerCase()];
  if (byId) {
    return byId;
  }

  const byPreset = preset ? BRAND_BY_PRESET[preset.toLowerCase()] : undefined;
  if (byPreset) {
    return withQualifier(byPreset, idQualifier(id, byPreset, preset));
  }

  // Nothing recognized the provider, so a hand-written label is the operator's
  // own naming and outranks anything we could synthesize from the slug.
  if (label && label.toLowerCase() !== id.toLowerCase()) {
    return label;
  }

  const vendorBrand = vendorName?.trim() ? prettifyVendorName(vendorName) : "";
  if (vendorBrand.length > 0) {
    return withQualifier(vendorBrand, idQualifier(id, vendorBrand, preset));
  }

  const idBrand = prettifyTokens(splitTokens(id));
  return idBrand.length > 0 ? idBrand : (label ?? id);
}
