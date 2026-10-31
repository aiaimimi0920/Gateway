import hashlib
import json
import pathlib
import tempfile
import unittest

from gateway_package_fixture import GatewayPackageFixture


class GeminiWorkerRuntimePackageTests(GatewayPackageFixture):
    def worker_fixture(self, source):
        self._write_fixture(source)
        scripts = source / "scripts"
        dependencies = {"playwright-core": "1.59.1", "ws": "8.21.0",
                        "selfsigned": "5.5.0", "@aws-sdk/client-s3": "3.1033.0"}
        (scripts / "package.json").write_text(
            json.dumps({"type": "module", "dependencies": dependencies}), encoding="utf-8")
        packages = {"": {"dependencies": dependencies}}
        for name, version in dependencies.items():
            package = scripts / "node_modules" / name
            package.mkdir(parents=True)
            packages[f"node_modules/{name}"] = {"version": version}
            (package / "package.json").write_text(json.dumps(
                {"name": name, "version": version, "main": "index.cjs"}), encoding="utf-8")
            code = "exports.chromium = { launch() {} };" if name == "playwright-core" else "module.exports = {};"
            (package / "index.cjs").write_text(code, encoding="utf-8")
        (scripts / "package-lock.json").write_text(
            json.dumps({"lockfileVersion": 3, "packages": packages}), encoding="utf-8")
        (scripts / "gemini-canvas-browser-pool.mjs").write_text(
            "import 'ws'; import 'selfsigned'; import '@aws-sdk/client-s3';\n"
            "if(process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN !== '1') throw Error('main not suppressed');\n",
            encoding="utf-8")

    def test_gemini_runtime_closure_is_imported_hashed_and_recorded_by_official_packager(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source"
            self.worker_fixture(source)
            result = self._run_packager(source, root / "release")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("locked Gemini worker import passed (main suppressed)", result.stdout)
            payload = root / "release/contract-v1"
            manifest = json.loads((payload / "manifest.json").read_text(encoding="utf-8"))
            records = {entry["path"]: entry for entry in manifest["supportFiles"]}
            for name in ("ws", "selfsigned", "@aws-sdk/client-s3", "playwright-core"):
                relative = f"scripts/node_modules/{name}/index.cjs"
                self.assertEqual(records[relative]["sha256"], hashlib.sha256((payload / relative).read_bytes()).hexdigest())
            self.assertFalse((payload / "scripts/node_modules/should-not-ship.txt").exists())

    def test_missing_gemini_runtime_dependency_prevents_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source"
            self.worker_fixture(source)
            (source / "scripts/node_modules/ws/package.json").unlink()
            result = self._run_packager(source, root / "release")
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "release/contract-v1").exists())


if __name__ == "__main__":
    unittest.main()
