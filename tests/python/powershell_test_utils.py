import os
import shutil


def powershell_executable() -> str:
    candidates = ["powershell", "pwsh"] if os.name == "nt" else ["pwsh", "powershell"]
    for candidate in candidates:
        if shutil.which(candidate):
            return candidate
    raise RuntimeError("Neither pwsh nor powershell is available on PATH")
