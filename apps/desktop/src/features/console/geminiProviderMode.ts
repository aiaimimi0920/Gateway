// Presets own credential/execution protocols; names are only a legacy fallback.
export type GeminiProviderMode = "web" | "canvas" | "canvas-chat" | "business";

export function geminiProviderMode(providerId: string, preset: string | null, label = ""): GeminiProviderMode | null {
  const resolve = (value: string): GeminiProviderMode | null => {
    if (["gemini-canvas", "gemini-canvas-browser-relay", "gemini-canvas-program-relay"].includes(value)) return "canvas";
    if (value === "gemini-canvas-chat") return "canvas-chat";
    if (value === "gemini-business") return "business";
    if (["gemini-web", "gemini-web-chat", "gemini-web-chat-modular", "gemini-web-secondary"].includes(value)) return "web";
    return null;
  };
  if (preset?.trim()) return resolve(preset.trim().toLowerCase());
  return resolve(providerId.trim().toLowerCase()) ?? resolve(label.trim().toLowerCase());
}
