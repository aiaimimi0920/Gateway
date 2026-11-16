// Runtime product owners, not authentication methods or evidence of upstream acceptance.
// Keep preset bindings aligned with protocol::call_flow::SpecialAdapter.
export const SPECIAL_ADAPTERS = {
  none: ["无（标准 API）", "None (standard API)", ""],
  basispoints: ["Basispoints", "Basispoints", "chatgpt-codex-oauth-official-api"],
  gemini_web: ["Gemini Web", "Gemini Web", "gemini-web-chat"],
  gemini_canvas: ["Gemini Canvas", "Gemini Canvas", "gemini-canvas-browser-relay"],
  chatgpt_web: ["ChatGPT Web", "ChatGPT Web", "chatgpt-web-reverse"],
  aistudio_web: ["AI Studio Web", "AI Studio Web", "aistudio-web-reverse"],
  flowmusic_web: ["FlowMusic Web", "FlowMusic Web", "producer"],
  flow_web: ["Google Flow Web", "Google Flow Web", "flow-web"],
  stitch_web: ["Google Stitch Web", "Google Stitch Web", "stitch-web"],
  antigravity_oauth: ["Antigravity OAuth", "Antigravity OAuth", "antigravity-oauth"],
  accio: ["Accio", "Accio", "accio"],
  kiro: ["Kiro", "Kiro", "kiro"],
  freebuff: ["FreeBuff", "FreeBuff", "freebuff"],
  grok_web: ["Grok Web", "Grok Web", "grok"],
  qwen_web: ["Qwen Web", "Qwen Web", "qwen-web-chat"],
  chataibot_web: ["ChatAIBot Web", "ChatAIBot Web", "chataibot"],
  lumalabs_web: ["LumaLabs Web", "LumaLabs Web", "lumalabs"],
  suno_web: ["Suno Web", "Suno Web", "suno"],
  udio_web: ["Udio Web", "Udio Web", "udio"],
  perplexity_web: ["Perplexity Web", "Perplexity Web", "perplexity-web"],
} as const;

export type SpecialAdapter = keyof typeof SPECIAL_ADAPTERS;

/** Basispoints is optional on the Codex preset; never infer it from that preset alone. */
export function specialAdapterForPreset(value: unknown): SpecialAdapter | null {
  if (typeof value !== "string") return null;
  const preset = ["qwen-web", "qwen-webui", "qwen-webui-replay", "qwen-webui-replay-live"].includes(value.trim())
    ? "qwen-web-chat" : value.trim();
  const entry = Object.entries(SPECIAL_ADAPTERS).find(([key, item]) => key !== "none" && key !== "basispoints" && item[2] === preset);
  return entry ? entry[0] as SpecialAdapter : null;
}

/** An API-profile edit must not replace the identity of a product-owned sender. */
export function specialAdapterOwnsFingerprint(value: unknown): boolean {
  return typeof value === "string" && value !== "none" && Object.hasOwn(SPECIAL_ADAPTERS, value);
}
