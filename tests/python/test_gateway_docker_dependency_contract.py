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
    def _shell_function(script: str, name: str) -> str:
        match = re.search(rf"(?ms)^{re.escape(name)}\(\) \{{\r?\n(.*?)^\}}", script)
        if match is None:
            raise AssertionError(f"missing shell function: {name}")
        return match.group(1)

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
        prepare = self._shell_function(entrypoint, "prepare_build_environment")
        watchers = self._shell_function(entrypoint, "run_watch_mode")
        main = self._shell_function(entrypoint, "main")

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
            prepare,
            scripts_audit,
            desktop_audit,
        )
        self._assert_markers_in_order(
            watchers,
            'log "starting frontend build watcher"',
            'log "starting gateway cargo watcher"',
        )
        self.assertIn("      prepare_build_environment\n      run_watch_mode", main)
        self.assertIn(
            '  prepare_build_environment\n  if [[ "${GATEWAY_DEV_WATCH}" == "1" ]]; then\n    run_watch_mode',
            main,
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

    def test_official_docker_apt_layers_use_cached_package_indexes_with_retries(self):
        dockerfile = self._read("Dockerfile")
        apt_runs = [
            instruction
            for instruction in self._docker_instructions(dockerfile)
            if instruction.startswith("RUN ") and "apt-get" in instruction
        ]

        self.assertEqual(len(apt_runs), 2)
        for instruction in apt_runs:
            with self.subTest(instruction=instruction):
                self.assertIn(
                    "--mount=type=cache,target=/var/cache/apt,sharing=locked",
                    instruction,
                )
                self.assertIn(
                    "--mount=type=cache,target=/var/lib/apt/lists,sharing=locked",
                    instruction,
                )
                self.assertIn("Acquire::Retries=3", instruction)

    def test_official_docker_node_installs_use_cached_prefer_offline_retries(self):
        dockerfile = self._read("Dockerfile")
        npm_ci_runs = [
            instruction
            for instruction in self._docker_instructions(dockerfile)
            if instruction.startswith("RUN ") and "npm ci" in instruction
        ]

        self.assertEqual(len(npm_ci_runs), 2)
        for instruction in npm_ci_runs:
            with self.subTest(instruction=instruction):
                self.assertIn("--mount=type=cache,target=/root/.npm", instruction)
                self.assertIn("--prefer-offline", instruction)
                self.assertIn("attempts=0", instruction)
                self.assertIn('until [ "$attempts" -ge 3 ]', instruction)

    def test_development_dockerfile_preinstalls_rustfmt_for_the_pinned_toolchain(self):
        dockerfile = self._read("Dockerfile.dev")

        self.assertIn("default-toolchain 1.98.0", dockerfile)
        self.assertIn("rustup component add rustfmt", dockerfile)
        self.assertIn("/root/.cargo/bin/rustfmt --version", dockerfile)

    def test_dev_entrypoint_checks_node_before_installing_dependencies(self):
        entrypoint = self._read("deploy/docker-dev-entrypoint.sh")
        prepare = self._shell_function(entrypoint, "prepare_build_environment")

        self.assertIn("assert_supported_node_version()", entrypoint)
        self.assertIn("process.versions.node", entrypoint)
        self.assertIn("22.22.0", entrypoint)
        self._assert_markers_in_order(
            prepare,
            "  assert_supported_node_version\n",
            'ensure_node_dependencies "${SCRIPTS_ROOT}"',
            'ensure_node_dependencies "${FRONTEND_ROOT}"',
            'audit_production_dependencies "${SCRIPTS_ROOT}"',
            'audit_production_dependencies "${FRONTEND_ROOT}"',
        )

    def test_dev_entrypoint_uses_faster_cargo_defaults_than_release_builds(self):
        compose = self._read("deploy/docker-compose.dev.yml")
        entrypoint = self._read("deploy/docker-dev-entrypoint.sh")
        dockerfile_dev = self._read("Dockerfile.dev")

        self.assertIn(
            "GATEWAY_DEV_CARGO_BUILD_JOBS=${GATEWAY_DEV_CARGO_BUILD_JOBS:-4}",
            compose,
        )
        self.assertIn(
            "GATEWAY_DEV_CARGO_INCREMENTAL=${GATEWAY_DEV_CARGO_INCREMENTAL:-1}",
            compose,
        )
        self.assertIn("GATEWAY_DEV_CARGO_BUILD_JOBS=4", dockerfile_dev)
        self.assertIn("GATEWAY_DEV_CARGO_INCREMENTAL=1", dockerfile_dev)
        self._assert_markers_in_order(
            entrypoint,
            ': "${GATEWAY_DEV_CARGO_BUILD_JOBS:=4}"',
            ': "${GATEWAY_DEV_CARGO_INCREMENTAL:=1}"',
            'export CARGO_BUILD_JOBS="${GATEWAY_DEV_CARGO_BUILD_JOBS}"',
            'export CARGO_INCREMENTAL="${GATEWAY_DEV_CARGO_INCREMENTAL}"',
        )
        self._assert_markers_in_order(
            self._shell_function(entrypoint, "main"),
            'log "mode=${GATEWAY_DEV_MODE} watch=${GATEWAY_DEV_WATCH} CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS} CARGO_INCREMENTAL=${CARGO_INCREMENTAL}"',
            "ensure_runtime_layout",
        )

    def test_dev_compose_routes_gemini_manual_add_to_the_host_browser_executor(self):
        compose = self._read("deploy/docker-compose.dev.yml")

        self.assertIn(
            "GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL=${GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL:-http://host.docker.internal:42341}",
            compose,
        )
        self.assertIn(
            "GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN=${GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN:-gateway-gemini-auth-dev}",
            compose,
        )
        self.assertIn('      - "host.docker.internal:host-gateway"', compose)

    def test_dev_deploy_helper_starts_the_host_browser_executor_for_gemini_manual_add(self):
        deploy_helper = self._read("tools/deploy-gateway-docker.ps1")
        start_helper = self._read("tools/start-gateway-browser-executor.ps1")

        self.assertIn("function Ensure-DevHostBrowserExecutor", deploy_helper)
        self.assertIn(
            'Set-DotEnvValueIfMissing -Path $envPath -Key "GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL" -Value "http://host.docker.internal:42341"',
            deploy_helper,
        )
        self.assertIn(
            'Ensure-DevHostBrowserExecutor -GatewayRoot $gatewayRoot -EnvPath $envPath',
            deploy_helper,
        )
        self.assertIn("local-browser-executor-service.mjs", start_helper)
        self.assertIn(
            "GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN",
            start_helper,
        )
        self.assertIn('SUNO_BROWSER_CDP_URL', start_helper)
        self.assertIn('UDIO_BROWSER_CDP_URL', start_helper)
        self.assertIn('http://127.0.0.1:9226', start_helper)
        self.assertIn('http://127.0.0.1:9225', start_helper)

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

    def test_official_docker_nonce_scope_starts_after_dependency_installs(self):
        dockerfile = self._read("Dockerfile")
        node_stages = [
            block
            for block in re.split(r"(?m)(?=^FROM )", dockerfile)
            if block.startswith("FROM node:")
        ]

        self.assertEqual(len(node_stages), 2)
        ui_builder_stage = node_stages[0]
        runtime_stage = node_stages[1]

        self._assert_markers_in_order(
            ui_builder_stage,
            "RUN --mount=type=cache,target=/root/.npm",
            'ARG GATEWAY_AUDIT_NONCE=""',
            'RUN printf \'%s\\n\' "${GATEWAY_AUDIT_NONCE}" >/dev/null',
        )
        self._assert_markers_in_order(
            runtime_stage,
            "cd /app/scripts",
            'ARG GATEWAY_AUDIT_NONCE=""',
            'RUN printf \'%s\\n\' "${GATEWAY_AUDIT_NONCE}" >/dev/null',
        )

    def test_official_docker_builder_reuses_a_cached_target_directory(self):
        dockerfile = self._read("Dockerfile")

        self.assertIn("CARGO_TARGET_DIR=/cargo-target", dockerfile)
        self.assertIn("--mount=type=cache,target=/cargo-target", dockerfile)
        self.assertIn("cp /cargo-target/release/gateway", dockerfile)
        self.assertIn("COPY --from=builder /app/gateway-release", dockerfile)

    def test_official_docker_builder_uses_lld_for_release_links(self):
        cargo = self._read("Cargo.toml")
        dockerfile = self._read("Dockerfile")

        self.assertIn("codegen-units = 1", cargo)
        self.assertIn("apt-get install -y --no-install-recommends cmake pkg-config clang lld", dockerfile)
        self.assertIn('RUSTFLAGS="-C link-arg=-fuse-ld=lld"', dockerfile)
        self.assertIn("cargo build --locked --release --bin gateway", dockerfile)

    def test_official_docker_runtime_keeps_script_dependency_layers_stable_across_rust_rebuilds(self):
        dockerfile = self._read("Dockerfile")
        runtime_stage = [
            block
            for block in re.split(r"(?m)(?=^FROM )", dockerfile)
            if block.startswith("FROM node:")
        ][-1]

        self.assertIn(
            "COPY scripts/package.json scripts/package-lock.json ./scripts/",
            runtime_stage,
        )
        self._assert_markers_in_order(
            runtime_stage,
            "COPY scripts/package.json scripts/package-lock.json ./scripts/",
            "cd /app/scripts",
            "npm run audit:prod --prefix /app/scripts",
            "COPY routes.example.yaml ./routes.yaml",
            "COPY routes.example.yaml ./routes.example.yaml",
            "COPY manifests ./manifests",
            "COPY scripts ./scripts",
            "COPY --from=builder /app/gateway-release /usr/local/bin/gateway",
        )
        self.assertNotIn("COPY routes.yaml", runtime_stage)

    def test_official_docker_builder_avoids_copying_non_runtime_compile_inputs(self):
        dockerfile = self._read("Dockerfile")
        builder_stage = [
            block
            for block in re.split(r"(?m)(?=^FROM )", dockerfile)
            if block.startswith("FROM rust:")
        ][0]

        self.assertNotIn("COPY examples ./examples", builder_stage)
        self.assertNotIn("COPY tests ./tests", builder_stage)
        self.assertNotIn("COPY manifests ./manifests", builder_stage)
        self.assertNotIn("COPY routes.yaml routes.example.yaml ./", builder_stage)

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
