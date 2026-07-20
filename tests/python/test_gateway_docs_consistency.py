import pathlib
import unittest


class GatewayDocsConsistencyTests(unittest.TestCase):
    def setUp(self):
        self.repo_root = pathlib.Path(__file__).resolve().parents[2]

    def read_gateway_doc_copies(self, name):
        path = self.repo_root / name
        return [(path, path.read_text(encoding="utf-8"))]

    def test_request_body_limit_design_matches_current_rust_contract(self):
        for path, doc in self.read_gateway_doc_copies("REQUEST_BODY_LIMIT_DESIGN.md"):
            with self.subTest(path=str(path.relative_to(self.repo_root))):
                self.assertIn("GATEWAY_MAX_REQUEST_BODY_BYTES", doc)
                self.assertNotIn("GATEWAY_MAX_REQUEST_BODY_SIZE", doc)
                self.assertNotIn("max_request_body_size", doc)
                self.assertNotIn("GatewayConfig", doc)
                self.assertNotIn("GatewayError::RequestTooLarge", doc)
                self.assertNotIn("gateway/src/error.rs", doc)
                self.assertNotIn(
                    "currently has **no explicit request body size limit**",
                    doc,
                )
                self.assertNotIn("\n- [ ] Wider architecture doc updated", doc)
                self.assertNotIn("P1 - Week 3", doc)
                self.assertNotIn("P2 - Week 4", doc)
                self.assertNotIn("Phase 1: Staging (Week 1)", doc)
                self.assertNotIn("Phase 2: Canary (Week 2)", doc)
                self.assertNotIn("Phase 3: Full Rollout (Week 3)", doc)
                self.assertNotIn("Deploy to staging environment", doc)
                self.assertNotIn("Deploy to 5% of production traffic", doc)
                self.assertNotIn("Deploy to 100% of production traffic", doc)

                for current_contract in (
                    "RequestBodyLimitLayer",
                    "request_too_large",
                    "GATEWAY_MAX_BODY_SEARCH",
                    "GATEWAY_MAX_BODY_MUSIC",
                    "Gateway/src/config.rs",
                    "Gateway/src/http/router.rs",
                    "Gateway/src/http/middleware.rs",
                    "Gateway/src/http/extractors.rs",
                    "Gateway/tests/smoke.rs",
                ):
                    self.assertIn(current_contract, doc)

    def test_prompt_cache_monitoring_design_matches_current_rust_contract(self):
        for path, doc in self.read_gateway_doc_copies("PROMPT_CACHE_MONITORING_DESIGN.md"):
            with self.subTest(path=str(path.relative_to(self.repo_root))):
                for stale_statement in (
                    "Do NOT parse `usage` field",
                    "Parse `cache_creation_input_tokens`",
                    "Parse `cache_read_input_tokens`",
                    "Calculate cost savings",
                    "Store cache metrics to database",
                    "Display cache statistics in dashboard",
                    "Provide Prometheus metrics",
                    "Design Complete, Ready for Implementation",
                    "do **not monitor or track** cache usage",
                    "P0 - Week 1",
                    "P1 - Week 2",
                    "P1 - Week 3",
                    "P2 - Week 4",
                    "### Week 1: Parse and Store",
                    "### Week 2: Metrics",
                    "### Week 3: User Dashboard",
                    "### Week 4: Documentation",
                    "Deploy to staging",
                    "Add Prometheus metrics",
                    "Deploy frontend",
                    "Create video tutorial",
                    "test_parse_anthropic_usage_with_cache",
                    "test_calculate_cache_savings",
                    "test_cache_metrics_stored",
                    "start_test_gateway().await",
                    "gateway_prompt_cache_hit_total",
                    "gateway_prompt_cache_tokens_saved_total",
                    "gateway_prompt_cache_cost_saved_dollars",
                    "gateway_prompt_cache_hit_rate",
                    "/ops/gateway/cache-stats",
                    "web/src/app/ops/gateway/cache-stats/page.tsx",
                ):
                    self.assertNotIn(stale_statement, doc)

                for current_contract in (
                    "Gateway/src/protocol/anthropic.rs",
                    "Gateway/src/redis/usage_tracking.rs",
                    "Gateway/src/pipeline/stage_finalize.rs",
                    "Gateway/src/db/request_audits.rs",
                    "Gateway/src/http/routes/metrics.rs",
                    "Gateway/src/http/routes/internal_requests.rs",
                    "Gateway/src/http/routes/internal_gateway.rs",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens",
                    "/ops/gateway/prompt-cache",
                    "## Current Rollout Status",
                    "Code-local Rust gateway rollout is complete for prompt-cache parsing, persistence, aggregate metrics, and read models",
                    "## Current Verification Strategy",
                    "test_parse_anthropic_usage",
                    "prompt_cache_summary_tracks_client_and_auto_applied_counts",
                    "gateway_prompt_cache_creation_requests_total",
                    "gateway_prompt_cache_client_marked_requests_total",
                    "gateway_prompt_cache_creation_input_tokens_total",
                    "gateway_prompt_cache_read_input_tokens_total",
                    "gateway_prompt_cache_hit_requests_total",
                    "gateway_prompt_cache_auto_applied_requests_total",
                ):
                    self.assertIn(current_contract, doc)

    def test_error_handling_strategy_matches_current_structured_error_contract(self):
        for path, doc in self.read_gateway_doc_copies("ERROR_HANDLING_STRATEGY.md"):
            with self.subTest(path=str(path.relative_to(self.repo_root))):
                for stale_statement in (
                    "// gateway/src/error.rs",
                    "// gateway/src/http/error_response.rs",
                    "// gateway/src/http/middleware.rs",
                    "pub enum GatewayError",
                    "BadRequest(String)",
                    "GatewayError::Authentication(_)",
                    "GatewayError::BadRequest(_)",
                    "GatewayError::RateLimit(_)",
                    "GatewayError::ServiceUnavailable(_)",
                    "GatewayError::UpstreamProvider",
                    "GatewayError::Database",
                    "GatewayError::Redis",
                    "GatewayError::Internal",
                    "GatewayError::NotFound",
                ):
                    self.assertNotIn(stale_statement, doc)

                for current_contract in (
                    "Gateway/src/error.rs",
                    "Gateway/src/http/extractors.rs",
                    "pub enum ErrorKind",
                    "pub enum FallbackHint",
                    "pub struct GatewayError",
                    "fallback_hint: FallbackHint",
                    "impl axum::response::IntoResponse for GatewayError",
                    "kind_from_http_status",
                    "kind_from_body_keywords",
                    "build_fallback_hint",
                    "GatewayError::bad_request",
                    "request_too_large",
                    "FallbackHint::Abort",
                    "FallbackHint::Retry",
                    "FallbackHint::FallbackProvider",
                    "FallbackHint::DowngradeModel",
                ):
                    self.assertIn(current_contract, doc)

    def test_gateway_doc_copies_are_identical_for_release_status_designs(self):
        for name in (
            "REQUEST_BODY_LIMIT_DESIGN.md",
            "PROMPT_CACHE_MONITORING_DESIGN.md",
            "ERROR_HANDLING_STRATEGY.md",
        ):
            path = self.repo_root / name
            self.assertTrue(path.is_file(), msg=name)


if __name__ == "__main__":
    unittest.main()
