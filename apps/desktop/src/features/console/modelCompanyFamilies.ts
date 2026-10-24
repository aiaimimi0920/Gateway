/** Display ownership rules, not proof of a relay's actual weights or availability.
 * Single-family developers use the model brand; multi-family developers use the company.
 * See docs/model-company-ownership.md for evidence and unresolved relay aliases.
 */
import type { ModelCompanyRule } from "./modelDisplayCatalog";

export const ADDITIONAL_MODEL_COMPANIES: readonly ModelCompanyRule[] = [
  { id: "bytedance", label: "ByteDance", namespaces: ["bytedance", "bytedance-seed"], models: [] },
  { id: "inclusion", label: "InclusionAI", namespaces: ["inclusionai", "antgroup"], models: [] },
  { id: "stepfun", label: "Step", namespaces: ["stepfun", "stepfun-ai"], models: [] },
  { id: "sensetime", label: "SenseNova", namespaces: ["sensetime"], models: [] },
  { id: "meituan", label: "LongCat", namespaces: ["meituan", "meituan-longcat"], models: [] },
  { id: "poolside", label: "Laguna", namespaces: ["poolside"], models: [] },
  { id: "typesafe", label: "Jev", namespaces: ["typesafe", "typesafe-ai"], models: [] },
  { id: "tencent", label: "Hunyuan", namespaces: ["tencent", "tencent-hunyuan"], models: [] },
  { id: "01ai", label: "Yi", namespaces: ["01-ai"], models: [] },
  { id: "abacus", label: "Dracarys", namespaces: ["abacusai"], models: [] },
  { id: "adept", label: "Fuyu", namespaces: ["adept"], models: [] },
  { id: "ai21", label: "AI21", namespaces: ["ai21labs", "ai21"], models: [] },
  { id: "aisingapore", label: "SEA-LION", namespaces: ["aisingapore"], models: [] },
  { id: "baai", label: "BGE", namespaces: ["baai"], models: [] },
  { id: "bigcode", label: "StarCoder", namespaces: ["bigcode"], models: [] },
  { id: "databricks", label: "DBRX", namespaces: ["databricks"], models: [] },
  { id: "sarvam", label: "Sarvam", namespaces: ["sarvamai"], models: [] },
  { id: "snowflake", label: "Arctic", namespaces: ["snowflake"], models: [] },
  { id: "stockmark", label: "Stockmark", namespaces: ["stockmark"], models: [] },
  { id: "upstage", label: "Solar", namespaces: ["upstage"], models: [] },
  { id: "writer", label: "Palmyra", namespaces: ["writer"], models: [] },
  { id: "zyphra", label: "Zyphra", namespaces: ["zyphra"], models: [] },
  { id: "jina", label: "Jina", namespaces: ["jinaai", "jina"], models: [] },
  { id: "iflytek", label: "Spark", namespaces: ["iflytek", "xfyun"], models: [] },
  { id: "luma", label: "Luma", namespaces: ["luma", "lumalabs"], models: [] },
  { id: "suno", label: "Suno", namespaces: ["suno"], models: [] },
  { id: "udio", label: "Udio", namespaces: ["udio"], models: [] },
  { id: "elevenlabs", label: "ElevenLabs", namespaces: ["elevenlabs"], models: [] },
  { id: "ideogram", label: "Ideogram", namespaces: ["ideogram"], models: ["ideogram"] },
  { id: "midjourney", label: "Midjourney", namespaces: ["midjourney"], models: [] },
  { id: "mistral-nvidia", label: "Mistral AI / NVIDIA", namespaces: ["nv-mistralai", "mistralai", "nvidia"], models: [] },
  // These configured IDs are tool/service adapters, not a claim of model authorship.
  { id: "services", label: "工具与服务", namespaces: [], models: [] },
];

const suffix = "[a-z0-9][a-z0-9._:-]*(?:\\[free\\])?";
const families = (...prefixes: string[]) => new RegExp(`^(?:${prefixes.join("|")})-${suffix}$`);
const rules: readonly [string, RegExp][] = [
  // Adapter smoke-test IDs do not identify which model generated the media.
  ["services", /^(?:(?:suno|udio|lumalabs)-(?:audio|music|image|video)-live|jina-(?:fetch|search))$/],
  // Derived models must win over the base model's family (e.g. Llama-Nemotron).
  ["nvidia", /^(?:llama[0-9.-]*-(?:nemotron|nemoguard|nemoretriever|nv-embedqa|chatqa)|mistral-nemo-minitron)(?:-|[0-9])/],
  ["nvidia", families("nemotron", "nvidia-nemotron", "nemoretriever", "cosmos", "nv-embed", "nv-embedcode", "nv-embedqa", "riva", "ising", "neva", "vila", "gliner", "embed-qa")],
  ["openai", families("gpt", "chatgpt", "dall-e", "sora", "whisper", "tts", "text-embedding", "omni-moderation")],
  ["openai", /^o[1-9][0-9]*(?:-[a-z0-9._:-]+)?$/],
  ["anthropic", families("claude")],
  ["google", families("gemini", "gemma", "codegemma", "recurrentgemma", "diffusiongemma", "imagen", "veo", "lyria", "google-nano-banana", "nano-banana")],
  ["google", /^(?:gemma[0-9]|nano-banana(?:-pro)?$|google-nano-banana$)/],
  ["deepseek", families("deepseek")],
  ["qwen", /^qwen(?:[0-9]|-[a-z0-9])[a-z0-9._:-]*(?:\[free\])?$/],
  ["xai", families("grok")],
  ["moonshot", families("kimi", "moonshot")],
  ["xiaomi", families("mimo")],
  ["agnes", families("agnes")],
  ["minimax", families("minimax", "abab", "hailuo", "speech")],
  ["minimax", /^abab[0-9][a-z0-9._:-]*$/],
  ["zai", /^glm/],
  ["meta", /^(?:llama|codellama)(?:-|[0-9])[a-z0-9._:-]+(?:\[free\])?$/],
  ["meta", families("muse-spark", "muse-glimmer", "muse-image", "muse-wonder")],
  ["mistral-nvidia", /^mistral-nemo-12b(?:-|$)/],
  ["mistral", families("mistral", "mixtral", "ministral", "magistral", "devstral", "codestral", "voxtral", "pixtral", "leanstral", "labs-leanstral", "open-mistral", "open-mixtral")],
  ["microsoft", families("phi", "kosmos", "mai")],
  ["ibm", families("granite")],
  ["bytedance", families("doubao", "seed", "seedream", "seedance", "bytedance-seedream")],
  ["bytedance", /^bytedance-seedream$/],
  ["inclusion", families("ling", "ring")],
  ["stepfun", families("step")],
  ["sensetime", families("sensenova")],
  ["meituan", families("longcat")],
  ["poolside", families("laguna")],
  ["typesafe", families("jev")],
  ["tencent", /^hunyuan(?:-|mt|ocr|[0-9])[a-z0-9._:-]*$/],
  ["01ai", families("yi")],
  ["abacus", families("dracarys")],
  ["adept", families("fuyu")],
  ["ai21", families("jamba", "ai21-jamba", "j2", "jurassic")],
  ["aisingapore", families("sea-lion")],
  ["baai", families("bge")],
  ["bigcode", /^starcoder[0-9]*(?:-[a-z0-9._:-]+)?$/],
  ["databricks", families("dbrx")],
  ["sarvam", families("sarvam")],
  ["snowflake", families("arctic")],
  ["stockmark", families("stockmark")],
  ["upstage", families("solar")],
  ["writer", families("palmyra")],
  ["zyphra", /^(?:zamba[0-9]*|zonos)-[a-z0-9._:-]+$/],
  ["jina", families("jina")],
  ["iflytek", families("spark")],
  ["luma", /^(?:ray(?:-|[0-9])|uni-|lumalabs-)[a-z0-9._:-]+$/],
  ["suno", families("chirp", "suno")],
  ["udio", families("udio")],
  ["elevenlabs", families("elevenlabs")],
  ["ideogram", families("ideogram")],
  ["midjourney", families("midjourney")],
  ["services", /^(?:(?:exa|linkup|tavily|websearchapi|you)-(?:search|fetch|research|balance)|chataibot-free-image-live|producer:(?:image|music-video|standard)|openrouter-random)$/],
];

// Exact native IDs established by the configured xfyun model_map, not substring guesses.
const nativeAliases: Record<string, string> = {
  xop35qwen2b: "qwen", xopqwen35v35b: "qwen",
  xophunyuan7bmt: "tencent", xophunyuanocr: "tencent",
};

export function modelFamilyCompany(name: string): string | undefined {
  if (name.length > 256 || !/^[a-z0-9][a-z0-9._:-]*(?:\[free\])?$/.test(name)) return undefined;
  return nativeAliases[name] ?? rules.find(([, pattern]) => pattern.test(name))?.[0];
}
