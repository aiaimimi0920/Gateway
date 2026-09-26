import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { previewText } from "./payload.mjs";

export async function writeWorkerDebugSnapshot(page, label, extra = {}) {
  try {
    const runtimeDir = path.resolve(process.cwd(), ".runtime");
    await mkdir(runtimeDir, { recursive: true });
    const screenshotPath = path.join(runtimeDir, "aistudio-worker-page-debug.png");
    const jsonPath = path.join(runtimeDir, "aistudio-worker-debug.json");
    await page.screenshot({ path: screenshotPath, fullPage: true }).catch(() => undefined);
    const selectors = [
      'textarea[aria-label="Enter a prompt to generate an app"]',
      'textarea[placeholder*="Describe an app"]',
      'textarea[placeholder*="Make changes"]',
      '[contenteditable="true"][role="textbox"]',
      '[contenteditable="true"]',
      'button:has-text("Next")',
      'button:has-text("Launch")',
    ];
    const locatorStates = [];
    for (const selector of selectors) {
      const locator = page.locator(selector).first();
      locatorStates.push({
        selector,
        count: await page.locator(selector).count().catch(() => 0),
        visible: await locator.isVisible({ timeout: 300 }).catch(() => false),
        text: previewText(await locator.textContent().catch(() => "")),
      });
    }
    await writeFile(
      jsonPath,
      JSON.stringify(
        {
          label,
          pageUrl: page.url(),
          locatorStates,
          capturedAt: new Date().toISOString(),
          ...extra,
        },
        null,
        2,
      ),
    );
  } catch (_) {}
}

export async function writeWorkerStageSnapshot(label, extra = {}) {
  try {
    const runtimeDir = path.resolve(process.cwd(), ".runtime");
    await mkdir(runtimeDir, { recursive: true });
    const jsonPath = path.join(runtimeDir, "aistudio-worker-stage.json");
    await writeFile(
      jsonPath,
      JSON.stringify(
        {
          label,
          capturedAt: new Date().toISOString(),
          ...extra,
        },
        null,
        2,
      ),
    );
  } catch (_) {}
}
