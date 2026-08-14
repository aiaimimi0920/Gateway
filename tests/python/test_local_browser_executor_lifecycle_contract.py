import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SERVICE = REPO_ROOT / "scripts" / "local-browser-executor-service.mjs"


class LocalBrowserExecutorLifecycleContractTests(unittest.TestCase):
    def test_cancels_disconnected_clients_and_serializes_each_provider(self):
        source = SERVICE.read_text(encoding="utf-8")

        self.assertIn("const activeProviderExecutions = new Map()", source)
        self.assertIn("activeProviderExecutions.has(provider)", source)
        self.assertIn('code: "browser_executor_provider_busy"', source)
        self.assertIn('req.once("aborted", cancelOnDisconnect)', source)
        self.assertIn('res.once("close", cancelOnDisconnect)', source)
        self.assertIn("activeProviderExecutions.get(provider)?.token === executionToken", source)

    def test_waits_for_worker_termination_and_has_windows_tree_kill_fallback(self):
        source = SERVICE.read_text(encoding="utf-8")

        self.assertIn("await terminateChild(child)", source)
        self.assertIn('execFile("taskkill", ["/PID", String(child.pid), "/T", "/F"]', source)
        self.assertIn("await unlinkAsync(resultFilePath).catch(() => undefined)", source)
        self.assertIn("if (res.destroyed || res.writableEnded)", source)

    def test_enforces_a_worker_deadline_and_returns_a_timeout_error(self):
        source = SERVICE.read_text(encoding="utf-8")

        self.assertIn("const WORKER_DEADLINE_GRACE_MS = 10_000", source)
        self.assertIn("deadlineTimer = setTimeout", source)
        self.assertIn('code: "browser_executor_worker_timeout"', source)
        self.assertIn("if (deadlineTimer) clearTimeout(deadlineTimer)", source)


if __name__ == "__main__":
    unittest.main()
