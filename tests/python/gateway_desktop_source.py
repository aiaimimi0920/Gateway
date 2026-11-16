import json
import re
from pathlib import Path


def read_neuro_ui(desktop_root: Path, filename: str) -> str:
    """Inspect the declared shared UI owner, not a duplicated application primitive."""
    package = json.loads((desktop_root / "package.json").read_text(encoding="utf-8"))
    dependency = package["dependencies"]["@neuro/ui"]
    if not dependency.startswith("file:"):
        raise ValueError("Shared UI source contract requires its declared local package")
    return (desktop_root / dependency[5:] / "src" / filename).read_text(encoding="utf-8")


def read_native_module(module_file: Path) -> str:
    owners = sorted(
        owner
        for owner in module_file.with_suffix("").glob("*.rs")
        if owner.name != "tests.rs"
    )
    return "\n".join(
        owner.read_text(encoding="utf-8") for owner in [module_file, *owners]
    )


def read_styles(styles_file: Path) -> str:
    """Read the authored flat CSS import entry in cascade order."""
    def expand(match: re.Match[str]) -> str:
        owner = styles_file.parent / match.group(1)
        css = owner.read_text(encoding="utf-8")
        if "@import" in css:
            raise ValueError("Nested stylesheet import is not covered")
        return css

    css = re.sub(
        r'^@import "(\./styles/[a-z-]+\.css)";\n',
        expand,
        styles_file.read_text(encoding="utf-8"),
        flags=re.MULTILINE,
    )
    if "@import" in css:
        raise ValueError("Unsupported stylesheet import")
    return css
