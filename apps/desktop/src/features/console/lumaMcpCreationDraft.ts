// Structured fields preserve exact model selection and omit unselected options.
import { imageParameterFields, imageParameterNames, isMcpVideo, mcpModels, validMcpSize, type McpModel } from "./lumaMcpCreationModels";
export type { McpModel } from "./lumaMcpCreationModels";
export type McpDraft = {
  model: McpModel; operation: "generate" | "edit" | "reframe"; ratio: string; source: string; sourceType: string;
  references: string; style: string; format: string; quality: string; imageResolution: string; background: string;
  resolution: string; duration: string; hdr: string; strength: string; autoControls: string;
  startFrame: string; endFrame: string; keyframes: string; position: string;
  reuseEnabled: boolean; reuseTask: string; sourceAsset: string; referenceAssets: string;
  size: string; thinkingLevel: string; enableSearch: string; temperature: string; magicPrompt: string; loopVideo: string;
};
export const newMcpDraft = (model: McpModel = "luma_uni_1", operation: McpDraft["operation"] = "generate"): McpDraft => ({
  model, operation, ratio: "", source: "", sourceType: "video/mp4", references: "", style: "", format: "",
  quality: "", imageResolution: "", background: "", resolution: "", duration: "", hdr: "", strength: "",
  autoControls: "", startFrame: "", endFrame: "", keyframes: "", position: "",
  reuseEnabled: false, reuseTask: "", sourceAsset: "", referenceAssets: "",
  size: "", thinkingLevel: "", enableSearch: "", temperature: "", magicPrompt: "", loopVideo: "",
});
const lines = (value: string) => value.split(/\r?\n/).map(v => v.trim()).filter(Boolean);
export function publicMediaUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password && !url.hash && (!url.port || url.port === "443")
      && value.length <= 4096;
  } catch { return false; }
}
export function mcpDraftOptions(draft: McpDraft) {
  const video = isMcpVideo(draft.model), profile = mcpModels[draft.model];
  const thirdParty = !video && draft.model !== "luma_uni_1";
  const references = lines(draft.references);
  const assetReferences = draft.reuseEnabled ? lines(draft.referenceAssets) : [];
  const sourceAsset = draft.reuseEnabled ? draft.sourceAsset.trim() : "";
  const assets = [...assetReferences, ...sourceAsset ? [sourceAsset] : []];
  const keyframes = lines(draft.keyframes).map(line => {
    const [index, ...url] = line.split(/\s+/);
    return { index: /^\d+$/.test(index) ? Number(index) : NaN, url: url.join(" ") };
  });
  const parts = draft.position.trim() ? draft.position.split(",").map(v => v.trim() ? Number(v) : NaN) : [];
  const urls = [...references, ...keyframes.map(frame => frame.url), ...[draft.source, draft.startFrame, draft.endFrame].filter(Boolean)];
  const parameters: Record<string, string | boolean | number> = {};
  let parametersValid = true;
  for (const [key, field] of Object.entries(imageParameterFields)) {
    const raw = draft[field];
    if (!raw || !thirdParty) continue;
    const schema = profile.parameters[key];
    const value = schema?.type === "boolean" ? raw === "true" : schema?.type === "number" ? Number(raw) : raw;
    parametersValid &&= !!schema && (!schema.enum || schema.enum.includes(raw))
      && (schema.type !== "boolean" || ["false", "true"].includes(raw))
      && (schema.type !== "number" || raw.trim().length > 0 && typeof value === "number" && Number.isFinite(value)
        && value >= (schema.minimum ?? -Infinity) && value <= (schema.maximum ?? Infinity));
    parameters[imageParameterNames[key as keyof typeof imageParameterNames]] = value;
  }
  const anchors = !!(draft.startFrame || draft.endFrame || keyframes.length);
  const valid = parametersValid && (!draft.ratio || profile.ratios.includes(draft.ratio))
    && (!thirdParty || draft.operation === "generate")
    && (draft.model !== "luma_ray3_14_hdr" || draft.operation === "generate" && draft.hdr !== "false"
      && draft.duration !== "10s" && !["360p", "540p"].includes(draft.resolution))
    && (draft.model === "luma_ray3_2" || !keyframes.length)
    && (!draft.loopVideo || draft.model === "luma_ray3_14" && draft.operation === "generate")
    && !(draft.model === "luma_ray3_14" && draft.operation === "generate"
      && (draft.hdr === "true" || draft.duration === "10s" && (anchors || draft.loopVideo === "true")))
    && urls.every(publicMediaUrl) && (draft.operation === "generate" || !!(draft.reuseEnabled ? sourceAsset : draft.source))
    && (!draft.reuseEnabled || /^luma-mcp-[0-9a-f-]{36}$/i.test(draft.reuseTask.trim()) && assets.length > 0
      && !draft.source && assets.every(id => /^(artifact:)?[A-Za-z0-9_-]+$/.test(id) && id.length <= 64)
      && new Set(assets.map(id => id.replace(/^artifact:/, ""))).size === assets.length)
    && (!video || draft.reuseEnabled || draft.operation === "generate" || ["video/mp4", "video/webm", "video/quicktime"].includes(draft.sourceType))
    && (draft.operation !== "reframe" || !!draft.ratio)
    && references.length + assetReferences.length <= profile.maxReferences - (draft.model === "luma_uni_1" && draft.operation === "edit" ? 1 : 0)
    && keyframes.length <= 64 && new Set(keyframes.map(f => f.index)).size === keyframes.length
    && keyframes.every(f => Number.isInteger(f.index) && f.index >= 0 && f.index <= (draft.duration === "10s" ? 240 : 120))
    && !(keyframes.length && (draft.startFrame || draft.endFrame))
    && (!parts.length || parts.length === 4 && parts.every(Number.isFinite) && parts.slice(0, 2).every(n => n >= -2 && n <= 2)
      && parts.slice(2).every(n => n > 0 && n <= 2))
    && !(draft.hdr === "true" && ["360p", "540p"].includes(draft.resolution))
    && !(draft.operation === "reframe" && draft.resolution === "1080p" && ["9:16", "3:4"].includes(draft.ratio))
    && !(thirdParty && draft.background === "transparent" && draft.format === "jpeg")
    && !(draft.magicPrompt && references.length + assetReferences.length > 0)
    && !(draft.model === "ideogram_4_6" && draft.quality === "very_low" && draft.size.includes("4096"))
    && !(draft.model === "bytedance_seedream_5_flash" && draft.size && !validMcpSize(draft.size));
  const edit = { ...(draft.strength ? { strength: draft.strength } : {}),
    ...(draft.autoControls ? { autoControls: draft.autoControls === "true" } : {}) };
  const controls = {
    ...(draft.resolution ? { resolution: draft.resolution } : {}), ...(draft.duration ? { duration: draft.duration } : {}),
    ...(draft.hdr ? { hdr: draft.hdr === "true" } : {}), ...(draft.startFrame ? { startFrameUrl: draft.startFrame } : {}),
    ...(draft.endFrame ? { endFrameUrl: draft.endFrame } : {}), ...(keyframes.length ? { keyframes } : {}),
    ...(Object.keys(edit).length ? { edit } : {}),
    ...(draft.loopVideo ? { loopVideo: draft.loopVideo === "true" } : {}),
    ...(parts.length === 4 ? { sourcePosition: { x: parts[0], y: parts[1], width: parts[2], height: parts[3] } } : {}),
  };
  const options = {
    model: draft.model, operation: draft.operation,
    ...(draft.ratio ? { aspectRatio: draft.ratio } : {}),
    ...(draft.source ? { sourceUrl: draft.source, ...(video ? { sourceMediaType: draft.sourceType } : {}) } : {}),
    ...(references.length ? { referenceUrls: references } : {}),
    ...(!video && !thirdParty && draft.style ? { style: draft.style } : {}),
    ...(!video && !thirdParty && draft.format ? { outputFormat: draft.format } : {}),
    ...(thirdParty && Object.keys(parameters).length ? { parameters } : {}),
    ...(video && Object.keys(controls).length ? { video: controls } : {}),
  };
  // Default options are omitted so pre-existing prepared tasks keep their original digest.
  const defaults = draft.operation === "generate" && ["luma_uni_1", "luma_ray3_2"].includes(draft.model) && Object.keys(options).length === 2;
  const reuse = draft.reuseEnabled ? { taskId: draft.reuseTask.trim(),
    ...(sourceAsset ? { sourceAssetId: sourceAsset } : {}), referenceAssetIds: assetReferences } : undefined;
  return { valid, kind: video ? "video" as const : "image" as const, options: defaults ? undefined : options, reuse };
}
