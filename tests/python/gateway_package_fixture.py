import hashlib
import json
import pathlib
import subprocess
import unittest

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
PACKAGER = GATEWAY_ROOT / "tools" / "package-gateway-release.ps1"
RUNTIME_SMOKE = GATEWAY_ROOT / "tools" / "smoke-gateway-packaged-runtime.ps1"
UI_SMOKE = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"


class GatewayPackageFixture(unittest.TestCase):
    """Temporary source trees and subprocesses for package contract tests."""

    def _write_fixture(self, root: pathlib.Path) -> None:
        files = {
            "target/release/gateway.exe": b"gateway-binary-v1\n",
            "apps/desktop/src-tauri/target/release/gateway-ui.exe": b"gateway-ui-binary-v1\n",
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
            "deploy/.env.example": b"IMAGE_TAG=latest\n",
            "deploy/README.md": b"# Docker deployment\n",
            "deploy/docker-compose.local.yml": b"services: {}\n",
            "deploy/docker-compose.yml": b"services: {}\n",
            "deploy/docker-deploy.sh": b"#!/usr/bin/env bash\n",
            "deploy/docker-entrypoint.sh": b"#!/usr/bin/env sh\n",
            "deploy/postgres/initdb/001-gateway-schema.sql": b"-- Synthetic schema fixture.\n",
            "deploy/postgres/initdb/002-gateway-bootstrap.sql": b"-- Synthetic bootstrap fixture.\n",
            "deploy/postgres/initdb/003-gateway-standalone-constraints.sql": b"-- Synthetic constraints fixture.\n",
            "deploy/.env": b"GATEWAY_MANAGEMENT_TOKEN=fixture-secret\n",
            "deploy/gateway_data/routes.yaml": b"api_key: sk-fixture-local\n",
            "deploy/redis_data/appendonly.aof": b"fixture redis state\n",
            "deploy/docker-compose.dev.yml": b"services: {}\n",
            "deploy/docker-dev-entrypoint.sh": b"#!/usr/bin/env bash\n",
            "tools/run-gateway-line-evidence.ps1": b"param()\n",
            "tools/line-evidence/process.ps1": b"function Get-PrivateEvidenceProcess {}\n",
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
                            "path": "target/release/gateway.exe",
                            "bytes": (source_root / "target/release/gateway.exe").stat().st_size,
                            "sha256": hashlib.sha256(
                                (source_root / "target/release/gateway.exe").read_bytes()
                            ).hexdigest(),
                        },
                        {
                            "path": "apps/desktop/src-tauri/target/release/gateway-ui.exe",
                            "bytes": (source_root / "apps/desktop/src-tauri/target/release/gateway-ui.exe").stat().st_size,
                            "sha256": hashlib.sha256(
                                (source_root / "apps/desktop/src-tauri/target/release/gateway-ui.exe").read_bytes()
                            ).hexdigest(),
                        },
                    ],
                }
            ),
            encoding="utf-8",
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
