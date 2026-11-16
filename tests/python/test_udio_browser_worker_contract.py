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
        owned_flow = source.split('if (!browserCdpUrl) {', 1)[1].split('} else {', 1)[0]
        self.assertIn('allowNavigation: true', owned_flow)
        self.assertEqual(source.count('allowNavigation: true'), owned_flow.count('allowNavigation: true'))

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
        submit_index = source.index("return await runGeneration({")
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
        self.assertIn('const headers = { accept: "application/json", origin, referer, ...(authToken ? { authorization: authToken } : {}) }', source)
        self.assertIn('headers: { ...headers, "content-type": "application/json" }', source)
        self.assertIn('method: "GET", headers, timeoutMs:', source)
        self.assertIn("cookies = await context.cookies().catch(() => [])", source)
        self.assertIn('session.send("Network.getAllCookies")', source)
        self.assertIn("delete requestBody.captchaToken", source)
        generation = read_sources("generation")
        self.assertEqual(generation.count("await submit(remaining())"), 1)
        self.assertIn("Generation was not replayed", generation)

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
        entry = read_sources("entry")
        flow = read_sources("native-flow")
        native = read_sources("native-submit")
        generation = read_sources("generation")
        self.assertIn("? submitGenerationThroughPageUi(page", entry)
        self.assertIn("submitNativeGeneration(page", flow)
        self.assertIn('page.locator("textarea").first()', native)
        self.assertIn('page.getByRole("button", { name: "Create", exact: true })', flow)
        self.assertIn('value.request().method().toUpperCase() !== "POST"', native)
        self.assertIn('url.origin === origin', native)
        self.assertIn('pageOnly: true, failOnBodyError: true, bodyBytes: 128 * 1024', native)
        self.assertLess(native.index("await capture.ready"), native.index("await input.fill"))
        self.assertLess(native.index("gate.arm()"), native.index("await button.click"))
        self.assertEqual(native.count("await button.click"), 1)
        self.assertIn("await gate?.stop()", native)
        self.assertIn("await capture?.stop()", native)
        self.assertIn("const until = Math.min(deadline, now() + waitTimeoutMs)", generation)
        self.assertIn("songs.length === trackIds.length && songsReadyForTarget", generation)
        self.assertIn("!trackIds.includes(song.id)", generation)


if __name__ == "__main__":
    unittest.main()
