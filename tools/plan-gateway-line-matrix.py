"""Skip expensive feature builds only for known non-feature changes; uncertainty runs all."""

import json
import os
import pathlib
import re
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_ROOT_FILES = {"README.md", "README.zh-CN.md", "LICENSE", "AGENTS.md"}
SAFE_PREFIXES = ("docs/", "apps/desktop/src/", "apps/desktop/e2e/", "apps/desktop/public/")
SAFE_SUFFIXES = {".md", ".txt", ".tsx", ".ts", ".css", ".json", ".svg", ".png", ".ico"}


def needs_matrix(paths):
    if not paths:
        return True
    for name in paths:
        # Unknown owners, Rust, build scripts, dependencies and workflow changes fail closed.
        if name in SAFE_ROOT_FILES:
            continue
        if name.startswith(SAFE_PREFIXES) and pathlib.PurePosixPath(name).suffix in SAFE_SUFFIXES:
            continue
        return True
    return False


def changed_paths(base, head, root=ROOT):
    for revision in (base, head):
        if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise ValueError("Missing or invalid comparison revision")
        if revision == "0" * 40:
            raise ValueError("No previous revision")
    # Disable rename detection so both the deleted owner and new owner influence coverage.
    result = subprocess.run(
        ["git", "diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, head, "--"],
        cwd=root, capture_output=True, check=True, timeout=30,
    )
    return [name for name in result.stdout.decode("utf-8").split("\0") if name]


def plan(event_name, event, head):
    if event_name not in {"push", "pull_request"}:
        return True, "Scheduled, manual or unknown event: full feature coverage."
    try:
        base = event["pull_request"]["base"]["sha"] if event_name == "pull_request" else event["before"]
        run = needs_matrix(changed_paths(base, head))
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError):
        return True, "Comparison unavailable: full feature coverage."
    reason = "Feature/build-sensitive changes: full coverage." if run else "Only known UI/documentation changes."
    return run, reason


def main():
    try:
        event = json.loads(pathlib.Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    except (KeyError, ValueError, OSError):
        event = {}
    run, reason = plan(os.environ.get("GITHUB_EVENT_NAME"), event, os.environ.get("GITHUB_SHA"))
    decision = f"run_matrix={str(run).lower()}\n"
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8", newline="\n") as handle:
            handle.write(decision)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8", newline="\n") as handle:
            handle.write(f"## Feature-line coverage\n\n{reason}\n\n{decision}")
    print(decision + reason)


if __name__ == "__main__":
    main()
