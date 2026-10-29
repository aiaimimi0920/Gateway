import hashlib
import json
import pathlib
import tempfile
import unittest

from gateway_package_fixture import GatewayPackageFixture


class ChatgptBrowserRuntimePackageTests(GatewayPackageFixture):
    def browser_fixture(self, source):
        self._write_fixture(source)
        scripts = source / "scripts"
        dependencies = {"playwright-core": "1.59.1"}
        (scripts / "package.json").write_text(json.dumps({"dependencies": dependencies}), encoding="utf-8")
        lock = {"lockfileVersion": 3, "packages": {"": {"dependencies": dependencies},
                "node_modules/playwright-core": {"version": "1.59.1"}}}
        (scripts / "package-lock.json").write_text(json.dumps(lock), encoding="utf-8")
        package = scripts / "node_modules/playwright-core"
        package.mkdir(parents=True)
        (package / "package.json").write_text(json.dumps({"name": "playwright-core", "version": "1.59.1", "main": "index.cjs"}), encoding="utf-8")
        (package / "index.cjs").write_text("exports.chromium = { launch() {} };\n", encoding="utf-8")
        (package / "tests").mkdir()
        (package / "tests/required-runtime-file.json").write_text("{}", encoding="utf-8")
        return package

    def test_browser_runtime_is_copied_intact_and_hashed_without_unlocked_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source"
            package = self.browser_fixture(source)
            result = self._run_packager(source, root / "release")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            destination = root / "release/contract-v1"
            relative = "scripts/node_modules/playwright-core/tests/required-runtime-file.json"
            self.assertEqual((destination / relative).read_bytes(), (package / "tests/required-runtime-file.json").read_bytes())
            self.assertFalse((destination / "scripts/node_modules/should-not-ship.txt").exists())
            manifest = json.loads((destination / "manifest.json").read_text(encoding="utf-8"))
            records = {entry["path"]: entry for entry in manifest["supportFiles"]}
            digest = hashlib.sha256((destination / relative).read_bytes()).hexdigest()
            self.assertEqual(records[relative]["sha256"], digest)
            self.assertIn(digest + "  " + relative, (destination / "checksums.sha256").read_text(encoding="ascii"))

    def test_mismatched_missing_and_new_dependency_runtime_are_rejected(self):
        for scenario in ("version", "missing", "new-dependency"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                source = root / "source"
                package = self.browser_fixture(source)
                if scenario == "version":
                    (package / "package.json").write_text('{"name":"playwright-core","version":"0.0.0"}', encoding="utf-8")
                elif scenario == "missing":
                    (package / "package.json").unlink()
                else:
                    lock_path = source / "scripts/package-lock.json"
                    lock = json.loads(lock_path.read_text(encoding="utf-8"))
                    lock["packages"]["node_modules/playwright-core"]["dependencies"] = {"unexpected": "1"}
                    lock_path.write_text(json.dumps(lock), encoding="utf-8")
                result = self._run_packager(source, root / "release")
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((root / "release/contract-v1").exists())


if __name__ == "__main__":
    unittest.main()
