"""HTTP 客户端切换必须同时保持根锁图、工具链和构建入口一致。"""
import pathlib
import re
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


def client_generation(manifest):
    http = manifest["dependencies"]["rquest"]
    return http.get("package", "rquest")


class GatewayHttpClientCutoverContractTests(unittest.TestCase):
    def test_http_aliases_preserve_explicit_features_and_reviewed_versions(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        http = manifest["dependencies"]["rquest"]
        profiles = manifest["dependencies"]["rquest-util"]
        generation = client_generation(manifest)
        self.assertIn(generation, {"rquest", "wreq"})
        if generation == "rquest":
            self.assertEqual(http, {"version": "5", "features": ["stream", "json"]})
            self.assertEqual(profiles, {"version": "2", "features": ["emulation"]})
            return
        self.assertEqual((http["package"], http["version"]), ("wreq", "=0.16.1"))
        self.assertEqual((profiles["package"], profiles["version"]), ("wreq-util", "=0.2.0"))
        self.assertFalse(http["default-features"])
        self.assertFalse(profiles["default-features"])
        self.assertEqual(set(http["features"]), {
            "webpki-roots", "tokio-rt", "stream", "json", "form", "query", "charset",
            "system-proxy", "gzip", "brotli", "deflate", "zstd",
        })
        self.assertEqual(profiles["features"], ["emulation"])

    def test_root_lock_removes_old_client_and_vulnerable_parent_chains(self):
        lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
        packages = {package["name"] for package in lock["package"]}
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        if client_generation(manifest) == "rquest":
            self.assertTrue({"rquest", "rquest-util", "boring2", "boring-sys2"} <= packages)
            self.assertFalse({"wreq", "wreq-util", "btls", "wreq-rt"} & packages)
            return
        self.assertTrue({"wreq", "wreq-util", "btls", "wreq-rt"} <= packages)
        self.assertFalse({"rquest", "rquest-util", "boring2", "boring-sys2", "tokio-boring2", "rsa"} & packages)
        for package in lock["package"]:
            if package["name"] == "lru":
                self.assertFalse(package["version"].startswith(("0.12.", "0.13.")))

    def test_root_toolchain_ci_docker_and_readme_match_client_msrv(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
        version = toolchain["toolchain"]["channel"]
        expected = "1.98.0" if client_generation(manifest) == "wreq" else "1.95.0"
        self.assertEqual(version, expected)
        self.assertEqual(manifest["package"]["rust-version"], version)
        self.assertEqual(toolchain["toolchain"]["components"], ["rustfmt"])
        for file in ["ci.yml", "build-windows.yml", "release-tag.yml"]:
            text = (ROOT / ".github/workflows" / file).read_text(encoding="utf-8")
            expected_refs = {version}
            if file == "ci.yml":
                formatter = "4dfd137a5aecc12897f37a01818006f58cf58347"
                expected_refs.add(formatter)
                self.assertIn(f"@{formatter} # Rust {version}", text)
            self.assertEqual(set(re.findall(r"dtolnay/rust-toolchain@([^\s]+)", text)), expected_refs)
        self.assertIn(f"FROM rust:{version}-bookworm AS builder", (ROOT / "Dockerfile").read_text(encoding="utf-8"))
        development = (ROOT / "Dockerfile.dev").read_text(encoding="utf-8")
        self.assertIn(f"--default-toolchain {version}", development)
        self.assertIn(f"--toolchain {version}-x86_64-unknown-linux-gnu", development)
        self.assertIn(f"Rust `{version}`", (ROOT / "README.md").read_text(encoding="utf-8"))

    def test_detached_lock_owners_do_not_gain_gateway_or_wreq(self):
        for file in ["apps/desktop/src-tauri/Cargo.lock", "crates/gateway-local-data/Cargo.lock"]:
            lock = tomllib.loads((ROOT / file).read_text(encoding="utf-8"))
            packages = {package["name"] for package in lock["package"]}
            self.assertFalse({"gateway", "wreq", "rquest"} & packages)
        self.assertFalse((ROOT / "crates/gateway-sqlx/Cargo.lock").exists())

    def test_wreq_default_builders_route_through_gateway_proxy_policy(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        if client_generation(manifest) == "rquest":
            # 切换前仍验收原生旧客户端，不将候选 factory 冒充生产接线。
            self.assertNotIn('package = "wreq"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"))
            return
        self.assertEqual(manifest["target"]["cfg(windows)"]["dependencies"]["windows-registry"], "0.6")
        self.assertIn("pub mod http_client;", (ROOT / "src/lib.rs").read_text(encoding="utf-8"))
        owners = {
            "console/gemini_auth_session_workers.rs": 1,
            "credential_pool_automation/driver.rs": 1,
            "http/routes/internal_provider_accounts/accio_catalog.rs": 1,
            "http/routes/internal_provider_accounts/model_discovery.rs": 1,
            "provider_quota/accio_http.rs": 1,
            "provider_quota/codex.rs": 1,
            "provider_quota/generic_balance.rs": 1,
            "provider_runtime/http_probe.rs": 1,
            "splitter/service.rs": 2,
            "upstream/client_initialization.rs": 2,
            "keepalive/chatgpt_web/mod.rs": 1,
            "keepalive/qwen_web/signin.rs": 1,
        }
        for file, count in owners.items():
            text = (ROOT / "src" / file).read_text(encoding="utf-8")
            self.assertNotRegex(text, r"Client::(?:builder|new)\(")
            self.assertEqual(len(re.findall(r"crate::http_client::(?:builder|client)\(", text)), count)


if __name__ == "__main__":
    unittest.main()
