import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("windows_icons", ROOT / "tools/verify-gateway-windows-icons.py")
ICONS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ICONS)
ICON = ROOT / "apps/desktop/src-tauri/icons/icon.ico"


class GatewayWindowsIconContractTests(unittest.TestCase):
    def test_headless_build_watches_and_links_the_shared_icon_only_to_gateway(self):
        build = (ROOT / "build.rs").read_text(encoding="utf-8")
        resource = (ROOT / "build_support/windows.rc").read_text(encoding="utf-8")
        self.assertIn('1 ICON "apps/desktop/src-tauri/icons/icon.ico"', resource)
        self.assertIn("cargo:rerun-if-changed=apps/desktop/src-tauri/icons/icon.ico", build)
        self.assertIn("cargo:rerun-if-changed=build_support/windows.rc", build)
        self.assertRegex(
            build,
            r'compile_for\(\s*"build_support/windows\.rc",\s*\["gateway"\],\s*embed_resource::NONE,?\s*\)',
        )
        self.assertIn(".manifest_required()", build)

    def test_shared_ico_has_all_dpi_sizes_with_256_first(self):
        frames = ICONS.ico_frames(ICON.read_bytes())
        self.assertEqual(list(frames), [256, 128, 96, 64, 48, 40, 32, 24, 20, 16])

    def test_rejects_truncated_or_invalid_ico(self):
        data = ICON.read_bytes()
        for invalid in (b"", b"broken", data[:6], data[:-1]):
            with self.subTest(length=len(invalid)), self.assertRaises(ValueError):
                ICONS.ico_frames(invalid)
        invalid = bytearray(data)
        struct.pack_into("<I", invalid, 18, 0)
        with self.assertRaises(ValueError):
            ICONS.ico_frames(invalid)

    def test_verifier_requires_both_executables_to_match_every_frame(self):
        expected = ICONS.ico_frames(ICON.read_bytes())
        with patch.object(ICONS, "read_exe_frames", return_value=[expected]) as read:
            result = ICONS.verify_release(Path("release"), ICON)
        self.assertTrue(result["passed"])
        self.assertEqual([call.args[0].name for call in read.call_args_list], ["gateway.exe", "gateway-ui.exe"])
        for broken in ({}, {256: b"wrong"}):
            with self.subTest(frames=list(broken)), patch.object(
                ICONS, "read_exe_frames", side_effect=[[expected], [broken]]
            ), self.assertRaisesRegex(ValueError, "gateway-ui.exe"):
                ICONS.verify_release(Path("release"), ICON)

    def test_verifier_uses_manifest_capabilities_for_each_edition(self):
        expected = ICONS.ico_frames(ICON.read_bytes())
        with tempfile.TemporaryDirectory() as directory:
            release = Path(directory)
            api = {"edition": "api", "capabilities": {"managementUi": False, "desktopUi": False}}
            with patch.object(ICONS, "read_exe_frames", return_value=[expected]) as read, patch.object(
                ICONS, "read_manifest", return_value=api
            ):
                result = ICONS.verify_release(release, ICON)
            self.assertEqual([call.args[0].name for call in read.call_args_list], ["gateway.exe"])
            self.assertEqual([item["executable"] for item in result["binaries"]], ["gateway.exe"])

            bad = {"edition": "web", "capabilities": {"managementUi": True, "desktopUi": True}}
            with patch.object(ICONS, "read_manifest", return_value=bad), self.assertRaisesRegex(
                ValueError, "capability mismatch"
            ):
                ICONS.verify_release(release, ICON)
            (release / "gateway-ui.exe").write_bytes(b"stale")
            with patch.object(ICONS, "read_manifest", return_value=api), self.assertRaisesRegex(
                ValueError, "forbidden gateway-ui.exe"
            ):
                ICONS.verify_release(release, ICON)


if __name__ == "__main__":
    unittest.main()
