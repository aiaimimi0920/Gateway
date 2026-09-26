import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time
import unittest

try:
    from .powershell_test_utils import powershell_executable
    from .gateway_repository_text_fixture import GatewayRepositoryTextFixture
except ImportError:
    from powershell_test_utils import powershell_executable
    from gateway_repository_text_fixture import GatewayRepositoryTextFixture


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneRepositoryContractTests(GatewayRepositoryTextFixture, unittest.TestCase):

    def test_repository_metadata_and_workflows_exist(self):
        required = [
            ".gitattributes",
            "LICENSE",
            "README.md",
            "README.zh-CN.md",
            "rust-toolchain.toml",
            "Dockerfile.dev",
            "deploy/.env.example",
            "deploy/README.md",
            "deploy/docker-compose.yml",
            "deploy/docker-compose.local.yml",
            "deploy/docker-compose.dev.yml",
            "deploy/docker-deploy.sh",
            "deploy/docker-dev-entrypoint.sh",
            ".github/workflows/ci.yml",
            ".github/workflows/build-windows.yml",
            ".github/workflows/docker.yml",
            ".github/workflows/release-tag.yml",
            "tools/compress-gateway-release.ps1",
            "tools/verify-gateway-line.ps1",
        ]

        missing = [path for path in required if not (GATEWAY_ROOT / path).is_file()]
        self.assertEqual([], missing, f"missing standalone repository files: {missing}")

    def test_cargo_metadata_identifies_the_independent_repository(self):
        cargo = (GATEWAY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        toolchain = (GATEWAY_ROOT / "rust-toolchain.toml").read_text(
            encoding="utf-8"
        )

        self.assertIn('description = "Production AI gateway runtime for Neuro"', cargo)
        self.assertIn('license = "MIT"', cargo)
        self.assertIn('repository = "https://github.com/aiaimimi0920/Gateway"', cargo)
        self.assertIn('readme = "README.md"', cargo)
        self.assertIn('rust-version = "1.91.1"', cargo)
        self.assertIn('channel = "1.91.1"', toolchain)




    def test_packager_defaults_to_repository_local_release_root(self):
        script = (GATEWAY_ROOT / "tools/package-gateway-release.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('Join-Path $gatewayRoot "release\\Gateway"', script)
        self.assertNotIn('Join-Path $PSScriptRoot "..\\..\\release\\Gateway"', script)

    def test_evidence_runner_owns_its_line_verifier(self):
        script = (GATEWAY_ROOT / "tools/run-gateway-line-evidence.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('Join-Path $GatewayRoot "tools\\verify-gateway-line.ps1"', script)
        self.assertNotIn('Join-Path $RepoRoot "deploy\\verify-gateway-line.ps1"', script)

    def test_cli_help_uses_repository_owned_validation_entrypoints(self):
        source = (GATEWAY_ROOT / "src/main.rs").read_text(encoding="utf-8")

        self.assertIn(
            "powershell -File tools/verify-gateway-release-candidate.ps1",
            source,
        )
        self.assertIn(
            "powershell -File tools/verify-gateway-line.ps1 -All",
            source,
        )
        self.assertNotIn("deploy/verify-gateway-line.ps1", source)

    def test_browser_probe_scripts_resolve_the_gateway_root_from_scripts(self):
        probe_scripts = sorted((GATEWAY_ROOT / "scripts").glob("probe-*.mjs"))
        self.assertTrue(probe_scripts, "expected Gateway browser probe scripts")
        for script_path in probe_scripts:
            script = script_path.read_text(encoding="utf-8")
            with self.subTest(script=script_path.name):
                self.assertNotIn('path.resolve(scriptDir, "..", "..")', script)


    def test_readme_documents_independent_clone_and_release(self):
        readme = (GATEWAY_ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("https://github.com/aiaimimi0920/Gateway", readme)
        self.assertIn("git clone", readme)
        self.assertIn(".\\tools\\build-gateway-release.ps1", readme)
        self.assertNotIn("Build and package from the monorepo root", readme)





    def test_docker_helper_scripts_and_release_bundle_are_repository_owned(self):
        required = [
            "tools/deploy-gateway-docker.ps1",
            "tools/verify-gateway-docker-stack.ps1",
            "tools/export-gateway-docker-deploy-bundle.ps1",
        ]
        missing = [path for path in required if not (GATEWAY_ROOT / path).is_file()]
        self.assertEqual([], missing, f"missing Docker helper scripts: {missing}")

        release = (GATEWAY_ROOT / ".github/workflows/release-tag.yml").read_text(
            encoding="utf-8"
        )
        docker = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("verify-gateway-docker-stack.ps1", docker)
        self.assertIn("export-gateway-docker-deploy-bundle.ps1", release)
        self.assertIn("Gateway-${{ env.GATEWAY_TAG }}-docker-deploy.zip", release)
        self.assertIn(
            "release/Gateway/packages/Gateway-${{ env.GATEWAY_TAG }}-docker-deploy.zip.sha256",
            release,
        )

    def test_packager_includes_deploy_directory_in_release_layout(self):
        script = (GATEWAY_ROOT / "tools/package-gateway-release.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('$deploySource = Join-Path $gatewayRoot "deploy"', script)
        self.assertIn("$deployPayloadRelativePaths", script)
        for relative_path in (
            ".env.example",
            "README.md",
            "docker-compose.local.yml",
            "docker-compose.yml",
            "docker-deploy.sh",
            "docker-entrypoint.sh",
        ):
            self.assertIn(f'"{relative_path}"', script)
        self.assertNotIn(
            'Copy-FilteredTree -Source $deploySource -Destination (Join-Path $staging "deploy")',
            script,
        )
        self.assertIn('deploy = "deploy/"', script)
        self.assertIn('-Kind "docker-deploy"', script)

    def test_docker_deploy_bundle_uses_a_release_payload_allowlist(self):
        script = (
            GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"
        ).read_text(encoding="utf-8")

        self.assertIn("$deployPayloadRelativePaths", script)
        for relative_path in (
            ".env.example",
            "README.md",
            "docker-compose.local.yml",
            "docker-compose.yml",
            "docker-deploy.sh",
            "docker-entrypoint.sh",
        ):
            self.assertIn(f'"{relative_path}"', script)
        self.assertNotIn(
            'Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "deploy")',
            script,
        )

    def test_docker_deploy_bundle_rejects_unsafe_version_ids(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                    "-VersionId",
                    "unsafe/version",
                    "-OutputDir",
                    temporary_directory,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("unsupported path characters", result.stdout + result.stderr)
            self.assertEqual([], list(pathlib.Path(temporary_directory).iterdir()))

    def test_docker_deploy_bundle_refuses_to_overwrite_existing_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            command = [
                powershell_executable(),
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                "-VersionId",
                "contract-v1",
                "-OutputDir",
                temporary_directory,
            ]
            first = subprocess.run(
                command,
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )
            self.assertEqual(0, first.returncode, msg=first.stdout + first.stderr)

            output_root = pathlib.Path(temporary_directory)
            zip_path = output_root / "Gateway-contract-v1-docker-deploy.zip"
            hash_path = output_root / "Gateway-contract-v1-docker-deploy.zip.sha256"
            original_zip = zip_path.read_bytes()
            original_hash = hash_path.read_bytes()

            second = subprocess.run(
                command,
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertNotEqual(0, second.returncode)
            self.assertIn("already exists and is immutable", second.stdout + second.stderr)
            self.assertEqual(original_zip, zip_path.read_bytes())
            self.assertEqual(original_hash, hash_path.read_bytes())

    def test_docker_deploy_bundle_resumes_an_interrupted_checksum_publication(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_root = pathlib.Path(temporary_directory)
            version_id = "contract-interrupted-publication"
            zip_path = output_root / f"Gateway-{version_id}-docker-deploy.zip"
            hash_path = pathlib.Path(str(zip_path) + ".sha256")
            journal_path = pathlib.Path(str(zip_path) + ".publishing.json")
            zip_bytes = b"interrupted-but-complete-zip"
            zip_path.write_bytes(zip_bytes)
            zip_hash = hashlib.sha256(zip_bytes).hexdigest()
            hash_record = f"{zip_hash} *{zip_path.name}\n"
            journal_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": 1,
                        "zipName": zip_path.name,
                        "zipSha256": zip_hash,
                        "hashRecord": hash_record,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                    "-VersionId",
                    version_id,
                    "-OutputDir",
                    temporary_directory,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
            self.assertEqual(zip_bytes, zip_path.read_bytes())
            self.assertEqual(hash_record, hash_path.read_text(encoding="utf-8"))
            self.assertFalse(journal_path.exists())

    def test_windows_release_compressor_resumes_an_interrupted_checksum_publication(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            release_dir = root / "release"
            output_dir = root / "packages"
            release_dir.mkdir()
            output_dir.mkdir()
            for name in ("manifest.json", "checksums.sha256", "gateway.exe", "gateway-ui.exe"):
                (release_dir / name).write_bytes(b"fixture")

            version_id = "contract-interrupted-windows-publication"
            zip_path = output_dir / f"Gateway-{version_id}-windows-x64.zip"
            checksum_path = pathlib.Path(str(zip_path) + ".sha256")
            journal_path = pathlib.Path(str(zip_path) + ".publishing.json")
            zip_bytes = b"interrupted-but-complete-windows-zip"
            zip_path.write_bytes(zip_bytes)
            zip_hash = hashlib.sha256(zip_bytes).hexdigest()
            checksum_record = f"{zip_hash}  {zip_path.name}{os.linesep}"
            journal_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": 1,
                        "zipName": zip_path.name,
                        "zipSha256": zip_hash,
                        "checksumRecord": checksum_record,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/compress-gateway-release.ps1"),
                    "-ReleaseDir",
                    str(release_dir),
                    "-OutputDir",
                    str(output_dir),
                    "-VersionId",
                    version_id,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
            self.assertEqual(zip_bytes, zip_path.read_bytes())
            with checksum_path.open("r", encoding="ascii", newline="") as checksum_file:
                self.assertEqual(checksum_record, checksum_file.read())
            self.assertFalse(journal_path.exists())

    def test_docker_deploy_bundle_rejects_late_hash_path_collision_before_publication(
        self,
    ):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            fixture_root = temporary_root / "Gateway"
            fixture_tools = fixture_root / "tools"
            fixture_deploy = fixture_root / "deploy"
            fixture_tools.mkdir(parents=True)
            fixture_deploy.mkdir(parents=True)

            exporter = GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"
            exporter_text = exporter.read_text(encoding="utf-8")
            shutil.copytree(
                GATEWAY_ROOT / "tools/release-publication",
                fixture_tools / "release-publication",
            )
            publish_anchor = (
                "    foreach ($artifactPath in @($zipPath, $hashPath, $journalPath)) {"
            )
            ready_gate = r'''    $testReadyPath = [Environment]::GetEnvironmentVariable("GATEWAY_TEST_EXPORT_READY_PATH")
    $testGatePath = [Environment]::GetEnvironmentVariable("GATEWAY_TEST_EXPORT_GATE_PATH")
    if (
        [string]::IsNullOrWhiteSpace($testReadyPath) -or
        [string]::IsNullOrWhiteSpace($testGatePath)
    ) {
        throw "Test export synchronization paths are required."
    }
    $testReadyTemporaryPath = $testReadyPath + ".write-" + [guid]::NewGuid().ToString("N")
    try {
        Write-Utf8NoBom -Path $testReadyTemporaryPath -Value $stagingRoot
        [System.IO.File]::Move($testReadyTemporaryPath, $testReadyPath)
    }
    finally {
        if (Test-Path -LiteralPath $testReadyTemporaryPath) {
            Remove-Item -LiteralPath $testReadyTemporaryPath -Force
        }
    }
    while (-not (Test-Path -LiteralPath $testGatePath -PathType Leaf)) {
        Start-Sleep -Milliseconds 10
    }

'''
            self.assertEqual(1, exporter_text.count(publish_anchor))
            fixture_exporter = fixture_tools / exporter.name
            fixture_exporter.write_text(
                exporter_text.replace(publish_anchor, ready_gate + publish_anchor),
                encoding="utf-8",
            )
            fixture_files = {
                "README.md": b"x" * (16 * 1024 * 1024),
                "README.zh-CN.md": b"# Gateway\n",
                "LICENSE": b"fixture license\n",
                "tools/deploy-gateway-docker.ps1": b"param()\n",
                "deploy/.env.example": b"IMAGE_TAG=latest\n",
                "deploy/README.md": b"# Deploy\n",
                "deploy/docker-compose.local.yml": b"services: {}\n",
                "deploy/docker-compose.yml": b"services: {}\n",
                "deploy/docker-deploy.sh": b"#!/usr/bin/env bash\n",
                "deploy/docker-entrypoint.sh": b"#!/usr/bin/env sh\n",
            }
            for relative_path, payload in fixture_files.items():
                destination = fixture_root / pathlib.PurePosixPath(relative_path)
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(payload)

            output_root = temporary_root / "output"
            staging_parent = temporary_root / "temp"
            staging_parent.mkdir()
            version_id = "contract-late-hash-path-collision"
            zip_path = output_root / f"Gateway-{version_id}-docker-deploy.zip"
            hash_path = pathlib.Path(str(zip_path) + ".sha256")
            ready_path = temporary_root / "export-ready"
            gate_path = temporary_root / "export-gate"
            environment = os.environ.copy()
            environment["TEMP"] = str(staging_parent)
            environment["TMP"] = str(staging_parent)
            environment["GATEWAY_TEST_EXPORT_READY_PATH"] = str(ready_path)
            environment["GATEWAY_TEST_EXPORT_GATE_PATH"] = str(gate_path)
            process = subprocess.Popen(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(fixture_exporter),
                    "-VersionId",
                    version_id,
                    "-OutputDir",
                    str(output_root),
                ],
                cwd=fixture_root,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            output_collected = False
            try:
                deadline = time.monotonic() + 30
                while not ready_path.is_file():
                    if process.poll() is not None:
                        stdout, stderr = process.communicate()
                        output_collected = True
                        self.fail(
                            "exporter exited before signaling staging readiness:\n"
                            f"stdout:\n{stdout}\nstderr:\n{stderr}"
                        )
                    if time.monotonic() >= deadline:
                        self.fail("timed out waiting for exporter staging readiness")
                    time.sleep(0.01)

                staging_root = pathlib.Path(
                    ready_path.read_text(encoding="utf-8")
                )
                self.assertTrue(
                    staging_root.is_dir(), "exporter signaled a missing staging directory"
                )
                hash_path.mkdir(parents=True)
                collision_marker = hash_path / "existing-artifact.txt"
                collision_marker.write_text("preserve me\n", encoding="utf-8")
                gate_path.touch()
                stdout, stderr = process.communicate(timeout=60)
                output_collected = True

                process_output = stdout + stderr
                self.assertNotEqual(0, process.returncode, msg=process_output)
                self.assertIn(
                    "Dockerdeploybundlepublicationpathbecameoccupiedandisimmutable",
                    "".join(process_output.split()),
                )
                self.assertFalse(
                    zip_path.exists(),
                    "ZIP was published despite the late hash-path collision",
                )
                self.assertTrue(
                    hash_path.is_dir(), "existing hash-path artifact was removed"
                )
                self.assertEqual(
                    "preserve me\n", collision_marker.read_text(encoding="utf-8")
                )
                self.assertFalse(
                    staging_root.exists(), "export staging directory was not cleaned"
                )
            finally:
                try:
                    gate_path.touch(exist_ok=True)
                finally:
                    if not output_collected:
                        try:
                            process.communicate(timeout=10)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.communicate()
                    if process.stdout is not None:
                        process.stdout.close()
                    if process.stderr is not None:
                        process.stderr.close()



if __name__ == "__main__":
    unittest.main()
