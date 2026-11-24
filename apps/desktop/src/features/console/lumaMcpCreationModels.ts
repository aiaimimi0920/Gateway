// Reviewed 2026-11-20 list/describe snapshot; shared with Rust write admission.
import profiles from "../../../../../manifests/luma-mcp-creation-models.json";
export type McpModel = keyof typeof profiles;
export type ModelParameter = { type: string; enum?: string[]; minimum?: number; maximum?: number };
export type ModelProfile = {
  label: string; kind: string; ratios: string[]; maxReferences: number; parameters: Record<string, ModelParameter>;
};
export const mcpModels = profiles as Record<McpModel, ModelProfile>;
export const isMcpVideo = (model: McpModel) => mcpModels[model].kind === "video";
export const imageParameterFields = {
  resolution: "imageResolution", quality: "quality", output_format: "format", background: "background",
  size: "size", thinking_level: "thinkingLevel", enable_search: "enableSearch", temperature: "temperature", magic_prompt: "magicPrompt",
} as const;
export const imageParameterNames = {
  resolution: "resolution", quality: "quality", output_format: "outputFormat", background: "background",
  size: "size", thinking_level: "thinkingLevel", enable_search: "enableSearch", temperature: "temperature", magic_prompt: "magicPrompt",
} as const;
export function validMcpSize(size: string) {
  if (["1K", "1.5K", "2K"].includes(size)) return true;
  if (!/^\d+x\d+$/.test(size) || size.length > 32) return false;
  const [width, height] = size.split("x").map(Number);
  return Number.isSafeInteger(width * height) && width * height >= 921600 && width * height <= 4624220
    && width <= height * 16 && height <= width * 16;
}
