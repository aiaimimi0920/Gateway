"""Keep PS5 publication tests independent of the parent PowerShell version."""

import os
import pathlib
import unittest
from unittest import mock

try:
    from . import gateway_release_publication_fixture as fixture
    from .powershell_test_utils import powershell_environment
except ImportError:
    import gateway_release_publication_fixture as fixture
    from powershell_test_utils import powershell_environment


class PowerShellEnvironmentTests(unittest.TestCase):
    def test_windows_powershell_rebuilds_module_path_without_mutating_input(self):
        environment = {"PSModulePath": "ps7-modules", "TEMP": "fixture-temp"}
        with mock.patch("os.name", "nt"):
            result = powershell_environment(r"C:\Windows\powershell.exe", environment)
        self.assertEqual({"TEMP": "fixture-temp"}, result)
        self.assertEqual("ps7-modules", environment["PSModulePath"])

    def test_windows_environment_keys_are_case_insensitive(self):
        with mock.patch("os.name", "nt"):
            result = powershell_environment("PowerShell.EXE", {"pSmOdUlEpAtH": "ps7"})
        self.assertEqual({}, result)

    def test_default_environment_is_copied_and_parent_is_not_changed(self):
        with mock.patch.dict(os.environ, {"PSModulePath": "parent-modules"}):
            with mock.patch("os.name", "nt"):
                result = powershell_environment("powershell")
            self.assertNotIn("PSModulePath", result)
            self.assertEqual("parent-modules", os.environ["PSModulePath"])

    def test_pwsh_and_non_windows_children_keep_their_module_path(self):
        environment = {"PSModulePath": "modules"}
        for platform, executable in (("nt", "pwsh.exe"), ("posix", "powershell")):
            with self.subTest(platform=platform, executable=executable):
                with mock.patch("os.name", platform):
                    result = powershell_environment(executable, environment)
                self.assertEqual(environment, result)
                self.assertIsNot(environment, result)

    def test_sync_and_barrier_publishers_use_the_isolated_environment(self):
        root = pathlib.Path("fixture")
        environment = {"PSModulePath": "ps7", "GATEWAY_TEST_STAGE_GATE": "gate"}
        completed = mock.Mock(returncode=0, stdout="", stderr="")
        with mock.patch.object(fixture, "POWERSHELL", "powershell.exe"):
            with mock.patch("os.name", "nt"), mock.patch.object(
                fixture.subprocess, "run", return_value=completed
            ) as run, mock.patch.object(fixture.subprocess, "Popen") as popen:
                fixture.ReleasePublicationFixture._run_script(root, [], root, environment)
                fixture.ReleasePublicationFixture._start_after_barrier(
                    root, [], root, root, root, environment
                )
        for call in (run.call_args, popen.call_args):
            self.assertEqual({"GATEWAY_TEST_STAGE_GATE": "gate"}, call.kwargs["env"])
        self.assertEqual("ps7", environment["PSModulePath"])


if __name__ == "__main__":
    unittest.main()
