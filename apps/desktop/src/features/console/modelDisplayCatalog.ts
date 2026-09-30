/** Release-reviewed editorial order, not a live popularity measurement.
 * Update with docs/model-display-catalog-release.md before each release.
 * Exact model aliases only: a known namespace does not classify unknown models.
 */
export const MODEL_DISPLAY_CATALOG_REVIEWED_AT = "2026-09-27";

export type ModelCompanyRule = {
  id: string;
  label: string;
  namespaces: readonly string[];
  models: readonly string[];
};

export const MODEL_COMPANY_RULES: readonly ModelCompanyRule[] = [
  { id: "openai", label: "OpenAI", namespaces: ["openai"], models: [
    "gpt-6-astra", "gpt-6-sol", "gpt-6-luna", "gpt-5.6", "gpt-5.4", "gpt-5",
    "gpt-oss-120b", "gpt-oss-20b", "o3", "o4-mini", "gpt-4.1", "gpt-4o",
  ] },
  { id: "anthropic", label: "Anthropic", namespaces: ["anthropic"], models: [
    "claude-opus-4-6", "claude-sonnet-4-6", "claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5",
  ] },
  { id: "google", label: "Google", namespaces: ["google"], models: [
    "gemini-3.1-pro-preview", "gemini-3-pro-preview", "gemini-3-flash-preview", "gemini-2.5-pro", "gemini-2.5-flash",
    "gemma-4-31b-it", "gemma-3-27b-it", "gemma-3-12b-it", "gemma-3-4b-it",
    "gemma-3n-e4b-it", "gemma-3n-e2b-it", "gemma-2-2b-it", "gemma-2b",
    "codegemma-7b", "codegemma-1.1-7b", "recurrentgemma-2b", "deplot",
  ] },
  { id: "meta", label: "Meta", namespaces: ["meta"], models: [
    "llama-4-maverick-17b-128e-instruct", "llama-3.3-70b-instruct", "llama-3.1-405b-instruct",
    "llama-3.1-70b-instruct", "llama-3.1-8b-instruct", "llama-3.2-90b-vision-instruct",
    "llama-3.2-11b-vision-instruct", "llama-3.2-3b-instruct", "llama-3.2-1b-instruct",
    "llama-guard-4-12b", "codellama-70b", "llama2-70b",
  ] },
  { id: "deepseek", label: "DeepSeek", namespaces: ["deepseek-ai", "deepseek"], models: [
    "deepseek-v3.2", "deepseek-v3.1-terminus", "deepseek-r1", "deepseek-v3", "deepseek-coder-6.7b-instruct",
  ] },
  { id: "qwen", label: "Alibaba / Qwen", namespaces: ["qwen"], models: [
    "qwen3.5-397b-a17b", "qwen3.5-122b-a10b", "qwen3-coder-480b-a35b-instruct",
    "qwen3-next-80b-a3b-thinking", "qwen3-next-80b-a3b-instruct", "qwen2.5-coder-32b-instruct",
  ] },
  { id: "xai", label: "xAI", namespaces: ["x-ai", "xai"], models: ["grok-4", "grok-3", "grok-3-mini"] },
  { id: "nvidia", label: "NVIDIA", namespaces: ["nvidia"], models: [
    "nemotron-3-super-120b-a12b", "nemotron-3-nano-30b-a3b", "nemotron-nano-3-30b-a3b",
    "llama-3.3-nemotron-super-49b-v1.5", "llama-3.3-nemotron-super-49b-v1",
    "llama-3.1-nemotron-ultra-253b-v1", "llama-3.1-nemotron-70b-instruct",
    "llama-3.1-nemotron-51b-instruct", "llama-3.1-nemotron-nano-8b-v1",
    "nemotron-nano-12b-v2-vl", "llama-3.1-nemotron-nano-vl-8b-v1",
    "nemotron-4-340b-instruct", "nemotron-4-340b-reward", "nemotron-mini-4b-instruct",
    "nvidia-nemotron-nano-9b-v2", "cosmos-reason2-8b", "nemotron-parse", "nemoretriever-parse",
    "llama-nemotron-embed-1b-v2", "llama-nemotron-embed-vl-1b-v2", "nv-embed-v1",
    "nv-embedcode-7b-v1", "nv-embedqa-e5-v5", "nv-embedqa-mistral-7b-v2", "embed-qa-4",
    "llama-3.2-nv-embedqa-1b-v1", "llama-3.2-nv-embedqa-1b-v2",
    "llama-3.2-nemoretriever-1b-vlm-embed-v1", "llama-3.2-nemoretriever-300m-embed-v1",
    "nemotron-content-safety-reasoning-4b", "nemotron-3-content-safety", "gliner-pii",
    "llama-3.1-nemoguard-8b-content-safety", "llama-3.1-nemoguard-8b-topic-control",
    "llama-3.1-nemotron-safety-guard-8b-v3", "riva-translate-4b-instruct-v1.1",
    "riva-translate-4b-instruct", "ising-calibration-1-35b-a3b", "mistral-nemo-minitron-8b-8k-instruct",
    "llama3-chatqa-1.5-70b", "nvclip", "neva-22b", "vila",
  ] },
  { id: "mistral", label: "Mistral AI", namespaces: ["mistralai"], models: [
    "mistral-large-3-675b-instruct-2512", "mistral-medium-3-instruct", "mistral-small-4-119b-2603",
    "devstral-2-123b-instruct-2512", "magistral-small-2506", "ministral-14b-instruct-2512",
    "mistral-large-2-instruct", "mistral-large", "codestral-22b-instruct-v0.1",
    "mixtral-8x22b-instruct-v0.1", "mixtral-8x22b-v0.1", "mixtral-8x7b-instruct-v0.1",
    "mistral-7b-instruct-v0.3", "mistral-nemotron",
  ] },
  { id: "moonshot", label: "Moonshot / Kimi", namespaces: ["moonshotai"], models: [
    "kimi-k2.5", "kimi-k2-thinking", "kimi-k2-instruct-0905", "kimi-k2-instruct",
  ] },
  { id: "minimax", label: "MiniMax", namespaces: ["minimaxai"], models: ["minimax-m2.7", "minimax-m2.5"] },
  { id: "zai", label: "Z.ai", namespaces: ["z-ai"], models: ["glm-5.1", "glm5", "glm4.7"] },
  { id: "microsoft", label: "Microsoft", namespaces: ["microsoft"], models: [
    "phi-4-multimodal-instruct", "phi-4-mini-instruct", "phi-3.5-moe-instruct", "phi-3-vision-128k-instruct", "kosmos-2",
  ] },
  { id: "ibm", label: "IBM", namespaces: ["ibm"], models: [
    "granite-3.0-8b-instruct", "granite-3.0-3b-a800m-instruct", "granite-34b-code-instruct", "granite-8b-code-instruct",
  ] },
];

export type CardModelGroup = { id: string; label: string | null; models: string[] };

const rankedModels = new Map<string, { company: ModelCompanyRule; rank: number }>();
for (const company of MODEL_COMPANY_RULES) {
  company.models.forEach((model, rank) => {
    for (const alias of [model, ...company.namespaces.map((namespace) => `${namespace}/${model}`)]) {
      rankedModels.set(alias.toLowerCase(), { company, rank });
    }
  });
}

/** Intersect the release catalogue with configured capabilities; never add models. */
export function groupCardModels(models: readonly string[]): CardModelGroup[] {
  const groups = new Map<string, CardModelGroup>();
  const unknown: CardModelGroup = { id: "unclassified", label: null, models: [] };
  for (const model of new Set(models.map((value) => value.trim()).filter(Boolean))) {
    const rule = rankedModels.get(model.toLowerCase());
    if (!rule) {
      unknown.models.push(model);
      continue;
    }
    const group = groups.get(rule.company.id) ?? { id: rule.company.id, label: rule.company.label, models: [] };
    group.models.push(model);
    groups.set(group.id, group);
  }
  const ordered = MODEL_COMPANY_RULES.flatMap((company) => {
    const group = groups.get(company.id);
    if (!group) return [];
    // Stable ties retain the capability list's order, including unknown models.
    group.models.sort((a, b) => rankedModels.get(a.toLowerCase())!.rank - rankedModels.get(b.toLowerCase())!.rank);
    return [group];
  });
  return unknown.models.length ? [...ordered, unknown] : ordered;
}
