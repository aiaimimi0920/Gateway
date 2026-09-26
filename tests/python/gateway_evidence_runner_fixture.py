import pathlib
import subprocess

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
RUNNER = GATEWAY_ROOT / "tools" / "run-gateway-line-evidence.ps1"


class EvidenceRunnerFixture:
    def run_runner(self, *arguments, env=None):
        return subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(RUNNER),
                *map(str, arguments),
            ],
            cwd=REPO_ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
