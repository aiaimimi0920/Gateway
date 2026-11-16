import { DEFAULT_NAVIGATION_TIMEOUT_MS } from "./request.mjs";
import { gotoUdio } from "./browser.mjs";
import { isChallengeOutcome, requireJsonResponse } from "./responses.mjs";
import { submitNativeGeneration } from "./native-submit.mjs";

export async function resolveVercelCheckpointInBorrowedPage(page, options) {
  const { baseUrl, referer, timeoutMs } = options;
  const checkpointUrl = `${baseUrl}/api/users/current`;
  const response = await page.goto(checkpointUrl, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, DEFAULT_NAVIGATION_TIMEOUT_MS),
  });
  if (!response) {
    throw Object.assign(new Error("Udio security checkpoint did not return a response."), {
      status: 502,
      code: "udio_checkpoint_navigation_failed",
    });
  }

  const initialOutcome = {
    status: response.status(),
    contentType: response.headers()["content-type"] ?? "",
    mitigated: response.headers()["x-vercel-mitigated"] ?? "",
    text: await response.text().catch(() => ""),
  };
  if (!response.ok() && !isChallengeOutcome(initialOutcome)) {
    requireJsonResponse(
      initialOutcome,
      "udio_checkpoint_invalid_json",
      "Udio security checkpoint probe returned invalid JSON.",
    );
  }

  const deadline = Date.now() + timeoutMs;
  while (!response.ok() && Date.now() < deadline) {
    const probe = await page.evaluate(async (requestUrl) => {
      try {
        const result = await fetch(requestUrl, { credentials: "include" });
        return {
          ok: result.ok,
          status: result.status,
          mitigated: result.headers.get("x-vercel-mitigated") || "",
        };
      } catch {
        return { ok: false, status: 0, mitigated: "" };
      }
    }, checkpointUrl);
    if (probe.ok && !probe.mitigated) {
      await gotoUdio(page, referer || `${baseUrl}/create`, DEFAULT_NAVIGATION_TIMEOUT_MS);
      return;
    }
    await page.waitForTimeout(Math.min(1_000, Math.max(100, deadline - Date.now())));
  }

  if (response.ok()) {
    await gotoUdio(page, referer || `${baseUrl}/create`, DEFAULT_NAVIGATION_TIMEOUT_MS);
    return;
  }
  throw Object.assign(
    new Error("Udio requires the visible Vercel Security Checkpoint to be completed."),
    {
      status: 429,
      code: "udio_browser_challenge_required",
    },
  );
}

export async function submitGenerationThroughPageUi(page, options) {
  return submitNativeGeneration(page, options, waitForNativeCreateButton);
}

export async function waitForNativeCreateButton(page, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const createButton = await findNativeCreateButton(page);
    if (createButton) {
      return createButton;
    }
    await page.waitForTimeout(Math.min(500, Math.max(50, deadline - Date.now())));
  }
  throw Object.assign(new Error("Udio native Create button did not become available."), {
    status: 409,
    code: "udio_browser_create_unavailable",
  });
}

export async function findNativeCreateButton(page) {
  const createButtons = page.getByRole("button", { name: "Create", exact: true });
  for (let index = 0; index < (await createButtons.count()); index += 1) {
    const candidate = createButtons.nth(index);
    if ((await candidate.isVisible()) && (await candidate.isEnabled())) {
      return candidate;
    }
  }
  return null;
}
