import hashlib
import json
import pathlib
import tempfile
import unittest

from gateway_package_fixture import GatewayPackageFixture


class GatewayPackageLayoutContractTests(GatewayPackageFixture):
    def test_staged_package_has_deterministic_gateway_layout_and_metadata(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            first = self._run_packager(source_root, release_root)
            self.assertEqual(
                first.returncode,
                0,
                msg=f"stdout:\n{first.stdout}\nstderr:\n{first.stderr}",
            )

            destination = release_root / "contract-v1"
            self.assertTrue(destination.is_dir())
            self.assertEqual([destination], list(release_root.iterdir()))

            required_files = {
                "gateway.exe",
                "gateway-ui.exe",
                "routes.yaml",
                "routes.example.yaml",
                ".env.example",
                "gateway-build-provenance.json",
                "manifests/schema/gateway.json",
                "manifests/lines/example/official.json",
                "scripts/worker.mjs",
                "scripts/package.json",
                "scripts/invoke-gateway-live-provider-canary.ps1",
                "docs/provider-inventory.json",
                "docs/operations-manual.md",
                "docs/evidence/README.md",
                "deploy/.env.example",
                "deploy/README.md",
                "deploy/docker-compose.local.yml",
                "deploy/docker-compose.yml",
                "deploy/docker-deploy.sh",
                "deploy/docker-entrypoint.sh",
                "deploy/postgres/initdb/001-gateway-schema.sql",
                "deploy/postgres/initdb/002-gateway-bootstrap.sql",
                "deploy/postgres/initdb/003-gateway-standalone-constraints.sql",
                "tools/smoke-gateway-packaged-runtime.ps1",
                "manifest.json",
                "checksums.sha256",
            }
            actual_files = {
                path.relative_to(destination).as_posix()
                for path in destination.rglob("*")
                if path.is_file()
            }
            self.assertTrue(required_files.issubset(actual_files))

            for forbidden in (
                "scripts/node_modules",
                "scripts/.runtime",
                "scripts/output",
                "tools/line-evidence",
                "deploy/.env",
                "deploy/gateway_data",
                "deploy/redis_data",
                "deploy/docker-compose.dev.yml",
                "deploy/docker-dev-entrypoint.sh",
            ):
                self.assertFalse((destination / pathlib.PurePosixPath(forbidden)).exists())

            manifest = json.loads((destination / "manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(manifest["schemaVersion"], 1)
            self.assertEqual(manifest["app"], "Gateway")
            self.assertEqual(manifest["sourceProject"], "Gateway")
            self.assertEqual(manifest["versionId"], "contract-v1")
            self.assertEqual(manifest["checksums"], "checksums.sha256")
            self.assertIn("sourceRevision", manifest)
            self.assertIn("buildTimestamp", manifest)
            self.assertIn("packagerVersion", manifest)
            self.assertIn("sourceTreeFingerprint", manifest)
            self.assertIn("sourceTreeDirty", manifest)
            self.assertIn("sourceTree", manifest)

            executable_records = {record["name"]: record for record in manifest["exes"]}
            self.assertEqual(
                set(executable_records),
                {"gateway.exe", "gateway-ui.exe"},
            )
            support_records = {
                record["path"]: record for record in manifest["supportFiles"]
            }
            support_paths = set(support_records)
            self.assertIn("routes.yaml", support_paths)
            self.assertIn("routes.example.yaml", support_paths)
            self.assertIn(".env.example", support_paths)
            self.assertEqual(
                support_records[".env.example"]["kind"],
                "environment-template",
            )
            self.assertIn("gateway-build-provenance.json", support_paths)
            self.assertEqual(
                support_records["gateway-build-provenance.json"]["kind"],
                "build-provenance",
            )
            self.assertEqual(manifest["layout"]["environmentTemplate"], ".env.example")
            self.assertEqual(
                manifest["layout"]["buildProvenance"],
                "gateway-build-provenance.json",
            )
            self.assertNotIn("routes.fixture.yaml", support_paths)
            self.assertFalse((destination / "routes.fixture.yaml").exists())
            self.assertIn("manifests/schema/gateway.json", support_paths)
            self.assertIn("scripts/worker.mjs", support_paths)
            for relative in required_files:
                if relative.startswith("deploy/postgres/initdb/"):
                    self.assertIn(relative, support_paths)
            self.assertIn("scripts/invoke-gateway-live-provider-canary.ps1", support_paths)
            self.assertIn("docs/provider-inventory.json", support_paths)
            self.assertIn("docs/evidence/README.md", support_paths)
            self.assertNotIn("docs/evidence/history.json", support_paths)
            self.assertFalse((destination / "docs/evidence/history.json").exists())
            for internal_directory in ("analysis", "plan", "progress", "superpowers"):
                self.assertFalse((destination / "docs" / internal_directory).exists())
            self.assertIn("tools/smoke-gateway-packaged-runtime.ps1", support_paths)
            self.assertNotIn("tools/run-gateway-line-evidence.ps1", support_paths)
            self.assertFalse((destination / "tools/run-gateway-line-evidence.ps1").exists())

            for record in [*manifest["exes"], *manifest["supportFiles"]]:
                relative = pathlib.PurePosixPath(record["path"])
                self.assertFalse(relative.is_absolute())
                self.assertNotIn("..", relative.parts)
                self.assertNotIn("\\", record["path"])
                payload = destination / pathlib.PurePath(*relative.parts)
                self.assertEqual(record["bytes"], payload.stat().st_size)
                self.assertEqual(
                    record["sha256"],
                    hashlib.sha256(payload.read_bytes()).hexdigest(),
                )

            checksum_entries = {}
            for line in (destination / "checksums.sha256").read_text(encoding="utf-8").splitlines():
                digest, relative = line.split("  ", 1)
                checksum_entries[relative] = digest
            for relative in sorted(actual_files - {"checksums.sha256"}):
                payload = destination / pathlib.PurePath(*pathlib.PurePosixPath(relative).parts)
                self.assertEqual(
                    checksum_entries[relative],
                    hashlib.sha256(payload.read_bytes()).hexdigest(),
                )

            source_provenance = source_root / "target/release/gateway-build-provenance.json"
            packaged_provenance = destination / "gateway-build-provenance.json"
            evidence_provenance = (
                source_root
                / "target/release-evidence/contract-v1/gateway-build-provenance.json"
            )
            self.assertEqual(source_provenance.read_bytes(), packaged_provenance.read_bytes())
            self.assertEqual(packaged_provenance.read_bytes(), evidence_provenance.read_bytes())
            self.assertIn(".env.example", checksum_entries)
            self.assertIn("gateway-build-provenance.json", checksum_entries)

            manifest_bytes = (destination / "manifest.json").read_bytes()
            checksums_bytes = (destination / "checksums.sha256").read_bytes()
            second = self._run_packager(source_root, release_root)
            self.assertNotEqual(second.returncode, 0)
            self.assertEqual(manifest_bytes, (destination / "manifest.json").read_bytes())
            self.assertEqual(checksums_bytes, (destination / "checksums.sha256").read_bytes())
            self.assertFalse(any(path.name.startswith(".contract-v1.staging-") for path in release_root.iterdir()))


if __name__ == "__main__":
    unittest.main()
