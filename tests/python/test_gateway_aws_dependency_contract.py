import pathlib
import re
import tomllib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(name):
    return (ROOT / name).read_text(encoding="utf-8")


def version(value):
    parts = [int(part) for part in value.split(".")[:3]]
    return tuple(parts + [0] * (3 - len(parts)))


class GatewayAwsDependencyContractTests(unittest.TestCase):
    """AWS 安全升级必须同时更新受支持的工具链，不能只改锁文件。"""

    def test_pinned_compiler_supports_the_reviewed_aws_sdk(self):
        channel = tomllib.loads(read("rust-toolchain.toml"))["toolchain"]["channel"]
        minimum = tomllib.loads(read("Cargo.toml"))["package"]["rust-version"]
        self.assertGreaterEqual(version(minimum), (1, 94, 1))
        self.assertGreaterEqual(version(channel), version(minimum))

    def test_build_entrypoints_share_the_pinned_compiler(self):
        channel = tomllib.loads(read("rust-toolchain.toml"))["toolchain"]["channel"]
        for name, count in (("ci.yml", 2), ("build-windows.yml", 1), ("release-tag.yml", 1)):
            with self.subTest(workflow=name):
                refs = re.findall(r"uses: dtolnay/rust-toolchain@([^\s]+)",
                                  read(f".github/workflows/{name}"))
                self.assertEqual(refs, [channel] * count)
        self.assertIn(f"FROM rust:{channel}-bookworm AS builder", read("Dockerfile"))
        self.assertIn(f"--default-toolchain {channel}", read("Dockerfile.dev"))
        self.assertIn(f"--toolchain {channel}-x86_64-unknown-linux-gnu", read("Dockerfile.dev"))

    def test_aws_manifest_preserves_s3_capabilities_and_the_upgrade_floor(self):
        sdk = tomllib.loads(read("Cargo.toml"))["dependencies"]["aws-sdk-s3"]
        self.assertGreaterEqual(version(sdk["version"]), (1, 144, 0))
        self.assertFalse(sdk["default-features"])
        self.assertEqual(set(sdk["features"]), {"sigv4a", "default-https-client", "rt-tokio"})

    def test_aws_lock_cannot_restore_either_affected_lru_version(self):
        packages = tomllib.loads(read("Cargo.lock"))["package"]
        sdk = next(package for package in packages if package["name"] == "aws-sdk-s3")
        self.assertGreaterEqual(version(sdk["version"]), (1, 144, 0))
        refs = [dependency.split() for dependency in sdk["dependencies"]
                if dependency.split()[0] == "lru"]
        self.assertEqual(len(refs), 1)
        # Cargo 只有在同名包出现多个版本时才在依赖引用中写出版本。
        caches = [package for package in packages if package["name"] == "lru"
                  and (len(refs[0]) == 1 or package["version"] == refs[0][1])]
        self.assertEqual(len(caches), 1)
        # 两项独立公告分别修复于 0.16.3 和 0.18.2；采用较严格的修复下限。
        self.assertGreaterEqual(version(caches[0]["version"]), (0, 18, 2))


if __name__ == "__main__":
    unittest.main()
