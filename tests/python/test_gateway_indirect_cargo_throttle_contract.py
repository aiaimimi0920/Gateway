import pathlib
import re
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


def _array_tokens(body: str) -> list[str]:
    tokens: list[str] = []
    token_pattern = re.compile(
        r'"(?P<double>(?:`.|[^"])*)"'
        r"|'(?P<single>(?:''|[^'])*)'"
        r"|(?:\[[^\]]+\]\s*)?(?P<variable>\$[A-Za-z_][A-Za-z0-9_]*)"
    )
    for match in token_pattern.finditer(body):
        value = match.group("double")
        if value is None:
            value = match.group("single")
        if value is None:
            value = match.group("variable")
        tokens.append(value)
    return tokens


def _function_section(script: str, name: str, end_marker: str) -> str:
    start = script.index(f"function {name}")
    end = script.index(end_marker, start)
    return script[start:end]


def _cargo_test_builders(section: str) -> list[tuple[str, list[str], list[str]]]:
    builders: list[tuple[str, list[str], list[str]]] = []
    assignment_pattern = re.compile(
        r"\$(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*@\((?P<body>.*?)\)",
        re.DOTALL,
    )
    for assignment in assignment_pattern.finditer(section):
        base_tokens = _array_tokens(assignment.group("body"))
        if base_tokens[:2] != ["cargo", "test"]:
            continue

        variable = assignment.group("name")
        append_pattern = re.compile(
            rf"(?m)^\s*\${re.escape(variable)}\s*\+=\s*(?P<value>.+?)\s*$"
        )
        append_tokens = []
        for append in append_pattern.finditer(section, assignment.end()):
            value_tokens = _array_tokens(append.group("value"))
            if len(value_tokens) != 1:
                raise AssertionError(
                    f"could not statically resolve ${variable} append: "
                    f"{append.group('value')}"
                )
            append_tokens.append(value_tokens[0])
        builders.append((variable, base_tokens, append_tokens))
    return builders


class GatewayIndirectCargoThrottleContractTests(unittest.TestCase):
    def test_line_verifier_locks_every_cargo_test_invocation(self):
        script = (GATEWAY_ROOT / "tools/verify-gateway-line.ps1").read_text(
            encoding="utf-8"
        )
        verification = _function_section(
            script,
            "Invoke-GatewayLineVerification",
            "\n$lines = @(Get-GatewayLineManifests)",
        )
        builders = _cargo_test_builders(verification)

        self.assertTrue(builders, "expected at least one cargo test argument builder")
        for variable, base_tokens, _ in builders:
            with self.subTest(variable=variable):
                self.assertEqual(
                    base_tokens,
                    ["cargo", "test", "--manifest-path", "$CargoToml", "--locked"],
                )
                self.assertRegex(
                    verification,
                    rf"(?m)^\s*Invoke-CheckedCommand\s+\${re.escape(variable)}\s*$",
                )

    def test_line_verifier_serializes_harness_after_features_and_filter(self):
        script = (GATEWAY_ROOT / "tools/verify-gateway-line.ps1").read_text(
            encoding="utf-8"
        )
        verification = _function_section(
            script,
            "Invoke-GatewayLineVerification",
            "\n$lines = @(Get-GatewayLineManifests)",
        )
        builders = _cargo_test_builders(verification)

        self.assertTrue(builders, "expected at least one cargo test argument builder")
        for variable, _, append_tokens in builders:
            with self.subTest(variable=variable):
                no_default_index = append_tokens.index("--no-default-features")
                features_index = append_tokens.index("--features")
                feature_text_index = append_tokens.index("$featureText")
                filter_index = append_tokens.index("$filter")
                self.assertLess(no_default_index, features_index)
                self.assertLess(features_index, feature_text_index)
                self.assertLess(feature_text_index, filter_index)
                self.assertEqual(
                    append_tokens[filter_index + 1 :],
                    ["--", "--nocapture", "--test-threads=1"],
                )

    def test_release_candidate_serializes_all_target_rust_tests(self):
        script = (
            GATEWAY_ROOT / "tools/verify-gateway-release-candidate.ps1"
        ).read_text(encoding="utf-8")
        start = script.index('Invoke-OptionalCommand -Name "rust-all-targets"')
        end = script.index('Invoke-OptionalCommand -Name "release-build"', start)
        rust_all_targets = script[start:end]
        command_match = re.search(
            r"-Command\s+@\((?P<body>.*?)\)\s*$",
            rust_all_targets,
            re.DOTALL,
        )

        self.assertIsNotNone(command_match, "missing rust-all-targets command array")
        self.assertEqual(
            _array_tokens(command_match.group("body")),
            [
                "cargo",
                "test",
                "--manifest-path",
                "$CargoToml",
                "--locked",
                "--all-targets",
                "--",
                "--test-threads=1",
            ],
        )

    def test_release_candidate_docker_build_uses_unique_audit_nonce(self):
        script = (
            GATEWAY_ROOT / "tools/verify-gateway-release-candidate.ps1"
        ).read_text(encoding="utf-8")
        start = script.index('Invoke-OptionalCommand -Name "docker-build"')
        end = script.index("\n\n  if ($IncludeRuntimeSmoke)", start)
        docker_build = script[start:end]
        command_match = re.search(
            r"-Command\s+@\((?P<body>.*?)\)\s*$",
            docker_build,
            re.DOTALL,
        )

        self.assertRegex(
            script[:start],
            r'(?m)^\s*\$auditNonce = \[guid\]::NewGuid\(\)\.ToString\("N"\)\s*$',
        )
        self.assertIsNotNone(command_match, "missing docker-build command array")
        self.assertEqual(
            _array_tokens(command_match.group("body")),
            [
                "docker",
                "build",
                "--build-arg",
                "GATEWAY_AUDIT_NONCE=$auditNonce",
                "-f",
                "Dockerfile",
                "-t",
                "gateway:release-candidate",
                ".",
            ],
        )

    def test_release_candidate_docker_build_stays_on_release_profile(self):
        script = (
            GATEWAY_ROOT / "tools/verify-gateway-release-candidate.ps1"
        ).read_text(encoding="utf-8")

        self.assertNotIn("GATEWAY_CARGO_PROFILE=docker-verify", script)
        self.assertNotIn("GATEWAY_CARGO_PROFILE=release", script)


if __name__ == "__main__":
    unittest.main()
