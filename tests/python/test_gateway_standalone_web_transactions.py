import hashlib
import json
import pathlib
import subprocess
import tempfile
import time
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneWebTransactionTests(unittest.TestCase):
    def test_two_concurrent_web_publishers_commit_a_consistent_live_revision(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            live = root / "live"
            ready = root / ".gateway-web-ready"
            barrier = root / ".gateway-web-publish.lock"
            live.mkdir()
            barrier.mkdir()
            (live / "index.html").write_text("old-index", encoding="utf-8")

            processes = []
            for name in ("alpha", "beta"):
                staging = root / f"staging-{name}"
                (staging / "static/js").mkdir(parents=True)
                (staging / "index.html").write_text(
                    f'<script src="/ui/static/js/{name}.js"></script>',
                    encoding="utf-8",
                )
                (staging / f"static/js/{name}.js").write_text(
                    name, encoding="utf-8"
                )
                processes.append(
                    subprocess.Popen(
                        [
                            "node",
                            str(publisher),
                            "--staging",
                            str(staging),
                            "--live",
                            str(live),
                            "--ready",
                            str(ready),
                        ],
                        cwd=GATEWAY_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                )

            try:
                time.sleep(0.3)
                self.assertTrue(all(process.poll() is None for process in processes))
                barrier.rmdir()
                results = [process.communicate(timeout=30) for process in processes]
            finally:
                if barrier.exists():
                    barrier.rmdir()
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=10)

            for process, (stdout, stderr) in zip(processes, results, strict=True):
                self.assertEqual(
                    0,
                    process.returncode,
                    f"publisher failed:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                )

            marker = json.loads(ready.read_text(encoding="utf-8"))
            index_bytes = (live / "index.html").read_bytes()
            self.assertEqual(
                hashlib.sha256(index_bytes).hexdigest(), marker["indexSha256"]
            )
            index = index_bytes.decode("utf-8")
            winner = "alpha" if "alpha.js" in index else "beta"
            self.assertEqual(
                winner,
                (live / f"static/js/{winner}.js").read_text(encoding="utf-8"),
            )

    def test_two_concurrent_pruning_publishers_leave_only_the_winning_revision(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            live = root / "live"
            ready = root / ".gateway-web-ready"
            barrier = root / ".gateway-web-publish.lock"
            (live / "static/js").mkdir(parents=True)
            (live / "index.html").write_text("old-index", encoding="utf-8")
            (live / "static/js/old.js").write_text("old", encoding="utf-8")
            barrier.mkdir()

            processes = []
            for name in ("alpha", "beta"):
                staging = root / f"staging-{name}"
                (staging / "static/js").mkdir(parents=True)
                (staging / "index.html").write_text(
                    f'<script src="/ui/static/js/{name}.js"></script>',
                    encoding="utf-8",
                )
                (staging / f"static/js/{name}.js").write_text(
                    name, encoding="utf-8"
                )
                processes.append(
                    subprocess.Popen(
                        [
                            "node",
                            str(publisher),
                            "--staging",
                            str(staging),
                            "--live",
                            str(live),
                            "--ready",
                            str(ready),
                            "--prune",
                        ],
                        cwd=GATEWAY_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                )

            try:
                time.sleep(0.3)
                self.assertTrue(all(process.poll() is None for process in processes))
                barrier.rmdir()
                results = [process.communicate(timeout=30) for process in processes]
            finally:
                if barrier.exists():
                    barrier.rmdir()
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=10)

            for process, (stdout, stderr) in zip(processes, results, strict=True):
                self.assertEqual(
                    0,
                    process.returncode,
                    f"publisher failed:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                )

            marker = json.loads(ready.read_text(encoding="utf-8"))
            index_bytes = (live / "index.html").read_bytes()
            self.assertEqual(
                hashlib.sha256(index_bytes).hexdigest(), marker["indexSha256"]
            )
            index = index_bytes.decode("utf-8")
            winner = "alpha" if "alpha.js" in index else "beta"
            loser = "beta" if winner == "alpha" else "alpha"
            self.assertTrue((live / f"static/js/{winner}.js").is_file())
            self.assertFalse((live / f"static/js/{loser}.js").exists())
            self.assertFalse((live / "static/js/old.js").exists())

    def test_web_dist_publisher_restores_overwritten_and_added_assets_when_marker_commit_fails(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (staging / "static/new-revision").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            old_index = b'<script src="/ui/static/js/app.js"></script>'
            old_marker = {
                "indexSha256": hashlib.sha256(old_index).hexdigest(),
                "publishedAt": "2026-07-28T00:00:00.000Z",
            }
            (staging / "index.html").write_bytes(
                b'<script src="/ui/static/js/app.js"></script>'
                b'<script src="/ui/static/new-revision/new-only.js"></script>'
            )
            (staging / "static/js/app.js").write_text(
                "new-app", encoding="utf-8"
            )
            (staging / "static/new-revision/new-only.js").write_text(
                "new-only", encoding="utf-8"
            )
            (live / "index.html").write_bytes(old_index)
            (live / "static/js/app.js").write_text("old-app", encoding="utf-8")
            (live / "static/js/old-only.js").write_text(
                "old-only", encoding="utf-8"
            )
            ready.write_text(json.dumps(old_marker), encoding="utf-8")

            script = f"""
                import {{ writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                    pruneLive: true,
                  }},
                  {{
                    replaceReadyMarker: async (readyFile) => {{
                      await writeFile(readyFile, 'corrupt-marker', 'utf8');
                      throw new Error('injected ready marker failure');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("injected ready marker failure", result.stderr)
            self.assertEqual(old_index, (live / "index.html").read_bytes())
            self.assertEqual(old_marker, json.loads(ready.read_text(encoding="utf-8")))
            self.assertEqual(
                "old-app", (live / "static/js/app.js").read_text(encoding="utf-8")
            )
            self.assertEqual(
                "old-only",
                (live / "static/js/old-only.js").read_text(encoding="utf-8"),
            )
            self.assertFalse((live / "static/new-revision/new-only.js").exists())
            self.assertFalse((live / "static/new-revision").exists())

    def test_web_dist_publisher_commits_ready_marker_after_prune_completes(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            stale_asset = live / "static/js/stale.js"
            (staging / "static/js").mkdir(parents=True)
            stale_asset.parent.mkdir(parents=True)
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")
            stale_asset.write_text("stale", encoding="utf-8")

            script = f"""
                import {{ access, writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                    pruneLive: true,
                  }},
                  {{
                    replaceReadyMarker: async (readyFile, marker) => {{
                      try {{
                        await access({json.dumps(str(stale_asset))});
                        throw new Error('ready marker committed before prune completed');
                      }} catch (error) {{
                        if (error?.code !== 'ENOENT') {{
                          throw error;
                        }}
                      }}
                      await writeFile(readyFile, marker, 'utf8');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertFalse(stale_asset.exists())
            self.assertTrue(ready.is_file())

    def test_web_dist_publisher_rolls_back_index_and_marker_when_marker_commit_fails(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            staging.mkdir()
            live.mkdir()
            old_index = b"old-index"
            old_marker = {
                "indexSha256": hashlib.sha256(old_index).hexdigest(),
                "publishedAt": "2026-07-28T00:00:00.000Z",
            }
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_bytes(old_index)
            ready.write_text(json.dumps(old_marker), encoding="utf-8")

            script = f"""
                import {{ writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                  }},
                  {{
                    replaceReadyMarker: async (readyFile) => {{
                      await writeFile(readyFile, 'corrupt-marker', 'utf8');
                      throw new Error('injected ready marker failure');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("injected ready marker failure", result.stderr)
            self.assertEqual(old_index, (live / "index.html").read_bytes())
            self.assertEqual(old_marker, json.loads(ready.read_text(encoding="utf-8")))
            residue = [
                item.name
                for item in root.rglob("*")
                if ".next-" in item.name
                or ".publish-" in item.name
                or item.name == ".gateway-web-publish.lock"
            ]
            self.assertEqual([], residue)

    def test_web_dist_publisher_prune_mode_removes_stale_live_assets(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")
            (live / "static/js/old.js").write_text("old", encoding="utf-8")

            result = subprocess.run(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                    "--prune",
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertFalse((live / "static/js/old.js").exists())
            self.assertTrue((live / "static/js/new.js").is_file())

    def test_web_dist_publisher_retries_transient_cleanup_failures(self):
        publisher = (
            GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        ).read_text(encoding="utf-8")
        cleanup = (
            GATEWAY_ROOT / "apps/desktop/tools/web-publish-cleanup.mjs"
        ).read_text(encoding="utf-8")

        self.assertIn("CLEANUP_MAX_ATTEMPTS", cleanup)
        self.assertIn("CLEANUP_RETRY_CODES", cleanup)
        self.assertIn("async function removePathWithRetry", cleanup)
        self.assertIn('from "./web-publish-cleanup.mjs"', publisher)
        self.assertIn("await removePathWithRetry", publisher)
