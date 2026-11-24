"""在仓库根运行显式命令，将临时数据、日志和退出码放进新的输出目录。"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from datetime import datetime, timezone


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("an explicit command is required after --")
    repo = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.mkdir()  # 不覆盖以前的回执；由操作者选择一次性输出位置。
    temporary = output / "temp"
    temporary.mkdir()
    env = os.environ.copy()
    env.update(TEMP=str(temporary), TMP=str(temporary), TMPDIR=str(temporary), PYTHONUTF8="1")
    receipt = {
        "args": command,
        "cwd": str(repo),
        "startedAt": datetime.now(timezone.utc).isoformat(),
    }
    started = time.monotonic()
    with (output / "command.log").open("wb") as log:
        try:
            result = subprocess.run(command, cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT)
            receipt["exitCode"] = result.returncode
        except OSError as error:
            receipt.update(exitCode=127, launchError=str(error))
            log.write((str(error) + "\n").encode("utf-8"))
    receipt["elapsedSeconds"] = round(time.monotonic() - started, 2)
    (output / "result.json").write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(receipt, ensure_ascii=False))
    return receipt["exitCode"]


if __name__ == "__main__":
    sys.exit(main())
