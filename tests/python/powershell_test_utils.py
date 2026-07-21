import shutil


def powershell_executable() -> str:
    candidates = ["pwsh", "powershell"]
    for candidate in candidates:
        if shutil.which(candidate):
            return candidate
    raise RuntimeError("Neither pwsh nor powershell is available on PATH")
