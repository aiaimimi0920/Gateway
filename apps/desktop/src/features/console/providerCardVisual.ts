import {
  PROVIDER_CATALOG_TEMPLATES,
  providerCatalogClassification,
  type ProviderCatalogCategory,
} from "./providerCatalog";

/**
 * Visual grouping for provider pool cards. This is broader than
 * `ProviderCatalogCategory` because `routes.yaml` also carries media and
 * browser-backed surfaces that the add-provider catalog does not template yet.
 */
export type ProviderCardCategory =
  | "mainstream"
  | "aggregator"
  | "search"
  | "media"
  | "browser"
  | "third-party-compatible";

export type ProviderCardVisual = {
  category: ProviderCardCategory;
  /** Stable local icon key. The UI never hot-links a provider website asset. */
  iconKey: string;
  /** Accessible local monogram used when no official logo asset is bundled. */
  iconLabel: string;
  /** Backwards-compatible initial used by older consumers/tests. */
  initial: string;
};

const CATEGORY_LABELS: Record<ProviderCardCategory, [string, string]> = {
  mainstream: ["主流服务", "Mainstream"],
  aggregator: ["聚合平台", "Aggregator"],
  search: ["搜索检索", "Search"],
  media: ["音视频生成", "Media"],
  browser: ["浏览器反代", "Browser"],
  "third-party-compatible": ["第三方兼容", "Third-party"],
};

const PRESET_CATEGORIES: Record<string, ProviderCardCategory> = {
  suno: "media",
  udio: "media",
  producer: "media",
  lumalabs: "media",
  "gemini-canvas": "media",
  "gemini-canvas-chat": "browser",
  "qwen-web-chat": "browser",
  chataibot: "browser",
  codex: "browser",
  accio: "search",
  linkup: "search",
  tavily: "search",
  you: "search",
  exa: "search",
  "jina-search": "search",
  "jina-reader": "search",
  websearchapi: "search",
  openrouter: "aggregator",
  "poe-openai": "aggregator",
};

const PROVIDER_ID_CATEGORIES: Record<string, ProviderCardCategory> = {
  "gemini-business": "mainstream",
  "gemini-canvas": "media",
  "gemini-canvas-chat": "browser",
  "qwen-web-chat": "browser",
};

const PROVIDER_ICON_KEYS: Record<string, { key: string; label: string }> = {
  openai: { key: "openai", label: "AI" },
  anthropic: { key: "anthropic", label: "A" },
  "gemini-api": { key: "google", label: "G" },
  "gemini-business": { key: "google", label: "G" },
  "azure-openai": { key: "azure", label: "AZ" },
  cohere: { key: "cohere", label: "C" },
  groq: { key: "groq", label: "GQ" },
  nvidia: { key: "nvidia", label: "N" },
  longcat: { key: "longcat", label: "LC" },
  together: { key: "together", label: "T" },
  deepseek: { key: "deepseek", label: "DS" },
  mistral: { key: "mistral", label: "M" },
  qwen: { key: "qwen", label: "Q" },
  "qwen-web-chat": { key: "qwen", label: "Q" },
  xai: { key: "xai", label: "x" },
  perplexity: { key: "perplexity", label: "P" },
  openrouter: { key: "openrouter", label: "OR" },
  poe: { key: "poe", label: "P" },
  linkup: { key: "linkup", label: "L" },
  tavily: { key: "tavily", label: "T" },
  you: { key: "you", label: "Y" },
  exa: { key: "exa", label: "E" },
  jina: { key: "jina", label: "J" },
  suno: { key: "suno", label: "S" },
  udio: { key: "udio", label: "U" },
};

function providerCardIcon(options: {
  providerId: string;
  providerPreset: string | null;
  providerLabel: string;
  adapter?: string | null;
}): { key: string; label: string } {
  const candidates = [
    options.providerId,
    options.providerPreset ?? "",
    options.providerLabel,
  ]
    .map((value) => value.trim().toLowerCase())
    .filter(Boolean);
  // Match an explicit brand before protocol suffixes such as nvidia-openai.
  // Looking for "openai" anywhere first incorrectly painted NVIDIA as OpenAI.
  for (const candidate of candidates) {
    const direct = PROVIDER_ICON_KEYS[candidate];
    if (direct) return direct;
  }
  const entries = Object.entries(PROVIDER_ICON_KEYS).sort(([left], [right]) => right.length - left.length);
  for (const candidate of candidates) {
    const prefix = entries.find(([key]) => candidate.startsWith(`${key}-`) || candidate.startsWith(`${key}_`));
    if (prefix) return prefix[1];
  }
  for (const candidate of candidates) {
    const matched = entries.find(([key]) => candidate.includes(key));
    if (matched) return matched[1];
  }
  if (options.adapter?.toLowerCase().includes("openai")) {
    return { key: "openai-compatible", label: "AI" };
  }
  return { key: "generic", label: providerCardInitial(options.providerLabel, options.providerId) };
}

const CATALOG_CATEGORY_BY_PROVIDER_ID: ReadonlyMap<string, ProviderCatalogCategory> = new Map(
  PROVIDER_CATALOG_TEMPLATES.map((template) => [
    template.providerId.toLowerCase(),
    template.category,
  ]),
);

export function providerCardCategoryLabel(
  category: ProviderCardCategory,
  t: (zh: string, en: string) => string,
): string {
  const [zh, en] = CATEGORY_LABELS[category];
  return t(zh, en);
}

/**
 * Pick the glyph shown in the art area. Latin identifiers use their first
 * letter; CJK labels keep their first character so the card is still
 * recognizable without a bundled logo.
 */
export function providerCardInitial(providerLabel: string, providerId: string): string {
  for (const source of [providerLabel, providerId]) {
    const match = source.match(/[\p{L}\p{N}]/u);
    if (match) {
      return match[0].toUpperCase();
    }
  }
  return "?";
}

export function providerCardVisual(options: {
  providerId: string;
  providerLabel: string;
  providerPreset: string | null;
  adapter?: string | null;
  protocolProfile?: string | null;
}): ProviderCardVisual {
  const normalizedId = options.providerId.trim().toLowerCase();
  const normalizedPreset = options.providerPreset?.trim().toLowerCase() ?? "";

  const category =
    PROVIDER_ID_CATEGORIES[normalizedId] ??
    PRESET_CATEGORIES[normalizedPreset] ??
    PRESET_CATEGORIES[normalizedId] ??
    providerCatalogClassification(
      options.providerId,
      options.providerPreset,
      options.adapter,
      options.protocolProfile,
    )?.category ??
    CATALOG_CATEGORY_BY_PROVIDER_ID.get(normalizedId) ??
    CATALOG_CATEGORY_BY_PROVIDER_ID.get(normalizedPreset) ??
    "mainstream";
  const icon = providerCardIcon(options);

  return {
    category,
    iconKey: icon.key,
    iconLabel: icon.label,
    initial: providerCardInitial(options.providerLabel, options.providerId),
  };
}
