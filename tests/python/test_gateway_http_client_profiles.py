import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
CONSTRUCTOR = r"(?:Client::builder|crate::http_client::builder)\(\)"


class GatewayHttpClientProfilesTests(unittest.TestCase):
    """真实 ClientHello 回归必须对应生产 owner 的 profile 和超时边界。"""

    def check_profiles(self, upstream, splitter):
        def builder(source, name):
            match = re.search(rf"let {name} = {CONSTRUCTOR}(.*?)\.build\(\)",
                              source, re.S)
            self.assertIsNotNone(match, f"missing HTTP client owner: {name}")
            return match[1]

        http = builder(upstream, "http")
        plain = builder(upstream, "plain_http")
        ready = builder(splitter, "ready_client")
        proxy = builder(splitter, "proxy_client")
        for configured, expected in ((http, ["Emulation::Chrome136"]), (plain, []),
                                     (ready, ["Emulation::Chrome131"]),
                                     (proxy, ["Emulation::Chrome131"])):
            self.assertEqual(re.findall(r"\.emulation\((.*?)\)", configured), expected)
        for bounded in (http, plain, ready):
            self.assertIn(".timeout(", bounded)
        # 流式反向代理不能继承 readiness 的短整体超时。
        self.assertNotIn(".timeout(", proxy)

    def owners(self):
        return tuple((ROOT / name).read_text(encoding="utf-8") for name in (
            "src/upstream/client_initialization.rs", "src/splitter/service.rs"))

    def test_production_owners_match_the_wire_profile_contract(self):
        self.check_profiles(*self.owners())

    def test_contract_rejects_profile_and_stream_timeout_regressions(self):
        upstream, splitter = self.owners()
        mutants = (
            (upstream.replace(".emulation(Emulation::Chrome136)", ""), splitter),
            (upstream.replace("Chrome136", "Chrome131"), splitter),
            (re.sub(rf"(let plain_http = {CONSTRUCTOR})",
                    r"\1.emulation(Emulation::Chrome136)", upstream), splitter),
            (upstream, splitter.replace("Chrome131", "Chrome136")),
            (upstream, re.sub(rf"(let proxy_client = {CONSTRUCTOR})",
                             r"\1.timeout(Duration::from_secs(1))", splitter)),
        )
        for index, sources in enumerate(mutants):
            with self.subTest(mutant=index), self.assertRaises(AssertionError):
                self.check_profiles(*sources)


if __name__ == "__main__":
    unittest.main()
