"""Direct, local-only Radeon Token Factory development client (Python 3.9+)."""
import argparse
import getpass
import json
import os
import re
import sys
import warnings
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, ProxyHandler, Request, build_opener

BASE = "https://developer.amd.com.cn/radeon/api/v1"
MAX_BODY = 8 * 1024 * 1024
MAX_LINE = 1024 * 1024
MAX_PROMPT = 64 * 1024


class RadeonError(Exception):
    """A sanitized failure safe to show in the terminal."""


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def request_for(key, model=None, prompt=None, stream=False, *, usage=False, max_tokens=None):
    if not re.fullmatch(r"rc-[A-Za-z0-9_.~-]+", key):
        raise RadeonError("Expected a Radeon Token Factory API key.")
    req_headers = {"Authorization": "Bearer " + key, "Accept": "application/json"}
    if usage:
        if model is not None or prompt is not None or stream or max_tokens is not None:
            raise RadeonError("Usage lookup does not accept generation options.")
        return Request(BASE + "/usage?include_recent=false", headers=req_headers, method="GET")
    if model is None:
        return Request(BASE + "/models", headers=req_headers, method="GET")
    if not model.strip() or len(model) > 256:
        raise RadeonError("Provide a nonempty exact model ID from the catalog.")
    if not prompt or not prompt.strip() or len(prompt.encode("utf-8")) > MAX_PROMPT:
        raise RadeonError("Enter a nonempty prompt, at most 64 KiB.")
    payload = {
        "model": model, "messages": [{"role": "user", "content": prompt}], "stream": stream,
    }
    if max_tokens is not None:
        if type(max_tokens) is not int or max_tokens < 1:
            raise RadeonError("max_tokens must be a positive integer.")
        payload["max_tokens"] = max_tokens
    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req_headers["Content-Type"] = "application/json"
    if stream:
        req_headers["Accept"] = "text/event-stream"
    return Request(BASE + "/chat/completions", data=data, headers=req_headers, method="POST")


def sse_events(response):
    fields = []
    size = 0
    while True:
        line = response.readline(MAX_LINE + 1)
        if not line:
            raise RadeonError("AMD stream ended without a [DONE] event.")
        size += len(line)
        if len(line) > MAX_LINE or size > MAX_BODY:
            raise RadeonError("AMD stream exceeds the local response limit.")
        line = line.rstrip(b"\r\n")
        if not line:
            if not fields:
                continue
            event = b"\n".join(fields)
            if event == b"[DONE]":
                yield None
                return
            value = json.loads(event)
            if not isinstance(value, dict) or "error" in value:
                raise RadeonError("AMD returned an error or invalid streaming event.")
            yield value
            fields.clear()
        elif line.startswith(b"data:"):
            value = line[5:]
            fields.append(value[1:] if value.startswith(b" ") else value)


def run_request(request, output, stream=False, *, opener=None, timeout=120):
    opener = opener if opener is not None else build_opener(ProxyHandler({}), NoRedirect())
    try:
        with opener.open(request, timeout=timeout) as response:
            if stream:
                if response.headers.get_content_type() != "text/event-stream":
                    raise RadeonError("AMD did not return an SSE stream.")
                for value in sse_events(response):
                    data = "[DONE]" if value is None else json.dumps(value, ensure_ascii=True)
                    output.write(f"data: {data}\n\n")
                    output.flush()
                return
            data = response.read(MAX_BODY + 1)
            if len(data) > MAX_BODY:
                raise RadeonError("AMD response exceeds the local response limit.")
            value = json.loads(data)
            if not isinstance(value, dict) or "error" in value:
                raise RadeonError("AMD returned an unexpected response.")
            if request.full_url == BASE + "/usage?include_recent=false":
                value = usage_summary(value)
            else:
                root = "data" if request.method == "GET" else "choices"
                if not isinstance(value.get(root), list):
                    raise RadeonError("AMD returned an unexpected response.")
            output.write(json.dumps(value, indent=2, ensure_ascii=True) + "\n")
    except HTTPError as error:
        error.close()
        raise RadeonError(f"AMD request failed: HTTP {error.code}.") from None
    except (URLError, OSError, ValueError):
        raise RadeonError("AMD connection or response failed.") from None


def usage_summary(value):
    names = {
        "daily_cost_limit_usd": "daily_limit_pts",
        "daily_cost_used_usd": "daily_used_pts",
        "daily_cost_remaining_usd": "daily_remaining_pts",
    }
    points = {target: value.get(source) for source, target in names.items()}
    if any(type(number) not in (int, float) or number < 0 or number == float("inf")
           or number != number for number in points.values()):
        raise RadeonError("AMD did not report a usable quota; remaining points are unknown.")
    return {"unit": "pts", **points, "reset_at": value.get("daily_reset_at")}


def main(argv=None):
    cli = argparse.ArgumentParser(description=__doc__)
    modes = cli.add_subparsers(dest="command", required=True)
    modes.add_parser("models", help="list the official Token Factory catalog")
    modes.add_parser('usage', help='show daily Token Factory points, without request history')
    chat = modes.add_parser("chat", help="read a nonsensitive message from stdin")
    chat.add_argument("--model", required=True, help="exact catalog ID")
    chat.add_argument("--stream", action="store_true", help="output SSE rather than JSON")
    chat.add_argument('--max-tokens', type=int, help='optional requested output token limit')
    args = cli.parse_args(argv)
    try:
        key = os.environ.get("RADEON_API_KEY")
        if not key:
            if not sys.stdin.isatty():
                raise RadeonError("Set RADEON_API_KEY for this process before piping input.")
            with warnings.catch_warnings():
                warnings.simplefilter("error", getpass.GetPassWarning)
                key = getpass.getpass("Radeon Token Factory key: ")
        if args.command == "models":
            run_request(request_for(key), sys.stdout)
        elif args.command == "usage":
            run_request(request_for(key, usage=True), sys.stdout)
        else:
            if sys.stdin.isatty():
                print("Prompt: ", end="", file=sys.stderr, flush=True)
                prompt = sys.stdin.readline(MAX_PROMPT + 1).rstrip("\r\n")
            else:
                prompt = sys.stdin.read(MAX_PROMPT + 1)
            request = request_for(key, args.model, prompt, args.stream, max_tokens=args.max_tokens)
            run_request(request, sys.stdout, args.stream)
        return 0
    except getpass.GetPassWarning:
        print("Cannot hide key entry in this terminal. Use a secure interactive terminal.", file=sys.stderr)
        return 1
    except (RadeonError, EOFError, KeyboardInterrupt) as error:
        print(str(error) or "Canceled.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
