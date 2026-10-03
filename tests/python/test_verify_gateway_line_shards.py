"""Run the real PowerShell selector without compiling Rust to prove complete disjoint shards."""

import json
import pathlib
import subprocess
import unittest

from powershell_test_utils import powershell_executable


ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayLineShardTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run(
            [powershell_executable(), "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
             "-File", str(ROOT / "tools/verify-gateway-line.ps1"), *args],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=45,
        )

    def listing(self, *args):
        result = self.invoke("-ListOnly", "-AsJson", *args)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        return [line["Id"] for line in (payload if isinstance(payload, list) else [payload])]

    def test_six_shards_cover_each_manifest_once_and_are_balanced(self):
        all_lines = self.listing()
        shards = [self.listing("-ShardIndex", str(index), "-ShardCount", "6") for index in range(6)]
        flat = [line for shard in shards for line in shard]
        self.assertEqual(set(flat), set(all_lines))
        self.assertEqual(len(flat), len(set(flat)))
        self.assertLessEqual(max(map(len, shards)) - min(map(len, shards)), 1)
        for index, shard in enumerate(shards):
            self.assertEqual(shard, all_lines[index::6])

    def test_default_single_shard_is_backwards_compatible(self):
        self.assertEqual(self.listing(), self.listing("-ShardIndex", "0", "-ShardCount", "1"))

    def test_all_mode_executes_only_selected_manifests_and_reports_shard(self):
        selected = self.listing("-ShardIndex", "1", "-ShardCount", "6")
        result = self.invoke("-All", "-SkipCargo", "-LibOnly", "-AsJson",
                             "-ShardIndex", "1", "-ShardCount", "6")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout[result.stdout.index("{\n"):])
        self.assertEqual([line["lineId"] for line in payload["results"]], selected)
        self.assertEqual(payload["shardIndex"], 1)
        self.assertEqual(payload["shardCount"], 6)
        self.assertTrue(payload["cargoSkipped"])

    def test_invalid_empty_or_ambiguous_shards_fail(self):
        for args in [
            ("-ListOnly", "-ShardCount", "0"),
            ("-ListOnly", "-ShardIndex", "6", "-ShardCount", "6"),
            ("-ListOnly", "-ShardIndex", "63", "-ShardCount", "64"),
            ("-LineId", "fixture", "-ShardCount", "6"),
        ]:
            with self.subTest(args=args):
                result = self.invoke(*args)
                self.assertNotEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
