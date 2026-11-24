"""匿名抓取固定的 Luma 官方合同页面；不会登录或调用生成 API。"""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from html.parser import HTMLParser
from pathlib import Path
from urllib.error import URLError
from urllib.request import Request, urlopen

PAGES = {
    "files": "guides/files/",
    "files-create": "api/resources/files/methods/create/",
    "files-complete": "api/resources/files/methods/complete/",
    "layering": "guides/images/layering/",
    "llm": "guides/llm-inference/",
    "video-edit": "guides/videos/editing/",
    "video-reframe": "guides/videos/reframing/",
    "usage": "guides/usage/",
    "create": "api/resources/generations/methods/create/",
}


class Text(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hidden = 0
        self.parts = []

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.hidden += 1

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.hidden = max(0, self.hidden - 1)

    def handle_data(self, data):
        if not self.hidden and data.strip():
            self.parts.append(data.strip())


def fetch(name, output):
    url = "https://docs.agents.lumalabs.ai/" + PAGES[name]
    entry = {"name": name, "url": url, "checkedAt": datetime.now(timezone.utc).isoformat()}
    try:
        request = Request(url, headers={
            "User-Agent": "Gateway-Contract-Research/1.0 (public documentation only)"
        })
        with urlopen(request, timeout=30) as response:
            raw = response.read(2 * 1024 * 1024 + 1)
            if len(raw) > 2 * 1024 * 1024:
                raise ValueError("document too large")
            entry.update(status=response.status, finalUrl=response.url)
        text = Text()
        text.feed(raw.decode("utf-8-sig"))
    except (URLError, OSError, ValueError) as error:
        entry["error"] = str(error)
        return entry
    # HTML 保留原始 bytes/hash，正文另存 UTF-8 无 BOM；不是稳定的 API schema 夹具。
    output.joinpath(name + ".html").write_bytes(raw)
    output.joinpath(name + ".txt").write_text("\n".join(text.parts) + "\n", encoding="utf-8")
    entry["sha256"] = hashlib.sha256(raw).hexdigest()
    return entry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--page", choices=PAGES, action="append")
    args = parser.parse_args()
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.mkdir()  # 每次独立抓取，不覆盖历史资料或混合抓取时间。
    names = dict.fromkeys(args.page or PAGES)
    results = [fetch(name, output) for name in names]
    output.joinpath("sources.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return int(any("error" in entry for entry in results))


if __name__ == "__main__":
    raise SystemExit(main())
