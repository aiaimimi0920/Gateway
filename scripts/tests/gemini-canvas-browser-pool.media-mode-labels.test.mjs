import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { createOperationUiOwner } from "../gemini-canvas-browser-pool-operation-ui.mjs";
import { dismissMediaModeOnboarding } from "../gemini-canvas-browser-pool-media-mode-onboarding.mjs";
import { locator, missingLocator, domPage, ElementFixture } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

const app = await importTestableScript();

test("ordinary Web music accepts the current localized tool label", () => {
  assert.match("制作音乐", app.operationConfig("music").buttonName);
  assert.match("创作音乐", app.operationConfig("music").buttonName);
  assert.match("Create music", app.operationConfig("music").buttonName);
});

for (const [operation, label] of [["music", "取消选择“音乐”"], ["video", "取消选择“视频”"]]) {
  test(`ordinary Web ${operation} recognizes the selected composer chip`, async () => {
    const config = app.operationConfig(operation);
    assert.ok(config.selectedButtonName instanceof RegExp);
    assert.match(label, config.selectedButtonName);
    const page = {
      getByRole: (role, { name }) => role === "dialog" ? missingLocator()
        : locator({ async isVisible() { return role === "button" && name.test(label); } }),
      async evaluate() { throw new Error("selected chip must not require whole-page text"); },
    };
    assert.equal(await app.operationModeAppearsSelected(page, config, 100), true);
  });
}

test("ordinary Web selects the menuitemcheckbox before broad text fallbacks", async () => {
  let hiddenWaits = 0;
  const missing = locator({ async count() { return 0; }, async waitFor() { hiddenWaits += 1; throw new Error("hidden mode control"); } });
  let menuOpen = false, selected = false, menuClicks = 0;
  const tools = locator({ async click() { menuOpen = true; } });
  const menuItem = locator({
    async waitFor() { assert.equal(menuOpen, true); },
    async click() { selected = true; menuClicks += 1; },
  });
  const page = {
    getByRole(role, { name }) {
      if (role === "button" && name.test("上传和工具")) return tools;
      // The asynchronous introduction wait is not a hidden mode-control fallback.
      if (role === "button" && name.test("立即体验")) return missingLocator();
      if (role === "menuitemcheckbox" && name.test("制作视频")) return menuItem;
      if (role === "button" && name.test("取消选择“视频”")) {
        return locator({ async isVisible() { return selected; } });
      }
      return missing;
    },
    locator: () => missing, getByText: () => missing,
    async evaluate() { return ""; }, async waitForTimeout() {},
  };
  const owner = createOperationUiOwner({ operationConfig: app.operationConfig, operationNavLabel: () => null,
    dismissGeminiAppInterstitials: async () => {}, log: () => {} });
  assert.equal(await owner.clickOperationMode(page, "video", 10), true);
  assert.equal(menuClicks, 1);
  assert.equal(hiddenWaits, 0);
});

test("ordinary Web tools-menu text alone is not selected-mode evidence", async () => {
  const menu = new ElementFixture({ attributes: { role: "menu" } });
  const { page } = domPage({ controls: [menu], bodyText: "制作音乐" });
  page.getByRole = () => missingLocator();
  assert.equal(await app.operationModeAppearsSelected(page, app.operationConfig("music"), 1), false);
});

for (const commercial of [false, true]) {
  test(`ordinary Web video onboarding ${commercial ? "refuses subscription actions" : "dismisses only the known informational dialog"}`, async () => {
    let acknowledgements = 0, dialogVisible = true;
    const dialog = locator({
      async isVisible() { return dialogVisible; },
      async innerText() { return commercial ? "创作视频 Gemini Omni 免费试用 订阅" : "创作视频\nGemini Omni 加持\n立即体验"; },
      getByRole(role, { name }) {
        assert.equal(role, "button"); assert.match("立即体验", name);
        return locator({ async isVisible() { return true; }, async click() { acknowledgements += 1; dialogVisible = false; } });
      },
      async waitFor({ state }) { assert.equal(state, "hidden"); assert.equal(dialogVisible, false); },
    });
    const page = {
      getByRole(role, { name }) {
        if (role === "dialog") return dialog;
        return locator({ async isVisible() { return role === "button" && name.test("取消选择“视频”"); } });
      },
    };
    assert.equal(await app.clickOperationMode(page, "video", 100), !commercial);
    assert.equal(acknowledgements, commercial ? 0 : 1);
  });
}

test("ordinary Web video acknowledges onboarding before requiring a selected chip", async () => {
  let menuOpen = false, dialogOpen = false, selected = false, acknowledgements = 0;
  const missing = missingLocator();
  const dialog = locator({
    async isVisible() { return dialogOpen; }, async innerText() { return "创作视频\nGemini Omni 加持\n立即体验"; },
    getByRole() { return locator({ async isVisible() { return true; }, async click() {
      dialogOpen = false; selected = true; acknowledgements += 1;
    } }); },
    async waitFor() { assert.equal(dialogOpen, false); },
  });
  const page = {
    getByRole(role, { name }) {
      if (role === "dialog") return dialog;
      if (role === "button" && name.test("上传和工具")) return locator({ async click() { menuOpen = true; } });
      if (role === "menuitemcheckbox" && name.test("制作视频")) return locator({
        async waitFor() { assert.equal(menuOpen, true); }, async click() { menuOpen = false; dialogOpen = true; },
      });
      if (role === "button" && name.test("取消选择“视频”")) return locator({ async isVisible() { return selected; } });
      return missing;
    },
    locator: () => missing, getByText: () => missing, async evaluate() { return ""; }, async waitForTimeout() {},
  };
  const owner = createOperationUiOwner({ operationConfig: app.operationConfig, operationNavLabel: () => null,
    dismissGeminiAppInterstitials: async () => {}, log: () => {} });
  assert.equal(await owner.clickOperationMode(page, "video", 10), true);
  assert.equal(acknowledgements, 1);
});

test("ordinary Web video handles a zero-bounds dialog with a visible acknowledgement", async () => {
  let clicks = 0, visible = true;
  const acknowledge = locator({ async isVisible() { return visible; }, async click() { clicks += 1; visible = false; },
    async waitFor({ state }) { assert.equal(state, "hidden"); assert.equal(visible, false); } });
  const dialog = locator({ async isVisible() { return false; },
    async innerText() { return "创作视频\nGemini Omni 加持\n立即体验"; }, getByRole: () => acknowledge });
  assert.equal(await dismissMediaModeOnboarding({ getByRole: () => dialog }, "video", 100), true);
  assert.equal(clicks, 1);
});
