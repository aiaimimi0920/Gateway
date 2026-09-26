import unittest
from pathlib import Path


GATEWAY_ROOT = Path(__file__).resolve().parents[2]


class GatewayPhase3AccessReliabilityContractTests(unittest.TestCase):
    def test_access_key_save_is_atomic_and_cache_versioned(self):
        save_source_file = GATEWAY_ROOT / "src" / "db" / "access" / "keys.rs"
        projection_file = GATEWAY_ROOT / "src" / "db" / "access" / "projection.rs"
        save_source = save_source_file.read_text(encoding="utf-8")
        projection_source = projection_file.read_text(encoding="utf-8")

        save_start = save_source.index("pub async fn save_access_key(")
        save_end = save_source.index("pub async fn delete_access_key(", save_start)
        save_source = save_source[save_start:save_end]
        self.assertIn("let mut tx = pool.begin()", save_source)
        self.assertIn(".execute(&mut *tx)", save_source)
        self.assertIn("tx.commit()", save_source)
        self.assertIn("ACCESS_PROJECTION_CACHE_SCHEMA_VERSION", projection_source)
        self.assertIn("projection_version", projection_source)
        self.assertIn("resolved_tenant_id", projection_source)
        self.assertIn("cache_projection_is_compatible", projection_source)

    def test_projection_dependencies_advance_database_versions(self):
        access_source = (GATEWAY_ROOT / "src" / "db" / "access.rs").read_text(
            encoding="utf-8"
        )
        routing_source = (
            GATEWAY_ROOT / "src" / "db" / "routing" / "aliases.rs"
        ).read_text(encoding="utf-8")
        owner_sources = {
            "save_provider_capability": (
                GATEWAY_ROOT / "src" / "db" / "access" / "catalog_write.rs"
            ).read_text(encoding="utf-8"),
            "save_platform_access": (
                GATEWAY_ROOT / "src" / "db" / "access" / "catalog_write.rs"
            ).read_text(encoding="utf-8"),
            "save_access_bundle": (
                GATEWAY_ROOT / "src" / "db" / "access" / "bundles.rs"
            ).read_text(encoding="utf-8"),
            "delete_access_bundle": (
                GATEWAY_ROOT / "src" / "db" / "access" / "bundles.rs"
            ).read_text(encoding="utf-8"),
            "replace_access_bundle_items": (
                GATEWAY_ROOT / "src" / "db" / "access" / "bundles.rs"
            ).read_text(encoding="utf-8"),
            "replace_access_key_aggregate_memberships": (
                GATEWAY_ROOT / "src" / "db" / "access" / "memberships.rs"
            ).read_text(encoding="utf-8"),
        }

        self.assertIn("bump_all_access_projection_versions", access_source)
        for function_name in (
            "save_provider_capability",
            "save_platform_access",
            "save_access_bundle",
            "delete_access_bundle",
            "replace_access_bundle_items",
            "replace_access_key_aggregate_memberships",
        ):
            source = owner_sources[function_name]
            start = source.index(f"pub async fn {function_name}(")
            next_function = source.find("\npub async fn ", start + 1)
            function_source = source[
                start : next_function if next_function >= 0 else len(source)
            ]
            self.assertIn(
                "bump_all_access_projection_versions",
                function_source,
                function_name,
            )

        for function_name in ("save_model_alias", "delete_model_alias"):
            start = routing_source.index(f"pub async fn {function_name}(")
            next_function = routing_source.find("\npub async fn ", start + 1)
            function_source = routing_source[
                start : next_function if next_function >= 0 else len(routing_source)
            ]
            self.assertIn(
                "bump_all_access_projection_versions",
                function_source,
                function_name,
            )

    def test_access_key_bindings_validate_project_tenant_and_bundle_scope(self):
        key_source = (GATEWAY_ROOT / "src" / "db" / "access" / "keys.rs").read_text(
            encoding="utf-8"
        )
        membership_source = (
            GATEWAY_ROOT / "src" / "db" / "access" / "memberships.rs"
        ).read_text(encoding="utf-8")
        projection_source = (
            GATEWAY_ROOT / "src" / "db" / "access" / "projection.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("ensure_project_tenant_boundary", key_source)
        self.assertIn("project_id = $1 or project_id is null", projection_source)
        self.assertIn("ensure_project_tenant_boundary", membership_source)
        self.assertIn("ensure_allowed_resource_ids", membership_source)

    def test_auto_route_memberships_are_limited_to_the_aggregate_boundary(self):
        membership_source = (
            GATEWAY_ROOT / "src" / "db" / "access" / "memberships.rs"
        ).read_text(encoding="utf-8")
        projection_source = (
            GATEWAY_ROOT / "src" / "db" / "access" / "projection_queries.rs"
        ).read_text(encoding="utf-8")

        self.assertIn(
            "member.resolved_project_id = aggregate.resolved_project_id", membership_source
        )
        self.assertIn(
            "member.resolved_tenant_id = aggregate.resolved_tenant_id", membership_source
        )
        self.assertIn("validate_aggregate_membership_boundaries", membership_source)
        self.assertIn("m.aggregate_access_key_id = $1", projection_source)
        self.assertIn("member.resolved_project_id = aggregate.resolved_project_id", projection_source)
        self.assertIn("member.resolved_tenant_id = aggregate.resolved_tenant_id", projection_source)

    def test_failed_replacement_readiness_terminates_the_spawned_worker(self):
        source = (GATEWAY_ROOT / "src" / "splitter" / "lifecycle.rs").read_text(
            encoding="utf-8"
        )

        self.assertIn("terminate_unready_worker", source)
        self.assertIn("replacement_readiness_failed", source)


if __name__ == "__main__":
    unittest.main()
