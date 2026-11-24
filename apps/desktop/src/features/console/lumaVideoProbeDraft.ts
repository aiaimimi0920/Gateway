// Draft-only video parameters; each explicit invocation is validated before charge confirmation.
import { emptyMediaProbeSource, mediaProbeSource, type MediaProbeSource } from "./LumaMediaProbeSource";

export const LUMA_SIGNALS = [
  ["pose", "姿态约束", "Pose conditioning", "strength", "姿态精度", "Pose precision"],
  ["depth", "深度约束", "Depth conditioning", "blur", "深度模糊", "Depth blur"],
  ["normals", "法线约束", "Normal conditioning", "augmentation", "法线变化", "Normal augmentation"],
  ["trajectory", "运动约束", "Motion conditioning", "sparsity", "轨迹稀疏度", "Trajectory sparsity"],
  ["face", "人脸约束", "Face conditioning", "", "", ""],
] as const;
type Signal = typeof LUMA_SIGNALS[number][0];
type SignalDraft = { mode: "default" | "enabled" | "disabled"; tuning: string };
export type LumaGuideFrame = { source: MediaProbeSource; index: string };
export type LumaVideoControlsDraft = {
  hdr: boolean;
  manualSignals: boolean;
  signals: Record<Signal, SignalDraft>;
  guideMode: "none" | "single" | "keyframes";
  guides: LumaGuideFrame[];
  positioned: boolean;
  position: { x_norm: string; y_norm: string; w_norm: string; h_norm: string };
};
export const emptyLumaGuide = (): LumaGuideFrame => ({ source: emptyMediaProbeSource(), index: "0" });
export const emptyLumaVideoControls = (): LumaVideoControlsDraft => ({
  hdr: false, manualSignals: false, guideMode: "none", guides: [emptyLumaGuide()], positioned: false,
  signals: { pose: { mode: "default", tuning: "" }, depth: { mode: "default", tuning: "" },
    normals: { mode: "default", tuning: "" }, trajectory: { mode: "default", tuning: "" }, face: { mode: "default", tuning: "" } },
  position: { x_norm: "0", y_norm: "0", w_norm: "1", h_norm: "1" },
});
function boundedNumber(text: string, min: number, max: number, positive = false): number | null {
  if (!/^-?(?:\d+(?:\.\d*)?|\.\d+)$/.test(text)) return null;
  const value = Number(text);
  return Number.isFinite(value) && value >= min && value <= max && (!positive || value > 0) ? value : null;
}
function manualControls(draft: LumaVideoControlsDraft): Record<string, unknown> | null {
  const controls: Record<string, unknown> = {};
  for (const [name, , , tuning] of LUMA_SIGNALS) {
    const signal = draft.signals[name];
    if (signal.mode === "default") continue;
    const item: Record<string, unknown> = { enabled: signal.mode === "enabled" };
    // Disabled signals keep only the explicit false; inactive tuning is never submitted.
    if (tuning && signal.mode === "enabled" && signal.tuning !== "") {
      if (name === "pose") {
        if (!["precise", "coarse"].includes(signal.tuning)) return null;
        item[tuning] = signal.tuning;
      } else {
        const value = boundedNumber(signal.tuning, 0, 1);
        if (value === null) return null;
        item[tuning] = value;
      }
    }
    controls[name] = item;
  }
  return controls;
}
/** Null means a visible control is invalid; callers must block rather than omit it. */
export function lumaVideoProbeControls(draft: LumaVideoControlsDraft, operation: string, resolution: string, strength: string): Record<string, unknown> | null {
  if (operation === "video_reframe") {
    if (!draft.positioned) return {};
    const position: Record<string, number> = {};
    for (const key of ["x_norm", "y_norm", "w_norm", "h_norm"] as const) {
      const dimension = key === "w_norm" || key === "h_norm";
      const value = boundedNumber(draft.position[key], dimension ? 0 : -2, 2, dimension);
      if (value === null) return null;
      position[key] = value;
    }
    return { source_position: position };
  }
  if (operation !== "video_edit") return {};
  const video: Record<string, unknown> = {}, edit: Record<string, unknown> = {};
  if (draft.hdr) {
    if (!["720p", "1080p"].includes(resolution)) return null;
    video.hdr = true;
  }
  if (strength !== "auto") edit.strength = strength;
  if (draft.manualSignals) {
    const controls = manualControls(draft);
    if (!controls) return null;
    edit.controls = controls;
  } else if (strength === "auto") edit.auto_controls = true;
  if (draft.guideMode !== "none") {
    const rows = draft.guides;
    if (!rows.length || rows.length > 64) return null;
    const sources = rows.map(row => mediaProbeSource(row.source));
    if (sources.some(source => source === null)) return null;
    if (draft.guideMode === "single") {
      if (rows.length !== 1) return null;
      video.start_frame = sources[0];
    } else {
      const indexes = rows.map(row => /^\d+$/.test(row.index) ? Number(row.index) : NaN);
      if (indexes.some(index => !Number.isSafeInteger(index) || index < 0) || new Set(indexes).size !== indexes.length) return null;
      edit.keyframes = sources;
      edit.keyframe_indexes = indexes;
    }
  }
  video.edit = edit;
  return video;
}
