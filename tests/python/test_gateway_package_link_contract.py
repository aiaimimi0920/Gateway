import os
import pathlib
import subprocess
import tempfile
import unittest

from gateway_package_fixture import GatewayPackageFixture
from powershell_test_utils import powershell_executable


@unittest.skipUnless(os.name == "nt", "Windows junction contract")
class GatewayPackageLinkContractTests(GatewayPackageFixture):
    def test_evidence_junctions_cannot_publish_into_an_outside_directory(self):
        for relative in ("target/release-evidence", "target/release-evidence/contract-v1"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary)
                source = root / "Gateway-source"
                source.mkdir()
                self._write_fixture(source)
                self._write_build_provenance(source)
                outside = root / "outside"
                outside.mkdir()
                sentinel = outside / "sentinel.txt"
                sentinel.write_bytes(b"preserve")
                link = source / relative
                link.parent.mkdir(parents=True, exist_ok=True)
                quote = lambda value: "'" + str(value).replace("'", "''") + "'"
                created = subprocess.run(
                    [powershell_executable(), "-NoProfile", "-Command",
                     f"New-Item -ItemType Junction -Path {quote(link)} -Target {quote(outside)} | Out-Null"],
                    capture_output=True, text=True, timeout=30,
                )
                self.assertEqual(created.returncode, 0, created.stdout + created.stderr)
                try:
                    result = self._run_packager(source, root / "release/Gateway", write_provenance=False)
                    self.assertNotEqual(result.returncode, 0)
                    diagnostic = "".join((result.stdout + result.stderr).lower().split())
                    self.assertIn("linkeddescendant", diagnostic)
                    self.assertEqual(sorted(item.name for item in outside.iterdir()), ["sentinel.txt"])
                    self.assertEqual(sentinel.read_bytes(), b"preserve")
                    self.assertFalse((root / "release/Gateway").exists())
                finally:
                    # Remove only this test-owned junction, never recurse into its target.
                    link.rmdir()
