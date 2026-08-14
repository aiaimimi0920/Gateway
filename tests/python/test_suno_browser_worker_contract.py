import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKER = REPO_ROOT / "scripts" / "suno-browser-worker.mjs"


class SunoBrowserWorkerContractTests(unittest.TestCase):
    def test_cdp_context_is_not_overwritten_or_closed(self):
        source = WORKER.read_text(encoding="utf-8")

        self.assertIn(
            "if (cookieHeader && !effectiveBrowserTarget.browserCdpUrl)", source
        )
        self.assertIn("await closeBrowser();", source)
        self.assertIn(
            'candidatePage.url().startsWith("https://suno.com")', source
        )
        self.assertNotIn(
            "await browser?.close().catch(() => undefined);\n    await releaseEasyBrowserLease",
            source,
        )
        self.assertNotIn("bringToFront", source)
        self.assertIn("borrowedContext: Boolean(effectiveBrowserTarget.browserCdpUrl)", source)
        self.assertIn("if (borrowedContext && !existingPage)", source)
        self.assertIn("if (!borrowedContext && (forceNavigate", source)

    def test_hidden_turnstile_inputs_are_not_treated_as_active_challenges(self):
        source = WORKER.read_text(encoding="utf-8")

        self.assertIn("element.getClientRects().length > 0", source)
        self.assertNotIn(
            "selectors.some((selector) => document.querySelector(selector))", source
        )

    def test_generation_uses_native_page_verification_and_current_simple_mode_controls(self):
        source = WORKER.read_text(encoding="utf-8")

        self.assertNotIn("requestGenerationTurnstileToken", source)
        self.assertNotIn("window.turnstile.render", source)
        self.assertNotIn("gateway-suno-turnstile-surface", source)
        self.assertNotIn("gateway-suno-turnstile-token", source)
        self.assertNotIn("token_provider:", source)
        self.assertIn("const createResponsePromise = page.waitForResponse(", source)
        self.assertIn("await createButton.click()", source)
        self.assertIn("if (await pageLooksChallenged(page))", source)
        self.assertIn(
            '"Complete the visible Suno security check before generation can continue."',
            source,
        )
        self.assertIn("timeoutMs,", source)
        self.assertIn("if (!hasVisibleSecurityChallenge)", source)
        self.assertIn('button:visible[aria-label="Create song"]', source)
        self.assertIn('textarea:visible[maxlength="3000"]', source)
        self.assertIn('textarea.value === expected', source)
        self.assertNotIn('buildStylePrompt(prompt, placeholder)', source)

    def test_wait_completion_requires_every_returned_clip_asset(self):
        source = WORKER.read_text(encoding="utf-8")

        self.assertIn("clips.length > 0", source)
        self.assertIn(
            "(clip) => clipCompletedStatus(clip) && clipHasTargetAsset(clip, targetAssetKind)",
            source,
        )
        self.assertNotIn(
            "clips.some((clip) => clipHasTargetAsset(clip, targetAssetKind))",
            source,
        )


if __name__ == "__main__":
    unittest.main()
