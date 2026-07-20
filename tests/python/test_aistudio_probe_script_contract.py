import pathlib
import re
import unittest


class AIStudioProbeScriptContractTests(unittest.TestCase):
    def test_probe_script_never_calls_process_exit_before_cleanup(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "probe-aistudio-live-request.mjs"
        script_text = script_path.read_text(encoding="utf-8")

        self.assertIsNone(
            re.search(r"\bprocess\.exit\s*\(", script_text),
            "probe-aistudio-live-request.mjs must not call process.exit() directly; "
            "doing so can bypass main() finally cleanup and leave browser/process "
            "resources behind. Set process.exitCode after cleanup instead.",
        )


if __name__ == "__main__":
    unittest.main()
