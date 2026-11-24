"""Fixed deployment pages for the packaged SAP discovery fixture."""
from urllib.parse import parse_qs

PATH = "/v2/lm/deployments"
PROTOCOLS = {"azure": ("foundation-models", "azure-openai", "gpt-4o"),
             "bedrock": ("foundation-models", "aws-bedrock", "anthropic.claude-fixture"),
             "orchestration": ("orchestration", None, None)}


def entry(host, protocol, suffix, **changes):
    scenario, executable, model = PROTOCOLS[protocol]
    name = f"catalog-{protocol}-{suffix}"
    row = dict(id=name, deploymentUrl=f"https://{host}/v2/inference/deployments/{name}",
               scenarioId=scenario, executableId=executable, status="RUNNING", configurationName="fixture-config")
    if model:
        row["details"] = {"resources": {"backendDetails": {"model": {"name": model, "version": "1"}}}}
    return dict(row, **changes)


def page(host, query):
    try:
        parsed = parse_qs(query, strict_parsing=True, max_num_fields=5)
    except ValueError:
        return None
    for protocol, (scenario, executable, _) in PROTOCOLS.items():
        for offset in ((0, 4) if protocol == "azure" else (0,)):
            expected = {"scenarioId": [scenario], "status": ["RUNNING"], "$top": ["50"], "$skip": [str(offset)]}
            if executable:
                expected["executableIds"] = [executable]
            if parsed != expected:
                continue
            first = entry(host, protocol, "a")
            if protocol != "azure":
                return protocol, offset, {"resources": [first, entry(host, protocol, "stopped", status="STOPPED")], "count": 2}
            last = entry(host, protocol, "b")
            rows = [first, entry(host, "bedrock", "unwanted"), entry(host, protocol, "stopped", status="STOPPED"),
                    entry("foreign.invalid", protocol, "foreign")] if offset == 0 else [first, last]
            return protocol, offset, {"resources": rows, "count": 6}
    return None
