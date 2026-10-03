"""Feature coverage decisions must never silently exclude an unknown code owner."""

import importlib.util
import json
import os
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock

from gateway_repository_text_fixture import GatewayRepositoryTextFixture


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("line_plan", ROOT / "tools/plan-gateway-line-matrix.py")
PLANNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PLANNER)
BASE = "a" * 40
HEAD = "b" * 40


class GatewayCiPlanTests(GatewayRepositoryTextFixture, unittest.TestCase):
    def test_only_known_ui_and_documentation_skip_feature_builds(self):
        self.assertFalse(PLANNER.needs_matrix([
            "README.md", "docs/ci-validation.md", "apps/desktop/src/console.tsx",
            "apps/desktop/src/styles.css", "apps/desktop/e2e/pool.spec.ts",
        ]))
        for path in [
            "src/lib.rs", "tests/runtime.rs", "crates/new/Cargo.toml", "Cargo.lock",
            ".cargo/config.toml", "build_support/ui.rs", "manifests/lines/new.json",
            "tools/verify-gateway-line.ps1", ".github/workflows/ci.yml",
            "apps/desktop/src-tauri/src/lib.rs", "apps/desktop/src/embedded.rs",
            "apps/desktop/package-lock.json", "new-owner/unknown.txt",
        ]:
            with self.subTest(path=path):
                self.assertTrue(PLANNER.needs_matrix(["README.md", path]))
        self.assertTrue(PLANNER.needs_matrix([]))

    def test_pr_compares_event_base_with_checked_out_merge_revision(self):
        event = {"pull_request": {"base": {"sha": BASE}}}
        with mock.patch.object(PLANNER, "changed_paths", return_value=["README.md"]) as diff:
            self.assertFalse(PLANNER.plan("pull_request", event, HEAD)[0])
        diff.assert_called_once_with(BASE, HEAD)

    def test_push_uses_before_and_full_events_do_not_need_git(self):
        with mock.patch.object(PLANNER, "changed_paths", return_value=["src/lib.rs"]) as diff:
            self.assertTrue(PLANNER.plan("push", {"before": BASE}, HEAD)[0])
            diff.assert_called_once_with(BASE, HEAD)
            diff.reset_mock()
            for event in ("schedule", "workflow_dispatch", "unknown", None):
                self.assertTrue(PLANNER.plan(event, {}, HEAD)[0])
            diff.assert_not_called()

    def test_missing_base_git_failure_and_empty_diff_run_full_coverage(self):
        self.assertTrue(PLANNER.plan("pull_request", {}, HEAD)[0])
        for error in (OSError(), ValueError(), subprocess.TimeoutExpired("git", 30)):
            with mock.patch.object(PLANNER, "changed_paths", side_effect=error):
                self.assertTrue(PLANNER.plan("push", {"before": BASE}, HEAD)[0])
        with mock.patch.object(PLANNER, "changed_paths", return_value=[]):
            self.assertTrue(PLANNER.plan("push", {"before": BASE}, HEAD)[0])

    def test_untrusted_revision_is_rejected_without_spawning_git(self):
        with mock.patch.object(PLANNER.subprocess, "run") as run:
            for base in (None, "--output=anything", "0" * 40, "a\n" * 20):
                with self.assertRaises(ValueError):
                    PLANNER.changed_paths(base, HEAD)
            run.assert_not_called()

    def test_rename_out_of_rust_owner_cannot_hide_deleted_path(self):
        with tempfile.TemporaryDirectory(prefix="ci-plan-") as directory:
            root = pathlib.Path(directory)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root).decode().strip()

            git("init", "-q")
            git("config", "user.name", "CI fixture")
            git("config", "user.email", "fixture@example.invalid")
            (root / "src").mkdir()
            (root / "src/old.rs").write_text("fixture\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "before")
            base = git("rev-parse", "HEAD")
            (root / "docs").mkdir()
            (root / "src/old.rs").rename(root / "docs/new.md")
            git("add", "-A")
            git("commit", "-qm", "rename")
            paths = PLANNER.changed_paths(base, git("rev-parse", "HEAD"), root)
            self.assertEqual(set(paths), {"src/old.rs", "docs/new.md"})
            self.assertTrue(PLANNER.needs_matrix(paths))

    def test_outputs_are_fixed_booleans_and_summary_not_changed_file_content(self):
        with tempfile.TemporaryDirectory(prefix="ci-output-") as directory:
            root = pathlib.Path(directory)
            event = root / "event.json"
            event.write_text(json.dumps({}), encoding="utf-8")
            with mock.patch.dict(os.environ, {
                "GITHUB_EVENT_PATH": str(event), "GITHUB_EVENT_NAME": "schedule",
                "GITHUB_OUTPUT": str(root / "output"), "GITHUB_STEP_SUMMARY": str(root / "summary"),
            }):
                PLANNER.main()
            self.assertEqual((root / "output").read_text(), "run_matrix=true\n")
            self.assertIn("full feature coverage", (root / "summary").read_text())

    def test_product_gate_requires_both_product_and_conditional_feature_results(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        gate = self._workflow_job_block(workflow, "line-coverage")
        self.assertIn("name: Windows product validation", gate)
        self.assertIn("needs: [line-plan, line-matrix, windows]", gate)
        self.assertIn("if: always()", gate)
        for condition in ('test "$PLAN_RESULT" = success', 'test "$PRODUCT_RESULT" = success',
                          'test "$MATRIX_RESULT" = success', 'test "$RUN_MATRIX" = false',
                          'test "$MATRIX_RESULT" = skipped'):
            self.assertIn(condition, gate)
        matrix = self._workflow_job_block(workflow, "line-matrix")
        self.assertIn("if: needs.line-plan.outputs.run_matrix == 'true'", matrix)
        self.assertIn("shard: [0, 1, 2, 3, 4, 5]", matrix)
        self.assertIn("-ShardCount 6", matrix)
        self.assertIn("max-parallel: 6", matrix)
        self.assertIn("fail-fast: false", matrix)
        self.assertNotIn("continue-on-error", matrix)
        self.assertIn("workspaces: . -> target/line-matrix", matrix)
        self.assertIn('Join-Path $env:GITHUB_WORKSPACE "target/line-matrix"', matrix)
        self.assertIn("cache-workspace-crates: true", matrix)
        self.assertIn("save-if: ${{ github.ref == 'refs/heads/main' }}", matrix)
        self.assertIn("schedule:", workflow)
        self.assertIn("cancel-in-progress: true", workflow)


if __name__ == "__main__":
    unittest.main()
