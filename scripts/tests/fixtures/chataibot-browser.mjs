import { appendFile, readFile } from "node:fs/promises";
import path from "node:path";

// This fixture replaces Playwright in an isolated worker process. It never
// launches a browser or reads anything outside the test's synthetic profile.
const record = (event) => appendFile(process.env.CHATAIBOT_TEST_EVENTS, `${JSON.stringify(event)}\n`);
const page = {
  goto: async () => {},
  waitForTimeout: async () => {},
  evaluate: async () => {
    if (process.env.CHATAIBOT_TEST_FAILURE === "probe") throw new Error("fixture probe failed");
    return { ok: true, authToken: "fixture-token", availableModels: ["fixture-model"] };
  },
};

export const chromium = {
  async launchPersistentContext(profile) {
    const preferences = await readFile(path.join(profile, "Default", "Preferences"), "utf8");
    await record({ event: "launch", profile, preferences });
    if (process.env.CHATAIBOT_TEST_FAILURE === "launch") throw new Error("fixture launch failed");
    return {
      pages: () => [page],
      cookies: async () => [{ name: "token", value: "fixture-token" }],
      async close() {
        await new Promise((resolve) => setTimeout(resolve, 30));
        await record({ event: "close" });
        if (process.env.CHATAIBOT_TEST_FAILURE === "close") throw new Error("fixture close failed");
      },
    };
  },
};
