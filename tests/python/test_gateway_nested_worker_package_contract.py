import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

import gateway_package_fixture as package_fixture
from powershell_test_utils import powershell_executable


class GatewayNestedWorkerPackageContractTests(unittest.TestCase):
    def test_extracted_worker_and_inventory_modules_ship_with_verified_support_records(self):
        modules = (
            "tools/package-gateway-release.ps1",
            "tools/package-release/artifact-copy.ps1",
            "tools/package-release/source-provenance.ps1",
            "tools/run-gateway-console-live-e2e.ps1",
            "tools/console-live-e2e/runtime.ps1",
            "tools/console-live-e2e/redis.ps1",
            "tools/console-live-e2e/revision-seed.ps1",
            "tools/console-live-e2e/upstream.ps1",
            "tools/generate-gateway-provider-inventory.py",
            "tools/provider_inventory/__init__.py",
            "tools/provider_inventory/contracts.py",
            "tools/provider_inventory/metadata.py",
            "tools/provider_inventory/redaction.py",
            "scripts/qwen-web-session/page-probe.mjs",
            "scripts/qwen-web-session/login.mjs",
            "scripts/qwen-web-session/credentials.mjs",
            "scripts/suno-browser/browser.mjs",
            "scripts/suno-browser/pure.mjs",
            "scripts/gemini-canvas-browser-pool.mjs",
            "scripts/gemini-canvas-connected-client.js",
            "scripts/gemini-canvas-browser-pool-resources.mjs",
            "scripts/gemini-canvas-browser-pool-input.mjs",
            "scripts/gemini-canvas-browser-pool-executable.mjs",
            "scripts/gemini-canvas-browser-pool-tls.mjs",
            "scripts/gemini-canvas-browser-pool-runtime-state.mjs",
            "scripts/gemini-canvas-browser-pool-profile.mjs",
            "scripts/gemini-canvas-browser-pool-context.mjs",
            "scripts/gemini-canvas-browser-pool-app.mjs",
            "scripts/gemini-canvas-browser-pool-cookies.mjs",
            "scripts/gemini-canvas-browser-pool-navigation.mjs",
            "scripts/gemini-canvas-browser-pool-conversation-reset.mjs",
            "scripts/gemini-canvas-browser-pool-program-handles.mjs",
            "scripts/gemini-canvas-browser-pool-capture-metadata.mjs",
            "scripts/gemini-canvas-browser-pool-body.mjs",
            "scripts/gemini-canvas-browser-pool-network-capture.mjs",
            "scripts/gemini-canvas-browser-pool-media-urls.mjs",
            "scripts/gemini-canvas-browser-pool-program-snapshot.mjs",
            "scripts/gemini-canvas-browser-pool-program-state.mjs",
            "scripts/gemini-canvas-browser-pool-operation-ui.mjs",
            "scripts/gemini-canvas-browser-pool-composer.mjs",
            "scripts/gemini-canvas-browser-pool-media-assets.mjs",
            "scripts/gemini-canvas-browser-pool-video-ui.mjs",
            "scripts/gemini-canvas-browser-pool-payload.mjs",
            "scripts/gemini-canvas-browser-pool-tts.mjs",
            "scripts/gemini-canvas-browser-pool-text.mjs",
            "scripts/gemini-canvas-browser-pool-debug.mjs",
            "scripts/gemini-canvas-browser-pool-page-snapshot.mjs",
            "scripts/gemini-canvas-browser-pool-bootstrap-preview-result.mjs",
            "scripts/gemini-canvas-browser-pool-bootstrap-polling.mjs",
            "scripts/gemini-canvas-browser-pool-bootstrap-result.mjs",
            "scripts/gemini-canvas-browser-pool-bootstrap-execution.mjs",
            "scripts/gemini-canvas-browser-pool-media-polling.mjs",
            "scripts/gemini-canvas-browser-pool-media-result.mjs",
            "scripts/gemini-canvas-browser-pool-media-execution.mjs",
            "scripts/gemini-canvas-browser-pool-dispatcher.mjs",
            "scripts/gemini-canvas-browser-pool-fetch-preview.mjs",
            "scripts/gemini-canvas-browser-pool-fetch-execution.mjs",
            "scripts/gemini-canvas-browser-pool-fetch-page.mjs",
            "scripts/gemini-canvas-browser-pool-fetch-page-music.mjs",
            "scripts/gemini-canvas-browser-pool-server.mjs",
            "scripts/gemini-canvas-browser-pool-media-policy.mjs",
            "scripts/export-gemini-canvas-storage-state.mjs",
            "scripts/export-gemini-canvas-auth-signal.mjs",
            "scripts/export-gemini-canvas-runtime-capture.mjs",
            "scripts/probe-gemini-canvas-program-browserless-invoke.mjs",
            "scripts/gemini-canvas-browserless-auth.mjs",
            "scripts/gemini-canvas-browserless-response.mjs",
            "scripts/gemini-canvas-browserless-requests.mjs",
            "scripts/gemini-canvas-browserless-payload.mjs",
            "scripts/gemini-canvas-browserless-generate-content.mjs",
            "scripts/gemini-canvas-browserless-video.mjs",
            "scripts/gemini-canvas-browserless-music.mjs",
            "scripts/gemini-canvas-browserless-material.mjs",
            "scripts/gemini-canvas-browserless-options.mjs",
            "scripts/probe-gemini-canvas-program-handle.mjs",
            "scripts/gemini-canvas-runtime-paths.mjs",
            "scripts/gemini-canvas-program-handle-evidence.mjs",
            "scripts/gemini-canvas-program-handle-media.mjs",
            "scripts/gemini-canvas-program-handle-interaction.mjs",
            "scripts/gemini-canvas-program-handle-snapshot.mjs",
            "scripts/gemini-canvas-program-handle-execution.mjs",
            "scripts/gemini-canvas-program-handle-network-capture.mjs",
            "scripts/gemini-canvas-program-handle-capture-budget.mjs",
            "scripts/gemini-canvas-program-handle-cdp-sessions.mjs",
            "scripts/gemini-canvas-program-handle-response-capture.mjs",
            "scripts/gemini-canvas-browser-pool-transport.mjs",
            "scripts/gemini-canvas-browser-pool-action.mjs",
            "scripts/gemini-canvas-browser-pool-media-targets.mjs",
            "scripts/gemini-canvas-browser-pool-invoke-merge.mjs",
            "scripts/gemini-canvas-browser-pool-rpc-candidates.mjs",
            "scripts/gemini-canvas-browser-pool-proxy-discovery.mjs",
            "scripts/gemini-canvas-browser-pool-proxy-html.mjs",
            "scripts/gemini-canvas-browser-pool-invoke.mjs",
            "scripts/gemini-canvas-browser-pool-proxy-launch.mjs",
            "scripts/gemini-canvas-browser-pool-headers.mjs",
            "scripts/gemini-canvas-browser-pool-preview.mjs",
            "scripts/gemini-canvas-browser-pool-auth-bridge.mjs",
            "scripts/gemini-canvas-browser-pool-google-auth.mjs",
            "scripts/gemini-canvas-browser-pool-connected-client.mjs",
            "scripts/gemini-canvas-browser-pool-preview-page.mjs",
            "scripts/gemini-canvas-browser-pool-media-page.mjs",
            "scripts/gemini-canvas-browser-pool-no-key-fetch.mjs",
            "scripts/gemini-canvas-browser-pool-preview-music.mjs",
            "scripts/gemini-canvas-browser-pool-no-key-music.mjs",
            "scripts/gemini-canvas-browser-pool-page-music.mjs",
            "scripts/producer-browser-worker.mjs",
            "scripts/probe-aistudio-live-request.mjs",
            "scripts/aistudio-live-probe/input-text.mjs",
            "scripts/aistudio-live-probe/rpc-contract.mjs",
            "scripts/aistudio-live-probe/diagnostics.mjs",
            "scripts/aistudio-live-probe/rpc-attribution.mjs",
            "scripts/aistudio-live-probe/websocket-capture.mjs",
            "scripts/aistudio-live-probe/pending-buffer.mjs",
            "scripts/aistudio-live-probe/capture-writer.mjs",
            "scripts/aistudio-live-probe/browser-capture.mjs",
            "scripts/aistudio-live-probe/capture-tasks.mjs",
            "scripts/aistudio-live-probe/capture-budget.mjs",
            "scripts/aistudio-live-probe/atomic-capture-file.mjs",
            "scripts/aistudio-live-probe/storage-path.mjs",
            "scripts/aistudio-live-probe/object-storage-request.mjs",
            "scripts/aistudio-live-probe/runtime-state-file.mjs",
            "scripts/aistudio-live-probe/browser-preflight.mjs",
            "scripts/aistudio-live-probe/runtime-storage.mjs",
            "scripts/aistudio-live-probe/ui-actions.mjs",
            "scripts/aistudio-live-probe/page-observation.mjs",
            "scripts/aistudio-live-probe/local-dispatch.mjs",
            "scripts/aistudio-live-probe/request-config.mjs",
            "scripts/aistudio-live-probe/cli-io.mjs",
            "scripts/aistudio-live-probe/browser-hook.mjs",
            "scripts/aistudio-live-probe/publication.mjs",
            "scripts/aistudio-live-probe/capture-polling.mjs",
            "scripts/producer-browser/request-fields.mjs",
            "scripts/producer-browser/conversation-stream.mjs",
            "scripts/producer-browser/video-prompts.mjs",
            "scripts/producer-browser/video-flow.mjs",
            "scripts/producer-browser/page-video-flow.mjs",
            "scripts/producer-browser/page-request-input.mjs",
            "scripts/producer-browser/page-conversation.mjs",
            "scripts/producer-browser/page-status.mjs",
            "scripts/producer-browser/profile.mjs",
            "scripts/producer-browser/input.mjs",
            "scripts/producer-browser/stdin.mjs",
            "scripts/producer-browser/transport.mjs",
            "scripts/producer-browser/browser-transport.mjs",
            "scripts/producer-browser/response-body.mjs",
            "scripts/producer-browser/trace.mjs",
            "scripts/chatgpt-web-session-worker.mjs",
            "scripts/chatgpt-web-session-worker-helpers.mjs",
        )
        chatgpt_modules = tuple(
            file.relative_to(package_fixture.GATEWAY_ROOT).as_posix()
            for file in sorted((package_fixture.GATEWAY_ROOT / "scripts/chatgpt-web-session").glob("*.mjs"))
        )
        self.assertEqual(len(chatgpt_modules), 23)
        modules += chatgpt_modules
        fixture = package_fixture.GatewayPackageFixture()
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            source = root / "Gateway-source"
            release = root / "release" / "Gateway"
            fixture._write_fixture(source)
            for relative in modules:
                target = source / pathlib.PurePosixPath(relative)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((package_fixture.GATEWAY_ROOT / relative).read_bytes())

            result = fixture._run_packager(source, release)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            destination = release / "contract-v1"
            manifest = json.loads((destination / "manifest.json").read_text(encoding="utf-8"))
            records = {record["path"]: record for record in manifest["supportFiles"]}
            checksums = {
                relative: digest
                for digest, relative in (
                    line.split("  ", 1)
                    for line in (destination / "checksums.sha256").read_text(encoding="utf-8").splitlines()
                )
            }
            for relative in modules:
                with self.subTest(module=relative):
                    expected = (source / relative).read_bytes()
                    self.assertEqual((destination / relative).read_bytes(), expected)
                    digest = hashlib.sha256(expected).hexdigest()
                    self.assertEqual(records[relative]["sha256"], digest)
                    self.assertEqual(records[relative]["bytes"], len(expected))
                    self.assertEqual(checksums[relative], digest)

            inventory_help = subprocess.run(
                [sys.executable, str(destination / "tools/generate-gateway-provider-inventory.py"), "--help"],
                cwd=root,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=30,
            )
            self.assertEqual(inventory_help.returncode, 0, msg=inventory_help.stderr)
            self.assertIn("--evidence-dir", inventory_help.stdout)

            probe_import = subprocess.run(
                [shutil.which("node") or "node", "--input-type=module", "-e",
                 "const module = await import(process.argv[1]); "
                 "if (typeof module.buildProbeSummary !== 'function' || "
                 "typeof module.extractAistudioUiSignals !== 'function') process.exit(1);",
                 (destination / "scripts/aistudio-live-probe/diagnostics.mjs").as_uri()],
                cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=15,
            )
            self.assertEqual(probe_import.returncode, 0, msg=probe_import.stderr)

            input_import = subprocess.run(
                [shutil.which("node") or "node", "--input-type=module", "-e",
                 "const { applyGeminiAccountScope, normalizeString, normalizeObject } = await import(process.argv[1]); "
                 "if (applyGeminiAccountScope({authUser: '2'}).baseUrl !== 'https://gemini.google.com/u/2/' || "
                 "normalizeString(' x ') !== 'x' || Object.keys(normalizeObject(null)).length !== 0) process.exit(1);",
                 (destination / "scripts/gemini-canvas-browser-pool-input.mjs").as_uri()],
                cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=15,
            )
            self.assertEqual(input_import.returncode, 0, msg=input_import.stderr)

            capture_import = subprocess.run(
                [shutil.which("node") or "node", "--input-type=module", "-e",
                 "const module = await import(process.argv[1]); "
                 "if (typeof module.createProgramNetworkCaptureOwner !== 'function') process.exit(1);",
                 (destination / "scripts/gemini-canvas-program-handle-network-capture.mjs").as_uri()],
                cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=15,
            )
            self.assertEqual(capture_import.returncode, 0, msg=capture_import.stderr)

            if os.name == "nt":
                helper_check = subprocess.run(
                    [powershell_executable(), "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                     str(package_fixture.GATEWAY_ROOT / "tests/powershell/test-console-live-functions.ps1"),
                     "-ModuleRoot", str(destination / "tools/console-live-e2e"),
                     "-WorkRoot", str(root / "console-fixture")],
                    cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=45,
                )
                self.assertEqual(helper_check.returncode, 0, msg=helper_check.stdout + helper_check.stderr)
                self.assertIn("console-live-functions: pass", helper_check.stdout)


if __name__ == "__main__":
    unittest.main()
