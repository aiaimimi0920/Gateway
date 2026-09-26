import hashlib
import json
import os
import pathlib
import subprocess
import tempfile
import time
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneWebReadinessTests(unittest.TestCase):
    def test_web_dist_publisher_keeps_previous_assets_until_new_index_is_published(self):
        publisher = (
            GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        )

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            (staging / "index.html").write_text(
                '<script src="/ui/static/js/new.js"></script>', encoding="utf-8"
            )
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text(
                '<script src="/ui/static/js/old.js"></script>', encoding="utf-8"
            )
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
            self.assertEqual(
                '<script src="/ui/static/js/new.js"></script>',
                (live / "index.html").read_text(encoding="utf-8"),
            )
            self.assertEqual(
                "new", (live / "static/js/new.js").read_text(encoding="utf-8")
            )
            self.assertEqual(
                "old", (live / "static/js/old.js").read_text(encoding="utf-8")
            )
            self.assertTrue(ready.is_file())

    def test_web_dist_ready_marker_hashes_every_published_file(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (staging / "static/css").mkdir(parents=True)
            (staging / "index.html").write_text(
                '<link rel="icon" href="/ui/favicon.svg">'
                '<link rel="stylesheet" href="/ui/static/css/app.css">'
                '<script src="/ui/static/js/app.js"></script>',
                encoding="utf-8",
            )
            (staging / "favicon.svg").write_text("icon", encoding="utf-8")
            (staging / "static/css/app.css").write_text(
                "body { color: white; }", encoding="utf-8"
            )
            (staging / "static/js/app.js").write_text(
                "console.log('ready');", encoding="utf-8"
            )

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
            marker = json.loads(ready.read_text(encoding="utf-8"))
            expected_files = {
                path.relative_to(staging).as_posix(): hashlib.sha256(
                    path.read_bytes()
                ).hexdigest()
                for path in sorted(staging.rglob("*"))
                if path.is_file()
            }
            self.assertEqual(1, marker.get("schemaVersion"))
            self.assertEqual(expected_files, marker.get("files"))
            self.assertEqual(
                expected_files["index.html"], marker.get("indexSha256")
            )

    def test_web_dist_publisher_invalidates_old_marker_while_lock_is_held(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock_owner = root / ".gateway-web-publish.lock" / "owner.json"
            staging.mkdir()
            live.mkdir()
            old_index = b"old-index"
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_bytes(old_index)
            ready.write_text(
                json.dumps(
                    {
                        "indexSha256": hashlib.sha256(old_index).hexdigest(),
                        "publishedAt": "2026-07-28T00:00:00.000Z",
                    }
                ),
                encoding="utf-8",
            )

            script = f"""
                import {{ access, writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                  }},
                  {{
                    replaceReadyMarker: async (readyFile, marker) => {{
                      try {{
                        await access(readyFile);
                        throw new Error('old ready marker remained valid while publish lock was held');
                      }} catch (error) {{
                        if (error?.code !== 'ENOENT') {{
                          throw error;
                        }}
                      }}
                      await access({json.dumps(str(publish_lock_owner))});
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
            self.assertTrue(ready.is_file())

    def test_web_dist_publisher_does_not_steal_an_old_lock_without_owner_release(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock = root / ".gateway-web-publish.lock"
            staging.mkdir()
            live.mkdir()
            publish_lock.mkdir()
            lock_owner = publish_lock / "owner.json"
            lock_owner.write_text(
                json.dumps({"ownerToken": "active-owner"}), encoding="utf-8"
            )
            old_timestamp = time.time() - 120
            os.utime(publish_lock, (old_timestamp, old_timestamp))
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")

            process = subprocess.Popen(
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
            try:
                time.sleep(0.3)
                self.assertIsNone(
                    process.poll(), "publisher ignored the active publish lock"
                )
                self.assertEqual(
                    "old-index", (live / "index.html").read_text(encoding="utf-8")
                )
                lock_owner.unlink()
                publish_lock.rmdir()
                stdout, stderr = process.communicate(timeout=30)
            finally:
                if publish_lock.exists():
                    if lock_owner.exists():
                        lock_owner.unlink()
                    publish_lock.rmdir()
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
                process.communicate(timeout=10)

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

    def test_web_dist_publisher_recovers_a_lock_owned_by_a_dead_process(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        exited_owner = subprocess.Popen(["node", "-e", "process.exit(0)"])
        exited_owner.wait(timeout=10)

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock = root / ".gateway-web-publish.lock"
            staging.mkdir()
            live.mkdir()
            publish_lock.mkdir()
            (publish_lock / "owner.json").write_text(
                json.dumps(
                    {
                        "ownerToken": "dead-owner",
                        "pid": exited_owner.pid,
                        "acquiredAt": "2026-07-28T00:00:00.000Z",
                    }
                ),
                encoding="utf-8",
            )
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")

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
            self.assertFalse(publish_lock.exists())
            self.assertEqual("new-index", (live / "index.html").read_text(encoding="utf-8"))
            self.assertTrue(ready.is_file())

    def test_web_dist_publisher_recovers_an_incomplete_owner_after_grace(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock = root / ".gateway-web-publish.lock"
            staging.mkdir()
            live.mkdir()
            publish_lock.mkdir()
            lock_owner = publish_lock / "owner.json"
            lock_owner.write_bytes(b"")
            old_timestamp = time.time() - 120
            os.utime(lock_owner, (old_timestamp, old_timestamp))
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")

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
            self.assertFalse(publish_lock.exists())
            self.assertEqual("new-index", (live / "index.html").read_text(encoding="utf-8"))
            self.assertTrue(ready.is_file())
