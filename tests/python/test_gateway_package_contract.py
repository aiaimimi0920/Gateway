import hashlib
import json
import pathlib
import subprocess
import tempfile
import time
import unittest

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
PACKAGER = GATEWAY_ROOT / "tools" / "package-gateway-release.ps1"
RUNTIME_SMOKE = GATEWAY_ROOT / "tools" / "smoke-gateway-packaged-runtime.ps1"
UI_SMOKE = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"


class GatewayPackageContractTests(unittest.TestCase):
    def _write_fixture(self, root: pathlib.Path) -> None:
        files = {
            "target/release/neuro-gateway.exe": b"gateway-binary-v1\n",
            "apps/desktop/src-tauri/target/release/neuro-gateway-ui.exe": b"gateway-ui-binary-v1\n",
            "routes.yaml": b"routes: []\n",
            "routes.example.yaml": b"routes: []\n",
            ".env.example": b"GATEWAY_RUNTIME_ROLE=standalone\n",
            "routes.fixture.yaml": b"routes: [fixture-only]\n",
            "manifests/schema/gateway.json": b'{"schemaVersion": 1}\n',
            "manifests/lines/example/official.json": b'{"id": "example"}\n',
            "scripts/worker.mjs": b"export default {};\n",
            "scripts/package.json": b'{"name": "gateway-workers"}\n',
            "scripts/invoke-gateway-live-provider-canary.ps1": b"param()\n",
            "scripts/node_modules/should-not-ship.txt": b"excluded\n",
            "scripts/.runtime/state.json": b"excluded\n",
            "scripts/output/result.json": b"excluded\n",
            "docs/provider-inventory.json": b'{"schemaVersion":"gateway-product-inventory/v1"}\n',
            "docs/operations-manual.md": b"# Operations\n",
            "docs/release/source-notes.md": b"# Release source notes\n",
            "docs/evidence/history.json": b'{"artifactPaths":["C:/Users/Public/host-only.log"]}\n',
            "docs/evidence/README.md": b"# Evidence\n",
            "docs/analysis/project-overview.md": b"C:/Users/Public/internal-analysis\n",
            "docs/plan/task-breakdown.md": b"C:/Users/Public/internal-plan\n",
            "docs/progress/MASTER.md": b"C:/Users/Public/internal-progress\n",
            "docs/superpowers/specs/design.md": b"C:/Users/Public/internal-spec\n",
            "tools/run-gateway-line-evidence.ps1": b"param()\n",
            "tools/smoke-gateway-packaged-runtime.ps1": b"param()\n",
        }
        for relative, content in files.items():
            path = root / pathlib.PurePosixPath(relative)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)

    def _run_packager(
        self,
        source_root: pathlib.Path,
        release_root: pathlib.Path,
        *,
        write_provenance: bool = True,
        allow_custom_release_root: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        if write_provenance:
            self._write_build_provenance(source_root)
        command = [
            powershell_executable(),
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(PACKAGER),
            "-SourceRoot",
            str(source_root),
            "-VersionId",
            "contract-v1",
            "-ReleaseRoot",
            str(release_root),
            "-StageOnly",
            "-SkipBuild",
            "-Clean",
        ]
        if allow_custom_release_root:
            command.append("-AllowCustomReleaseRoot")
        return subprocess.run(
            command,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def _source_tree_state(self, source_root: pathlib.Path) -> dict[str, object]:
        excluded = {".git", "target", "node_modules", ".runtime", "output"}
        records = []
        algorithm = "sha256-file-list-v1"
        if (source_root / ".git").exists():
            result = subprocess.run(
                [
                    "git",
                    "-C",
                    str(source_root),
                    "-c",
                    "core.quotepath=false",
                    "ls-files",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            paths = [source_root / path for path in result.stdout.splitlines() if path]
            algorithm = "sha256-git-source-list-v2"
        else:
            paths = source_root.rglob("*")
        for path in paths:
            if not path.is_file():
                continue
            relative = path.relative_to(source_root)
            parts = tuple(part.lower() for part in relative.parts)
            if any(
                part in excluded
                or part.startswith("tmp-")
                or (index == 0 and part == "release")
                for index, part in enumerate(parts)
            ):
                continue
            payload = path.read_bytes()
            records.append(
                f"{relative.as_posix()}\t{len(payload)}\t{hashlib.sha256(payload).hexdigest()}\n"
            )
        return {
            "algorithm": algorithm,
            "fingerprint": hashlib.sha256(
                "".join(sorted(records)).encode("utf-8")
            ).hexdigest(),
            "fileCount": len(records),
            "dirty": None,
        }

    def _source_tree_fingerprint(self, source_root: pathlib.Path) -> str:
        return str(self._source_tree_state(source_root)["fingerprint"])

    def test_packager_source_fingerprint_ignores_git_ignored_local_files(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            (source_root / ".gitignore").write_text(
                "/.env\n/apps/desktop/dist/\n__pycache__/\n",
                encoding="utf-8",
            )
            ignored_files = {
                ".env": "GATEWAY_SECRET=fixture-only\n",
                "apps/desktop/dist/bundle.js": "generated-v1\n",
                "tests/python/__pycache__/contract.pyc": "cache-v1\n",
            }
            for relative, content in ignored_files.items():
                path = source_root / pathlib.PurePosixPath(relative)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")

            subprocess.run(
                ["git", "init"],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                ["git", "add", "."],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Gateway Contract",
                    "-c",
                    "user.email=gateway-contract@example.invalid",
                    "commit",
                    "-m",
                    "fixture",
                ],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            self._write_build_provenance(source_root)

            for relative in ignored_files:
                path = source_root / pathlib.PurePosixPath(relative)
                path.write_text(path.read_text(encoding="utf-8") + "changed\n", encoding="utf-8")

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            manifest = json.loads(
                (release_root / "contract-v1" / "manifest.json").read_text(encoding="utf-8")
            )
            self.assertEqual(
                manifest["sourceTreeFingerprint"],
                self._source_tree_fingerprint(source_root),
            )

    def _write_build_provenance(self, source_root: pathlib.Path) -> None:
        # The real build script writes this file. Fixture packages include a
        # deliberately matching provenance record so -SkipBuild cannot bypass
        # the stale-artifact check.
        source_tree = self._source_tree_state(source_root)
        provenance_path = source_root / "target" / "release" / "gateway-build-provenance.json"
        provenance_path.parent.mkdir(parents=True, exist_ok=True)
        provenance_path.write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "sourceTreeFingerprint": source_tree["fingerprint"],
                    "sourceTreeDirty": source_tree["dirty"],
                    "sourceTree": source_tree,
                    "artifacts": [
                        {
                            "path": "target/release/neuro-gateway.exe",
                            "bytes": (source_root / "target/release/neuro-gateway.exe").stat().st_size,
                            "sha256": hashlib.sha256(
                                (source_root / "target/release/neuro-gateway.exe").read_bytes()
                            ).hexdigest(),
                        },
                        {
                            "path": "apps/desktop/src-tauri/target/release/neuro-gateway-ui.exe",
                            "bytes": (source_root / "apps/desktop/src-tauri/target/release/neuro-gateway-ui.exe").stat().st_size,
                            "sha256": hashlib.sha256(
                                (source_root / "apps/desktop/src-tauri/target/release/neuro-gateway-ui.exe").read_bytes()
                            ).hexdigest(),
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )

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
                "neuro-gateway.exe",
                "neuro-gateway-ui.exe",
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
                {"neuro-gateway.exe", "neuro-gateway-ui.exe"},
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

    def test_repository_local_release_root_does_not_change_source_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            source_root = pathlib.Path(temporary_directory) / "Gateway-source"
            release_root = source_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                allow_custom_release_root=False,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            destination = release_root / "contract-v1"
            self.assertTrue(destination.is_dir())
            manifest = json.loads(
                (destination / "manifest.json").read_text(encoding="utf-8")
            )
            provenance = json.loads(
                (destination / "gateway-build-provenance.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(
                manifest["sourceTreeFingerprint"],
                provenance["sourceTreeFingerprint"],
            )

    def test_gateway_source_owns_packaged_canary_and_runner_resolves_it_locally(self):
        canary = GATEWAY_ROOT / "scripts" / "invoke-gateway-live-provider-canary.ps1"
        runner = GATEWAY_ROOT / "tools" / "run-gateway-line-evidence.ps1"

        self.assertTrue(canary.is_file())
        self.assertTrue((GATEWAY_ROOT / ".env.example").is_file())
        self.assertIn(
            'Join-Path $GatewayRoot "scripts\\invoke-gateway-live-provider-canary.ps1"',
            runner.read_text(encoding="utf-8"),
        )

    def test_portable_docs_mark_the_full_evidence_runner_as_source_only(self):
        documentation_paths = (
            GATEWAY_ROOT / "README.md",
            GATEWAY_ROOT / "docs" / "operations-manual.md",
            GATEWAY_ROOT / "docs" / "provider-evidence.md",
            GATEWAY_ROOT / "docs" / "evidence" / "README.md",
        )

        for path in documentation_paths:
            content = path.read_text(encoding="utf-8")
            self.assertIn("source repository only", content, msg=str(path))
            self.assertIn(
                "scripts/invoke-gateway-live-provider-canary.ps1",
                content,
                msg=str(path),
            )

    def test_skip_build_rejects_missing_or_stale_provenance_without_partial_output(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            # No provenance at all: packaging must fail before creating a
            # destination or leaving a staging directory behind.
            provenance = source_root / "target" / "release" / "gateway-build-provenance.json"
            result = self._run_packager_without_provenance(source_root, release_root)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(release_root.exists())
            self.assertFalse(provenance.exists())

            self._write_build_provenance(source_root)
            payload = json.loads(provenance.read_text(encoding="utf-8"))
            payload["sourceTreeFingerprint"] = "0" * 64
            provenance.write_text(json.dumps(payload), encoding="utf-8")
            result = self._run_packager(source_root, release_root, write_provenance=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((release_root / "contract-v1").exists())

    def test_skip_build_rejects_inconsistent_nested_source_provenance(self):
        mutations = {
            "algorithm": "sha256-incompatible-source-list",
            "fingerprint": "f" * 64,
            "fileCount": -1,
        }
        for field, invalid_value in mutations.items():
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary_directory:
                temporary_root = pathlib.Path(temporary_directory)
                source_root = temporary_root / "Gateway-source"
                release_root = temporary_root / "release" / "Gateway"
                source_root.mkdir(parents=True)
                self._write_fixture(source_root)
                self._write_build_provenance(source_root)
                provenance = (
                    source_root / "target" / "release" / "gateway-build-provenance.json"
                )
                payload = json.loads(provenance.read_text(encoding="utf-8"))
                payload["sourceTree"][field] = invalid_value
                provenance.write_text(json.dumps(payload), encoding="utf-8")

                result = self._run_packager(
                    source_root,
                    release_root,
                    write_provenance=False,
                )

                self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
                self.assertIn("build provenance", (result.stdout + result.stderr).lower())
                self.assertFalse((release_root / "contract-v1").exists())

    def test_existing_evidence_provenance_blocks_release_without_overwrite(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            self._write_build_provenance(source_root)

            evidence_provenance = (
                source_root
                / "target/release-evidence/contract-v1/gateway-build-provenance.json"
            )
            evidence_provenance.parent.mkdir(parents=True)
            sentinel = b"immutable-evidence-sentinel\n"
            evidence_provenance.write_bytes(sentinel)

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("immutable", result.stderr.lower())
            self.assertEqual(evidence_provenance.read_bytes(), sentinel)
            self.assertFalse((release_root / "contract-v1").exists())
            if release_root.exists():
                self.assertFalse(
                    any(
                        path.name.startswith(".contract-v1.staging-")
                        for path in release_root.iterdir()
                    )
                )

    def test_packager_rolls_back_a_new_release_when_evidence_publish_fails(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            self._write_build_provenance(source_root)

            evidence_root = source_root / "target" / "release-evidence"
            sentinel = b"evidence-parent-obstruction\n"
            evidence_root.write_bytes(sentinel)

            failed = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(failed.returncode, 0)
            normalized_failure = " ".join((failed.stdout + failed.stderr).lower().split())
            self.assertIn(
                "evidence provenance publication failed; the new release was rolled back",
                normalized_failure,
            )
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertTrue(evidence_root.is_file())
            self.assertEqual(evidence_root.read_bytes(), sentinel)
            if release_root.exists():
                self.assertFalse(any(release_root.glob(".contract-v1.staging-*")))

            evidence_root.unlink()
            retry = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )
            self.assertEqual(
                retry.returncode,
                0,
                msg=f"stdout:\n{retry.stdout}\nstderr:\n{retry.stderr}",
            )
            self.assertTrue((release_root / "contract-v1").is_dir())

    def test_packager_rejects_source_changes_during_staging(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            for index in range(250):
                path = source_root / "scripts" / f"mutation-padding-{index:03d}.txt"
                path.write_bytes((f"padding-{index}\n" * 64).encode("ascii"))
            self._write_build_provenance(source_root)

            command = [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(PACKAGER),
                "-SourceRoot",
                str(source_root),
                "-VersionId",
                "contract-v1",
                "-ReleaseRoot",
                str(release_root),
                "-StageOnly",
                "-SkipBuild",
                "-AllowCustomReleaseRoot",
            ]
            process = subprocess.Popen(
                command,
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            staging_path = None
            deadline = time.monotonic() + 30
            while process.poll() is None and time.monotonic() < deadline:
                if release_root.exists():
                    staging_candidates = list(release_root.glob(".contract-v1.staging-*"))
                    if staging_candidates:
                        staging_path = staging_candidates[0]
                        break
                time.sleep(0.002)

            self.assertIsNotNone(staging_path, "packager completed before staging could be observed")
            release_source = source_root / "docs" / "release" / "source-notes.md"
            release_source.write_bytes(
                release_source.read_bytes() + b"changed during staging\n"
            )
            stdout, stderr = process.communicate(timeout=60)

            self.assertNotEqual(process.returncode, 0, msg=stdout + stderr)
            self.assertIn("source tree changed during packaging", (stdout + stderr).lower())
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertFalse(
                (
                    source_root
                    / "target/release-evidence/contract-v1/gateway-build-provenance.json"
                ).exists()
            )
            self.assertFalse(
                any(release_root.glob(".contract-v1.staging-*")),
                msg=f"staging directory was not cleaned: {staging_path}",
            )

    def test_missing_environment_template_creates_no_release_or_evidence(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            (source_root / ".env.example").unlink()
            self._write_build_provenance(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("environment template", result.stderr.lower())
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertFalse(
                (
                    source_root
                    / "target/release-evidence/contract-v1/gateway-build-provenance.json"
                ).exists()
            )

    def _run_packager_without_provenance(self, source_root: pathlib.Path, release_root: pathlib.Path):
        command = [
            powershell_executable(),
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(PACKAGER),
            "-SourceRoot",
            str(source_root),
            "-VersionId",
            "contract-v1",
            "-ReleaseRoot",
            str(release_root),
            "-StageOnly",
            "-SkipBuild",
            "-AllowCustomReleaseRoot",
        ]
        return subprocess.run(
            command,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_custom_release_root_requires_explicit_test_opt_in(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                allow_custom_release_root=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("AllowCustomReleaseRoot", result.stderr + result.stdout)
            self.assertFalse(release_root.exists())

    def _run_smoke(self, script: pathlib.Path, destination: pathlib.Path, *extra: str):
        return subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(script),
                "-ReleaseDir",
                str(destination),
                *extra,
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_runtime_and_ui_integrity_smokes_reject_checksum_path_aliases(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            packaged = self._run_packager(source_root, release_root)
            self.assertEqual(packaged.returncode, 0, msg=packaged.stderr)
            destination = release_root / "contract-v1"

            runtime_valid = self._run_smoke(RUNTIME_SMOKE, destination, "-IntegrityOnly")
            ui_valid = self._run_smoke(UI_SMOKE, destination)
            self.assertEqual(runtime_valid.returncode, 0, msg=runtime_valid.stderr)
            self.assertEqual(ui_valid.returncode, 0, msg=ui_valid.stderr)

            checksums = destination / "checksums.sha256"
            checksum_text = checksums.read_text(encoding="ascii")
            checksums.write_text(
                checksum_text.replace("  neuro-gateway.exe", "  alias-neuro-gateway.exe"),
                encoding="ascii",
                newline="",
            )

            runtime_tampered = self._run_smoke(RUNTIME_SMOKE, destination, "-IntegrityOnly")
            ui_tampered = self._run_smoke(UI_SMOKE, destination)
            self.assertNotEqual(runtime_tampered.returncode, 0)
            self.assertNotEqual(ui_tampered.returncode, 0)
            self.assertIn("missing packaged file", runtime_tampered.stderr + runtime_tampered.stdout)
            self.assertIn("missing packaged file", ui_tampered.stderr + ui_tampered.stdout)


if __name__ == "__main__":
    unittest.main()
