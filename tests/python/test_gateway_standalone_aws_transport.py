"""Protect S3 capabilities while excluding its unused legacy HTTPS connector."""

import pathlib
import tomllib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayAwsTransportTests(unittest.TestCase):
    def test_s3_retains_signing_runtime_and_modern_https_features(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        dependency = manifest["dependencies"]["aws-sdk-s3"]
        self.assertIsInstance(dependency, dict)
        self.assertIs(dependency["default-features"], False)
        self.assertEqual(
            set(dependency["features"]),
            {"sigv4a", "default-https-client", "rt-tokio"},
        )

    def test_each_s3_builder_selects_latest_behavior(self):
        builders = {}
        marker = "aws_sdk_s3::config::Builder::new()"
        for path in (ROOT / "src").rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            if marker not in source:
                continue
            builders[path.relative_to(ROOT).as_posix()] = source.count(marker)
            for builder in source.split(marker)[1:]:
                # The latest behavior selects the retained HTTPS connector.
                chain = builder.split(".build()", 1)[0]
                self.assertIn(".behavior_version_latest()", chain, str(path))
                self.assertNotIn(".behavior_version(", chain, str(path))
        self.assertEqual(
            builders,
            {
                "src/object_storage/configuration.rs": 1,
                "src/credential_pool_storage/s3.rs": 1,
                "src/object_storage/tests/s3.rs": 1,
                "src/object_storage/listing/tests.rs": 1,
            },
        )

    def test_lock_excludes_obsolete_aws_https_stack(self):
        lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
        packages = {(item["name"], item["version"]) for item in lock["package"]}
        for package in packages:
            name, version = package
            self.assertFalse(name == "h2" and version.startswith("0.3."), package)
            self.assertFalse(name == "rustls" and version.startswith("0.21."), package)
            self.assertFalse(
                name == "rustls-webpki" and version.startswith("0.101."), package
            )
        self.assertTrue(any(name == "aws-sdk-s3" for name, _ in packages))
        self.assertTrue(any(name == "rustls" for name, _ in packages))


if __name__ == "__main__":
    unittest.main()
