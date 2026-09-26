import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKER = REPO_ROOT / "scripts" / "udio-browser-worker.mjs"


def read_sources(*owners):
    paths = [
        WORKER if owner == "entry" else WORKER.parent / "udio-browser" / f"{owner}.mjs"
        for owner in owners
    ]
    return "\n".join(path.read_text(encoding="utf-8") for path in paths)


class UdioBrowserWorkerContractTests(unittest.TestCase):
    def test_debug_log_path_can_be_injected_without_logging_secrets(self):
        source = read_sources("entry", "captcha", "diagnostics")

        self.assertIn("process.env.UDIO_BROWSER_DEBUG_LOG_PATH", source)
        self.assertNotIn('debugLog(debugLogPath, "captcha_token_ready", {\n          token:', source)
        self.assertIn("tokenLength:", source)

    def test_cdp_worker_does_not_steal_window_focus(self):
        source = read_sources("entry", "browser", "native-flow")

        self.assertNotIn("bringToFront", source)
        self.assertIn("borrowedContext: true", source)
        self.assertIn("if (borrowedContext && !existingPage)", source)
        self.assertIn("if (!borrowedContext && (!existingPage", source)
        self.assertIn("allowNavigation: !browserCdpUrl", source)

    def test_runtime_state_is_not_overwritten_by_static_cookie_material(self):
        source = WORKER.read_text(encoding="utf-8")

        self.assertIn(
            "if (cookieHeader && !runtimeStateObjectKey && !browserCdpUrl)", source
        )

    def test_cdp_attach_prefers_the_authenticated_provider_context(self):
        source = read_sources("entry", "browser")

        self.assertIn("resolveBorrowedContext(browser, baseUrl, browserCdpTargetUrl, referer)", source)
        self.assertIn("target.origin === base.origin", source)
        self.assertIn("pageMatchesTarget(page, target)", source)
        self.assertNotIn("candidatePage.url().startsWith(baseUrl)", source)
        self.assertNotIn("browser.contexts()[0]", source)

    def test_generation_submit_resolves_and_sends_authorization_token(self):
        source = read_sources("entry", "auth")
        resolve_index = source.index(
            "const authToken = await resolveUdioAccessToken(page, baseUrl);"
        )
        submit_index = source.index("let submitOutcome = await submitGeneration()")
        authorization_index = source.index(
            "...(authToken ? { authorization: authToken } : {}),"
        )
        captcha_probe_index = source.index(
            'requestUrl: `${baseUrl}/api/generate-proxy/captcha`'
        )

        self.assertLess(resolve_index, captcha_probe_index)
        self.assertLess(resolve_index, submit_index)
        self.assertGreater(authorization_index, captcha_probe_index)
        self.assertLess(authorization_index, submit_index)
        authorization_occurrences = source.count(
            "...(authToken ? { authorization: authToken } : {}),"
        )
        self.assertGreaterEqual(authorization_occurrences, 3)
        self.assertIn("cookies = await context.cookies().catch(() => [])", source)
        self.assertIn('session.send("Network.getAllCookies")', source)
        self.assertIn("delete requestBody.captchaToken", source)
        self.assertEqual(source.count("submitOutcome = await submitGeneration()"), 2)

    def test_manual_captcha_uses_one_bounded_wait_window(self):
        source = read_sources("captcha")

        self.assertEqual(source.count("waitForSolvedToken(hcaptcha, manualWidgetId, manualChallengeWaitMs)"), 1)
        self.assertIn("__udioManualHcaptchaWidgetId", source)
        self.assertIn('size: "normal"', source)
        self.assertIn("removeWidget(window.__udioManualHcaptchaWidgetId, manualContainer)", source)
        self.assertNotIn("document.querySelectorAll(selector)", source)
        self.assertNotIn("hcaptcha.getResponse()", source)
        self.assertIn('typeof widgetId === "string"', source)
        self.assertIn("window.__udioManualHcaptchaResolvedToken", source)
        self.assertIn("callback: (token) =>", source)
        self.assertIn('"expired-callback": () =>', source)
        self.assertIn('"error-callback": () =>', source)

    def test_cdp_generation_uses_the_native_create_flow(self):
        source = read_sources("entry", "native-flow", "responses")

        helper_index = source.index("async function submitGenerationThroughPageUi")
        response_index = source.index(".waitForResponse", helper_index)
        fill_index = source.index("await promptInput.fill(prompt)", helper_index)
        click_index = source.index("await createButton.click()", helper_index)

        self.assertIn('page.locator("textarea").first()', source[helper_index:])
        self.assertIn(
            'page.getByRole("button", { name: "Create", exact: true })',
            source[helper_index:],
        )
        self.assertIn('normalizePathname(requestUrl.pathname) === "/api/generate-proxy"', source)
        self.assertIn('response.request().method() === "POST"', source)
        self.assertLess(fill_index, response_index)
        self.assertLess(response_index, click_index)
        self.assertIn("? await submitGenerationThroughPageUi(page", source)
        self.assertIn("await waitForNativeCreateButton(page", source)
        self.assertIn("const availableCreateButton = await findNativeCreateButton(page)", source)
        self.assertIn("if (sawCreateUnavailable && challengeCycleRetries < 3)", source)
        self.assertIn("await availableCreateButton.click()", source)
        self.assertIn("await resolveVercelCheckpointInBorrowedPage(page", source)
        self.assertIn('const checkpointUrl = `${baseUrl}/api/users/current`', source)
        self.assertIn('await debugLog(debugLogPath, "native_checkpoint_ready", {})', source)
        self.assertNotIn('"native_submit_challenge_retry"', source)
        self.assertIn("while (Date.now() < deadline)", source)
        self.assertIn("const waitDeadline = Date.now() + waitTimeoutMs", source)
        self.assertIn(
            "songs.every((song) => songHasTargetAsset(song, targetAssetKind))",
            source,
        )
        self.assertNotIn("song?.readyToStream === true", source)
        self.assertNotIn("song?.ready_to_stream === true", source)
        self.assertNotIn(
            "const waitDeadline = Math.min(overallDeadline, Date.now() + waitTimeoutMs)",
            source,
        )
        self.assertIn("await browserFetch(page", source)


if __name__ == "__main__":
    unittest.main()
