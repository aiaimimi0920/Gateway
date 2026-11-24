"""Capture saved-account model contracts through existing read-only management APIs."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def failure_detail(error):
    # Only fixed local error identifiers are printable; supplier prose stays private.
    allowed_codes = {"luma_mcp_material_unavailable", "luma_mcp_management_only",
                     "luma_model_query_invalid", "console_revision_conflict",
                     "luma_mcp_catalog_failed"}
    allowed_messages = {"luma_oauth_expired", "luma_oauth_invalid_input"}
    try:
        body = json.loads(error.read(64 * 1024))
        detail = body.get("error", {})
        if not isinstance(detail, dict):
            return ""
        parts = [detail[key] for key, allowed in
                 (("code", allowed_codes), ("message", allowed_messages))
                 if isinstance(detail.get(key), str) and detail[key] in allowed]
        return " (" + ", ".join(parts) + ")" if parts else ""
    except (ValueError, AttributeError):
        return ""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--origin", default="http://127.0.0.1:4200")
    parser.add_argument("--provider", required=True)
    parser.add_argument("--credential")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    origin = urllib.parse.urlsplit(args.origin)
    if (origin.scheme != "http" or origin.hostname != "127.0.0.1"
            or origin.username or origin.password or origin.path not in ("", "/")
            or origin.query or origin.fragment or not origin.port):
        raise SystemExit("Only an explicit loopback Gateway origin is supported.")
    token = os.environ.get("GATEWAY_LUMA_MANAGEMENT_TOKEN", "")
    if not token:
        raise SystemExit("Set GATEWAY_LUMA_MANAGEMENT_TOKEN; never pass secrets in arguments.")
    args.output.mkdir(parents=True, exist_ok=False)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    headers = {"x-management-token": token, "Content-Type": "application/json", "Origin": args.origin}
    receipts = []

    def call(path, body=None):
        request = urllib.request.Request(args.origin.rstrip("/") + "/v1/internal/gateway/console/" + path,
                                         data=None if body is None else json.dumps(body).encode(), headers=headers)
        try:
            with opener.open(request, timeout=40) as response:
                raw = response.read(8 * 1024 * 1024 + 1)
                if len(raw) > 8 * 1024 * 1024:
                    raise RuntimeError("Management response exceeded the capture limit.")
                receipts.append({"path": path, "status": response.status,
                                 "noStore": "no-store" in response.headers.get("Cache-Control", "")})
                return json.loads(raw)
        except urllib.error.HTTPError as error:
            detail = failure_detail(error)
            raise RuntimeError(f"Management {path} returned HTTP {error.code}{detail}.") from None

    initial = call("route-config")["routeConfig"]
    providers = [p for p in initial["document"]["providers"] if p["id"] == args.provider]
    if len(providers) != 1:
        raise RuntimeError("Expected one saved provider.")
    candidates = [c for c in providers[0]["credentials"]
                  if c.get("luma_oauth") and (not args.credential or c.get("id") == args.credential)]
    if len(candidates) != 1:
        raise RuntimeError("Select exactly one saved MCP credential with --credential.")
    credential = candidates[0]["id"]
    revision = initial["revision"]["id"]
    headers["x-secret-grant"] = call("session/confirm-secret-access", {"token": token})["grant"]
    binding = {"providerId": args.provider, "credentialId": credential, "expectedRevision": revision}

    def models(action, **query):
        response = call("luma-mcp-models", {**binding, "action": action, **query})
        if (response["revision"] != revision or response["providerId"] != args.provider
                or response["credentialId"] != credential):
            raise RuntimeError("Saved account or revision changed during capture.")
        return response["result"]

    catalog = models("list")
    descriptions = []
    for model in catalog["models"]:
        if model["modality"] not in ("image", "video"):
            continue
        descriptions.append(models("describe", model=model["id"], type=model["modality"]))
        print(json.dumps({"model": model["id"], "status": "captured"}), flush=True)
    final = call("route-config")["routeConfig"]
    if final["revision"]["id"] != revision:
        raise RuntimeError("Revision changed during capture; results are not published.")
    documents = {"models.json": catalog, "model-descriptions.json": descriptions}
    identities = [token, headers["x-secret-grant"], credential]
    hashes = {}
    for name, document in documents.items():
        text = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
        if any(value and value in text for value in identities):
            raise RuntimeError("Account material appeared in model data; export refused.")
        raw = text.encode("utf-8")
        (args.output / name).write_bytes(raw)
        hashes[name] = hashlib.sha256(raw).hexdigest()
    receipt = {"capturedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "scope": "model list and describe only; no business session, refresh or generation",
               "status": "passed", "revisionUnchanged": True, "modelCount": len(catalog["models"]),
               "descriptionCount": len(descriptions), "requests": receipts, "sha256": hashes}
    (args.output / "verification.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        # Avoid traceback locals, URLs with query strings and untrusted response excerpts.
        raise SystemExit(f"Capture failed ({type(error).__name__}): {error}" if isinstance(error, RuntimeError)
                         else f"Capture failed ({type(error).__name__}).") from None
