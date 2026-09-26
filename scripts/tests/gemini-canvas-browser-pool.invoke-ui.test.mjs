import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const { inferInvokeUiState } = await importTestableScript();
const cases = [
  ["unknown operation and missing snapshot", "image", null, null],
  ["empty video snapshot", "video", {}, null],
  ["music download without a player clock", "music", { buttons: [{ text: "下载音乐作品" }] }, null],
  ["music clock without a download control", "music", { bodyText: "0:00 / 0:30", buttons: [] }, null],
  ["music player with localized download label", "music", { bodyText: "0:00 / 0:30", buttons: [{ ariaLabel: "下载音乐作品" }] }, "music_player_ready"],
  ["music generation before ready controls", "music", { bodyText: "Generating your music 0:00 / 0:30", buttons: [{ title: "下载音乐作品" }] }, "music_generating"],
  ["video generation before ready text", "video", { bodyText: "Generating your video. Your video is ready!", buttons: [{ text: "Download video" }] }, "video_generating"],
  ["video ready text without controls", "video", { bodyText: "Your video is ready!" }, "video_player_ready"],
  ["video player with localized control", "video", { buttons: [{ ariaLabel: "播放视频" }] }, "video_player_ready"],
  ["retry control without recognized operation", "unknown", { buttons: [{ title: "不使用应用，再试一次" }] }, "retry_without_app_visible"],
  ["music ready before retry control", "music", { bodyText: "0:00 / 0:30", buttons: [{ text: "下载音乐作品" }, { title: "不使用应用，再试一次" }] }, "music_player_ready"],
  ["video ready before retry control", "video", { bodyText: "视频已生成", buttons: [{ title: "不使用应用，再试一次" }] }, "video_player_ready"],
];

for (const [label, operation, snapshot, expected] of cases) {
  test(`invoke UI inference handles ${label}`, () => {
    if (snapshot) {
      for (const button of snapshot.buttons ?? []) Object.freeze(button);
      if (snapshot.buttons) Object.freeze(snapshot.buttons);
      Object.freeze(snapshot);
    }
    assert.equal(inferInvokeUiState(operation, snapshot), expected);
  });
}
