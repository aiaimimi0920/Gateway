import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayContainerBindingTests(unittest.TestCase):
    def test_container_listener_is_independent_from_host_port_binding(self):
        for filename in ("docker-compose.yml", "docker-compose.local.yml", "docker-compose.dev.yml"):
            with self.subTest(compose=filename):
                source = (ROOT / "deploy" / filename).read_text(encoding="utf-8")
                gateway = source.split("  gateway:", 1)[1].split("\n  redis:", 1)[0]
                self.assertIn("- GATEWAY_BIND_HOST=0.0.0.0", gateway)
                self.assertIn('"${GATEWAY_BIND_HOST:-0.0.0.0}:${GATEWAY_PORT:-4200}:4200"', gateway)

    def test_isolated_ci_keeps_the_published_port_on_host_loopback(self):
        verifier = (ROOT / "tools/verify-gateway-docker-stack.ps1").read_text(encoding="utf-8")
        workflow = (ROOT / ".github/workflows/docker.yml").read_text(encoding="utf-8")
        self.assertIn('[string]$BindHost = "127.0.0.1"', verifier)
        self.assertIn('"GATEWAY_BIND_HOST=$BindHost"', verifier)
        self.assertIn("verify-gateway-docker-stack.ps1", workflow)
        self.assertNotIn("-BindHost", workflow)


if __name__ == "__main__":
    unittest.main()
