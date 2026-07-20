import unittest
from pathlib import Path


GATEWAY_ROOT = Path(__file__).resolve().parents[2]


class GatewayPhase3AccessReliabilityContractTests(unittest.TestCase):
    def test_access_key_save_is_atomic_and_cache_versioned(self):
        source = (GATEWAY_ROOT / "src" / "db" / "access.rs").read_text(encoding="utf-8")

        save_start = source.index("pub async fn save_access_key(")
        save_end = source.index("pub async fn rotate_access_key(", save_start)
        save_source = source[save_start:save_end]
        self.assertIn("let mut tx = pool.begin()", save_source)
        self.assertIn(".execute(&mut *tx)", save_source)
        self.assertIn("tx.commit()", save_source)
        self.assertIn("ACCESS_PROJECTION_CACHE_SCHEMA_VERSION", source)
        self.assertIn("projection_version", source)
        self.assertIn("resolved_tenant_id", source[source.index("struct CachedAccessProjection"):])
        self.assertIn("cache_projection_is_compatible", source)

    def test_projection_dependencies_advance_database_versions(self):
        access_source = (GATEWAY_ROOT / "src" / "db" / "access.rs").read_text(
            encoding="utf-8"
        )
        routing_source = (GATEWAY_ROOT / "src" / "db" / "routing.rs").read_text(
            encoding="utf-8"
        )

        self.assertIn("bump_all_access_projection_versions", access_source)
        for function_name in (
            "save_provider_capability",
            "save_platform_access",
            "save_access_bundle",
            "delete_access_bundle",
            "replace_access_bundle_items",
            "replace_access_key_aggregate_memberships",
        ):
            start = access_source.index(f"pub async fn {function_name}(")
            next_function = access_source.find("\npub async fn ", start + 1)
            function_source = access_source[
                start : next_function if next_function >= 0 else len(access_source)
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
        source = (GATEWAY_ROOT / "src" / "db" / "access.rs").read_text(encoding="utf-8")

        self.assertIn("ensure_project_tenant_boundary", source)
        self.assertIn("project_id = $2 or project_id is null", source)
        self.assertIn("ensure_allowed_resource_ids", source)

    def test_auto_route_memberships_are_limited_to_the_aggregate_boundary(self):
        source = (GATEWAY_ROOT / "src" / "db" / "access.rs").read_text(encoding="utf-8")

        self.assertIn(
            "member.resolved_project_id = aggregate.resolved_project_id", source
        )
        self.assertIn(
            "member.resolved_tenant_id = aggregate.resolved_tenant_id", source
        )
        self.assertIn("validate_aggregate_membership_boundaries", source)

    def test_failed_replacement_readiness_terminates_the_spawned_worker(self):
        source = (GATEWAY_ROOT / "src" / "splitter.rs").read_text(encoding="utf-8")

        self.assertIn("terminate_unready_worker", source)
        self.assertIn("replacement_readiness_failed", source)


if __name__ == "__main__":
    unittest.main()
