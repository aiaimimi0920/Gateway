import { expect, it } from "vitest";
import { groupCardModels, MODEL_COMPANY_RULES } from "./modelDisplayCatalog";

it.each([
  ["gpt-image-next", "openai", "OpenAI"], ["dall-e-next", "openai", "OpenAI"],
  ["claude-next", "anthropic", "Claude"], ["grok-next", "xai", "Grok"],
  ["Gemma-Next", "google", "Google"], ["google/diffusiongemma-next", "google", "Google"],
  ["nano-banana", "google", "Google"], ["google-nano-banana-pro", "google", "Google"],
  ["meta-llama/llama3.1-8B", "meta", "Meta"], ["muse-spark-next", "meta", "Meta"],
  ["nvidia/llama-9-nemotron-next", "nvidia", "NVIDIA"],
  ["nvidia/nemotron-new:free", "nvidia", "NVIDIA"],
  ["nv-mistralai/mistral-nemo-12b-instruct", "mistral-nvidia", "Mistral AI / NVIDIA"],
  ["codestral-next", "mistral", "Mistral AI"], ["voxtral-next", "mistral", "Mistral AI"],
  ["labs-leanstral-next", "mistral", "Mistral AI"], ["ministral-next", "mistral", "Mistral AI"],
  ["MiniMax-M3", "minimax", "MiniMax"], ["abab5.5-chat", "minimax", "MiniMax"],
  ["智谱/GLM-5.2", "zai", "GLM"], ["ibm/granite-next", "ibm", "Granite"],
  ["microsoft/kosmos-next", "microsoft", "Microsoft"],
  ["doubao-seed-next", "bytedance", "ByteDance"], ["seedream-next", "bytedance", "ByteDance"],
  ["Ling-3.1-Flash", "inclusion", "InclusionAI"], ["inclusionai/ring-next", "inclusion", "InclusionAI"],
  ["stepfun-ai/step-next", "stepfun", "Step"], ["SenseNova-next", "sensetime", "SenseNova"],
  ["longcat-next", "meituan", "LongCat"], ["laguna-next", "poolside", "Laguna"],
  ["jev-latest", "typesafe", "Jev"], ["xophunyuanocr", "tencent", "Hunyuan"],
  ["xop35qwen2b", "qwen", "Qwen"], ["01-ai/yi-large", "01ai", "Yi"],
  ["abacusai/dracarys-llama-next", "abacus", "Dracarys"], ["adept/fuyu-next", "adept", "Fuyu"],
  ["ai21labs/jamba-next", "ai21", "AI21"], ["aisingapore/sea-lion-next", "aisingapore", "SEA-LION"],
  ["baai/bge-m3", "baai", "BGE"], ["bigcode/starcoder2-15b", "bigcode", "StarCoder"],
  ["databricks/dbrx-instruct", "databricks", "DBRX"], ["sarvamai/sarvam-m", "sarvam", "Sarvam"],
  ["snowflake/arctic-embed-l", "snowflake", "Arctic"], ["stockmark/stockmark-next", "stockmark", "Stockmark"],
  ["upstage/solar-next", "upstage", "Solar"], ["writer/palmyra-next", "writer", "Palmyra"],
  ["zyphra/zamba2-next", "zyphra", "Zyphra"], ["jina-embeddings-v4", "jina", "Jina"],
  ["spark-x2.5-4b", "iflytek", "Spark"], ["uni-1", "luma", "Luma"], ["ray3.14", "luma", "Luma"],
  ["chirp-v3-5", "suno", "Suno"], ["udio-music", "udio", "Udio"],
  ["elevenlabs-music-v1", "elevenlabs", "ElevenLabs"], ["ideogram", "ideogram", "Ideogram"],
  ["midjourney-7", "midjourney", "Midjourney"],
])("classifies %s without rewriting its ID", (model, id, label) => {
  expect(groupCardModels([model])).toEqual([{ id, label, models: [model] }]);
});

it("keeps all reviewed exact models and namespace aliases under their declared owner", () => {
  for (const company of MODEL_COMPANY_RULES) {
    for (const model of company.models) {
      for (const alias of [model, ...company.namespaces.map((namespace) => `${namespace}/${model}`)]) {
        expect(groupCardModels([alias])[0].id).toBe(company.id);
        expect(groupCardModels([`${alias}:free`])[0].id).toBe(company.id);
      }
    }
  }
});

it("does not confuse tools, anonymous aliases, conflicting namespaces or model substrings with developers", () => {
  const tools = ["exa-search", "openrouter-random", "producer:standard", "suno-image-live", "jina-fetch"];
  expect(groupCardModels(tools)).toEqual([{ id: "services", label: "工具与服务", models: tools }]);
  const unknown = ["big-pickle", "space-bunny", "Meow", "42-pro-0721-free", "exo",
    "custom/phi-next", "meta/llama-9-nemotron-next", "google/claude-next", "my-qwen-proxy",
    "xop-unknown-qwen", "proxy/google/gemma-next", "starcoder2/not-a-model", "gemma-next/extra/path"];
  expect(groupCardModels(unknown)).toEqual([{ id: "other", label: "other", models: unknown }]);
});
