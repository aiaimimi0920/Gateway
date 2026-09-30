import ntpath
import os
import shutil


def powershell_executable() -> str:
    candidates = ["pwsh", "powershell"]
    for candidate in candidates:
        if shutil.which(candidate):
            return candidate
    raise RuntimeError("Neither pwsh nor powershell is available on PATH")


def powershell_environment(
    executable: str, environment: dict[str, str] | None = None
) -> dict[str, str]:
    child_environment = dict(os.environ if environment is None else environment)
    if os.name == "nt" and ntpath.basename(executable).casefold() in {
        "powershell", "powershell.exe"
    }:
        # A pwsh -> Python -> Windows PowerShell 5.1 chain otherwise inherits
        # incompatible PS7 modules. Let PS5 rebuild its own module search path.
        for key in list(child_environment):
            if key.casefold() == "psmodulepath":
                del child_environment[key]
    return child_environment
