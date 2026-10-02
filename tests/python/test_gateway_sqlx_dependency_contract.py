import pathlib
import tomllib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
ADAPTER = "crates/gateway-sqlx"


def read(name):
    return (ROOT / name).read_text(encoding="utf-8")


def manifest(name):
    return tomllib.loads(read(name))


class GatewaySqlxDependencyContractTests(unittest.TestCase):
    """保留官方数据库行为，并阻止 MySQL/RSA 再次进入交付锁图。"""

    def test_root_uses_the_internal_dependency_boundary(self):
        dependency = manifest("Cargo.toml")["dependencies"]["sqlx"]
        self.assertEqual(dependency, {"package": "gateway-sqlx", "path": ADAPTER})
        adapter = manifest(f"{ADAPTER}/Cargo.toml")
        self.assertFalse(adapter["package"]["publish"])
        self.assertNotIn("workspace", adapter)
        self.assertFalse((ROOT / ADAPTER / "Cargo.lock").exists())

    def test_official_versions_and_runtime_capabilities_remain_explicit(self):
        dependencies = manifest(f"{ADAPTER}/Cargo.toml")["dependencies"]
        expected = {
            "sqlx-core": {"_rt-tokio", "_tls-rustls-ring-webpki", "json", "time"},
            "sqlx-postgres": {"json", "time"},
            "sqlx-sqlite": {"bundled", "json", "time"},
            "sqlx-macros": {"derive"},
        }
        self.assertEqual(set(dependencies), set(expected))
        for name, features in expected.items():
            with self.subTest(dependency=name):
                self.assertEqual(dependencies[name]["version"], "=0.8.6")
                self.assertFalse(dependencies[name]["default-features"])
                self.assertEqual(set(dependencies[name]["features"]), features)

    def test_root_lock_has_no_umbrella_mysql_or_rsa(self):
        packages = manifest("Cargo.lock")["package"]
        names = {package["name"] for package in packages}
        self.assertIn("gateway-sqlx", names)
        self.assertTrue({"sqlx-core", "sqlx-postgres", "sqlx-sqlite", "sqlx-macros"} <= names)
        self.assertTrue({"sqlx", "sqlx-mysql", "rsa"}.isdisjoint(names))
        for package in packages:
            if package["name"].startswith("sqlx-"):
                self.assertEqual(package["version"], "0.8.6")

    def test_local_storage_and_delivery_entrypoints_keep_their_owner(self):
        runtime = read("src/local_runtime.rs")
        self.assertIn(".journal_mode(SqliteJournalMode::Wal)", runtime)
        self.assertIn(".synchronous(SqliteSynchronous::Full)", runtime)
        self.assertIn("COPY crates ./crates", read("Dockerfile"))
        self.assertIn(f'"/{ADAPTER}"', read(".github/dependabot.yml"))
        self.assertIn(f"--watch {ADAPTER}", read("deploy/docker-dev-entrypoint.sh"))


if __name__ == "__main__":
    unittest.main()
