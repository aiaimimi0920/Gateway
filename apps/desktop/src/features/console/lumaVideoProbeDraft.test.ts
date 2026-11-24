import { expect, it } from "vitest";
import { emptyLumaGuide, emptyLumaVideoControls, lumaVideoProbeControls } from "./lumaVideoProbeDraft";

const image = "https://images.example.test/guide.png";
it("omits inactive conditioning, guides and position when the selected operation does not use them", () => {
  const draft = emptyLumaVideoControls();
  draft.hdr = true; draft.positioned = true; draft.position.w_norm = "0";
  draft.manualSignals = true; draft.signals.depth = { mode: "enabled", tuning: "2" };
  draft.guideMode = "keyframes";
  expect(lumaVideoProbeControls(draft, "image", "360p", "auto")).toEqual({});
  expect(lumaVideoProbeControls(draft, "video", "360p", "auto")).toEqual({});
});
it("keeps unspecified tuning unknown and omits disabled signal tuning", () => {
  const draft = emptyLumaVideoControls(); draft.manualSignals = true;
  draft.signals.pose = { mode: "enabled", tuning: "" };
  draft.signals.face = { mode: "disabled", tuning: "" };
  draft.signals.depth = { mode: "disabled", tuning: "not-used" };
  expect(lumaVideoProbeControls(draft, "video_edit", "720p", "auto")).toEqual({ edit: {
    controls: { pose: { enabled: true }, depth: { enabled: false }, face: { enabled: false } },
  } });
  for (const tuning of ["-0.1", "1.01", "NaN", "Infinity", " "]) {
    draft.signals.depth = { mode: "enabled", tuning };
    expect(lumaVideoProbeControls(draft, "video_edit", "720p", "auto")).toBeNull();
  }
});
it("accepts at most 64 complete unique guide pairs and rejects unsafe or fractional frame indices", () => {
  const draft = emptyLumaVideoControls(); draft.guideMode = "keyframes";
  draft.guides = Array.from({ length: 64 }, (_, index) => ({ source: { kind: "url", value: image, mime: "image/png" }, index: String(index) }));
  const video = lumaVideoProbeControls(draft, "video_edit", "720p", "auto");
  expect(video).toMatchObject({ edit: { auto_controls: true, keyframe_indexes: Array.from({ length: 64 }, (_, index) => index) } });
  draft.guides.push({ source: { kind: "url", value: image, mime: "image/png" }, index: "64" });
  expect(lumaVideoProbeControls(draft, "video_edit", "720p", "auto")).toBeNull();
  draft.guides = [{ source: { kind: "url", value: image, mime: "image/png" }, index: "0" }];
  for (const index of ["", "-1", "1.5", "9007199254740992", "1e2"]) {
    draft.guides[0].index = index;
    expect(lumaVideoProbeControls(draft, "video_edit", "720p", "auto")).toBeNull();
  }
  draft.guides = [emptyLumaGuide()];
  expect(lumaVideoProbeControls(draft, "video_edit", "720p", "auto")).toBeNull();
});
