import json
import pathlib
import re
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayDockerDependencyContractTests(unittest.TestCase):
    @staticmethod
    def _read(relative_path: str) -> str:
        return (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")

    @staticmethod
    def _assert_markers_in_order(text: str, *markers: str) -> None:
        positions = []
        for marker in markers:
            position = text.find(marker)
            if position < 0:
                raise AssertionError(f"missing ordered marker: {marker}")
            positions.append(position)
        if positions != sorted(positions):
            raise AssertionError(
                f"markers are out of order: {list(zip(markers, positions))}"
            )

    @staticmethod
    def _docker_instructions(dockerfile: str) -> list[str]:
        instructions: list[str] = []
        current = ""
        for raw_line in dockerfile.splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            current = f"{current} {line}".strip()
            if current.endswith("\\"):
                current = current[:-1].rstrip()
                continue
            instructions.append(current)
            current = ""
        if current:
            instructions.append(current)
        return instructions

    def test_dev_compose_isolates_worker_dependencies_in_a_named_volume(self):
        compose = self._read("deploy/docker-compose.dev.yml")

        self.assertIn(
            "gateway-dev-scripts-node-modules:/workspace/scripts/node_modules",
            compose,
        )
        self.assertRegex(
            compose,
            r"(?m)^  gateway-dev-scripts-node-modules:\s*$",
        )

    def test_dev_entrypoint_installs_lock_matched_worker_dependencies(self):
        entrypoint = self._read("deploy/docker-dev-entrypoint.sh")

        self.assertIn('SCRIPTS_ROOT="${WORKSPACE_ROOT}/scripts"', entrypoint)
        self.assertIn(
            'SCRIPTS_NODE_MODULES_STAMP="${SCRIPTS_ROOT}/node_modules/'
            '.gateway-package-lock.sha256"',
            entrypoint,
        )
        self.assertIn(
            'sha256sum "${package_root}/package-lock.json"',
            entrypoint,
        )
        self.assertIn(
            'npm ci --prefix "${package_root}" --no-audit --no-fund',
            entrypoint,
        )
        self.assertIn(
            'ensure_node_dependencies "${SCRIPTS_ROOT}" '
            '"${SCRIPTS_NODE_MODULES_STAMP}" "browser worker"',
            entrypoint,
        )

    def test_dev_entrypoint_audits_both_node_trees_before_watchers(self):
        entrypoint = self._read("deploy/docker-dev-entrypoint.sh")

        scripts_audit = (
            'audit_production_dependencies "${SCRIPTS_ROOT}" "browser worker"'
        )
        desktop_audit = (
            'audit_production_dependencies "${FRONTEND_ROOT}" "desktop UI"'
        )
        self.assertIn(
            'npm run audit:prod --prefix "${package_root}"',
            entrypoint,
        )
        self._assert_markers_in_order(
            entrypoint,
            scripts_audit,
            desktop_audit,
            'log "starting frontend build watcher"',
            'log "starting gateway cargo watcher"',
        )

    def test_worker_package_declares_the_supported_node_floor(self):
        package = json.loads(self._read("scripts/package.json"))
        package_lock = json.loads(self._read("scripts/package-lock.json"))

        self.assertEqual(package.get("engines", {}).get("node"), ">=22.22.0")
        self.assertEqual(
            package_lock.get("packages", {})
            .get("", {})
            .get("engines", {})
            .get("node"),
            ">=22.22.0",
        )

    def test_official_docker_node_stages_assert_the_supported_node_floor(self):
        dockerfile = self._read("Dockerfile")
        stages = [
            block
            for block in re.split(r"(?m)(?=^FROM )", dockerfile)
            if block.startswith("FROM node:")
        ]

        self.assertEqual(len(stages), 2)
        for stage in stages:
            with self.subTest(stage=stage.splitlines()[0]):
                self.assertTrue(stage.startswith("FROM node:22-bookworm-slim"))
                self.assertIn("process.versions.node", stage)
                self.assertIn("22.22.0", stage)

    def test_development_dockerfile_asserts_the_supported_node_floor(self):
        dockerfile = self._read("Dockerfile.dev")

        self.assertTrue(dockerfile.startswith("FROM node:22-bookworm-slim"))
        self.assertIn("process.versions.node", dockerfile)
        self.assertIn("22.22.0", dockerfile)

    def test_dev_entrypoint_checks_node_before_installing_dependencies(self):
        entrypoint = self._read("deploy/docker-dev-entrypoint.sh")

        self.assertIn("assert_supported_node_version()", entrypoint)
        self.assertIn("process.versions.node", entrypoint)
        self.assertIn("22.22.0", entrypoint)
        self._assert_markers_in_order(
            entrypoint,
            "\n  assert_supported_node_version\n",
            'ensure_node_dependencies "${SCRIPTS_ROOT}"',
            'audit_production_dependencies "${SCRIPTS_ROOT}"',
            'log "starting frontend build watcher"',
        )

    def test_official_docker_audits_read_the_cache_bust_nonce(self):
        dockerfile = self._read("Dockerfile")
        node_stages = [
            block
            for block in re.split(r"(?m)(?=^FROM )", dockerfile)
            if block.startswith("FROM node:")
        ]
        audit_runs = [
            instruction
            for instruction in self._docker_instructions(dockerfile)
            if instruction.startswith("RUN ") and "npm run audit:prod" in instruction
        ]

        self.assertEqual(len(audit_runs), 2)
        for stage in node_stages:
            self.assertIn('ARG GATEWAY_AUDIT_NONCE=""', stage)
        for instruction in audit_runs:
            with self.subTest(instruction=instruction):
                self.assertIn("${GATEWAY_AUDIT_NONCE}", instruction)

    def test_docker_workflow_supplies_a_unique_nonce_to_both_image_builds(self):
        workflow = self._read(".github/workflows/docker.yml")
        nonce_argument = (
            "GATEWAY_AUDIT_NONCE=${{ github.run_id }}-${{ github.run_attempt }}"
        )

        self.assertEqual(workflow.count("uses: docker/build-push-action@v6"), 2)
        self.assertEqual(workflow.count("build-args:"), 2)
        self.assertEqual(workflow.count(nonce_argument), 2)
        self.assertIn("npm run audit:prod --prefix scripts", workflow)
        self.assertIn("npm run audit:prod --prefix apps/desktop", workflow)

    def test_docker_verifier_builds_with_a_unique_nonce_without_host_node(self):
        verifier = self._read("tools/verify-gateway-docker-stack.ps1")

        self.assertIn('$auditNonce = [guid]::NewGuid().ToString("N")', verifier)
        self.assertIn('"--build-arg"', verifier)
        self.assertIn('"GATEWAY_AUDIT_NONCE=$auditNonce"', verifier)
        self.assertNotRegex(verifier, r"(?mi)^\s*&\s*(?:npm|node)(?:\.exe)?\b")

    def test_readmes_distinguish_fresh_audit_builds_from_raw_cached_builds(self):
        english = self._read("README.md")
        chinese = self._read("README.zh-CN.md")
        deploy = self._read("deploy/README.md")
        fresh_build_argument = '--build-arg "GATEWAY_AUDIT_NONCE=$auditNonce"'

        for relative_path, content in (
            ("README.md", english),
            ("README.zh-CN.md", chinese),
            ("deploy/README.md", deploy),
        ):
            with self.subTest(readme=relative_path):
                self.assertIn(fresh_build_argument, content)
                self.assertIn(
                    ".\\tools\\verify-gateway-docker-stack.ps1 -BuildImage",
                    content,
                )
        self.assertIn(
            "does not prove that the dependency audit was refreshed",
            english,
        )
        self.assertIn("不能证明本次重新执行了依赖审计", chinese)
        self.assertIn(
            "does not prove that the dependency audit was refreshed",
            deploy,
        )

    def test_readmes_use_serial_rust_tests_and_audit_portable_workers(self):
        for relative_path in ("README.md", "README.zh-CN.md"):
            readme = self._read(relative_path)
            portable_install = 'npm ci --prefix ".\\release\\Gateway\\$id\\scripts"'
            portable_audit = (
                'npm run audit:prod --prefix ".\\release\\Gateway\\$id\\scripts"'
            )

            with self.subTest(readme=relative_path):
                self.assertIn(
                    "cargo test --locked -- --test-threads=1",
                    readme,
                )
                self._assert_markers_in_order(
                    readme,
                    portable_install,
                    portable_audit,
                )


if __name__ == "__main__":
    unittest.main()
