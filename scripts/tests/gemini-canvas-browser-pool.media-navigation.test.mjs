import assert from "node:assert/strict";
import test from "node:test";
import { createOperationUiOwner } from "../gemini-canvas-browser-pool-operation-ui.mjs";
import { dismissMediaModeOnboarding } from "../gemini-canvas-browser-pool-media-mode-onboarding.mjs";
import { locator, missingLocator } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

for (const labels of [["Tools", "More tools", "Create music"], ["上传和工具", "更多工具", "制作音乐"]]) {
  test(`media navigation reveals the bounded additional-tools menu: ${labels[1]}`, async (t) => {
    let now = 0, stage = "closed", selected = false;
    const clicks = [];
    t.mock.method(Date, "now", () => now);
    const controls = labels.map((label, index) => locator({
      async isVisible() { return stage === ["closed", "outer", "inner"][index]; },
      async waitFor() { if (stage !== ["closed", "outer", "inner"][index]) throw new Error("hidden"); },
      async click() { clicks.push(label); if (index === 2) selected = true; stage = ["outer", "inner", "closed"][index]; },
    }));
    const page = {
      getByRole(role, { name }) {
        if (role !== "button") return missingLocator();
        if (name.test("Deselect music")) return locator({ async isVisible() { return selected; } });
        const index = labels.findIndex(label => name.test(label));
        return index < 0 ? missingLocator() : controls[index];
      },
      locator: () => missingLocator(), getByText: () => missingLocator(),
      async evaluate() { return ""; }, async waitForTimeout(ms) { now += ms; },
    };
    const ui = createOperationUiOwner({
      operationConfig: () => ({ buttonName: /制作音乐|Create music/i, selectedButtonName: /Deselect music/i }),
      operationNavLabel: () => null, dismissGeminiAppInterstitials: async () => {}, log() {},
    });
    assert.equal(await ui.clickOperationMode(page, "music", 50), true);
    assert.deepEqual(clicks, labels);
  });
}

test("video onboarding waits for the asynchronously mounted informational dialog", async () => {
  let mounted = false, acknowledged = 0;
  const acknowledge = locator({
    async isVisible() { return mounted; },
    async waitFor({ state }) { if (state === "visible") mounted = true; },
    async click() { acknowledged += 1; mounted = false; },
  });
  const dialog = locator({
    async count() { return mounted ? 1 : 0; }, async isVisible() { return mounted; },
    async innerText() { return "创作视频\nGemini Omni 加持\n多样素材，随心创作"; },
    getByRole() { return acknowledge; },
  });
  const page = { getByRole(role) { return role === "button" ? acknowledge : dialog; } };
  assert.equal(await dismissMediaModeOnboarding(page, "video", 5000), true);
  assert.equal(acknowledged, 1);
});

test("video onboarding never acknowledges a delayed subscription dialog", async () => {
  let mounted = false;
  const acknowledge = locator({ async waitFor() { mounted = true; }, async isVisible() { return mounted; },
    async click() { throw new Error("must not accept subscription"); } });
  const dialog = locator({ async count() { return mounted ? 1 : 0; }, async isVisible() { return mounted; },
    async innerText() { return "创作视频\nGemini Omni 加持\n升级订阅 $20"; }, getByRole() { return acknowledge; } });
  assert.equal(await dismissMediaModeOnboarding({ getByRole: role => role === "button" ? acknowledge : dialog }, "video", 5000), false);
});
