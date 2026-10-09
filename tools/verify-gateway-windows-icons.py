"""Verify both packaged EXEs contain every byte-identical shared Windows icon frame."""

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import struct


def ico_frames(data):
    if len(data) < 6 or struct.unpack_from("<HH", data) != (0, 1):
        raise ValueError("Invalid ICO header")
    count = struct.unpack_from("<H", data, 4)[0]
    end = 6 + count * 16
    if not count or end > len(data):
        raise ValueError("Invalid ICO directory")
    frames = {}
    for index in range(count):
        at = 6 + index * 16
        size, height = data[at] or 256, data[at + 1] or 256
        length, offset = struct.unpack_from("<II", data, at + 8)
        if size != height or size in frames or not length or offset < end or offset + length > len(data):
            raise ValueError("Invalid ICO frame")
        frames[size] = data[offset:offset + length]
    return frames


def read_exe_frames(executable):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(
        wintypes.BOOL, wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ssize_t
    )
    signatures = {
        "LoadLibraryExW": ([wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD], wintypes.HMODULE),
        "FindResourceW": ([wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p], ctypes.c_void_p),
        "SizeofResource": ([wintypes.HMODULE, ctypes.c_void_p], wintypes.DWORD),
        "LoadResource": ([wintypes.HMODULE, ctypes.c_void_p], ctypes.c_void_p),
        "LockResource": ([ctypes.c_void_p], ctypes.c_void_p),
        "FreeLibrary": ([wintypes.HMODULE], wintypes.BOOL),
        "EnumResourceNamesW": ([wintypes.HMODULE, ctypes.c_void_p, callback_type, ctypes.c_ssize_t], wintypes.BOOL),
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(kernel, name)
        function.argtypes, function.restype = arguments, result

    # Load as data/image resources only: inspecting an EXE must never execute it.
    module = kernel.LoadLibraryExW(str(executable.resolve()), None, 0x22)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())

    def resource(kind, name):
        entry = kernel.FindResourceW(module, name, kind)
        if not entry:
            raise ctypes.WinError(ctypes.get_last_error())
        length = kernel.SizeofResource(module, entry)
        pointer = kernel.LockResource(kernel.LoadResource(module, entry))
        if not pointer or not length:
            raise ValueError("Empty Windows icon resource")
        return ctypes.string_at(pointer, length)

    names = []

    @callback_type
    def collect(unused_module, unused_kind, name, unused_context):
        # Copy string names while the callback pointer is valid.
        names.append(name if name <= 0xFFFF else ctypes.create_unicode_buffer(ctypes.wstring_at(name)))
        return True

    try:
        if not kernel.EnumResourceNamesW(module, 14, collect, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        groups = []
        for name in names:
            group = resource(14, name)
            count = struct.unpack_from("<H", group, 4)[0]
            if struct.unpack_from("<HH", group) != (0, 1) or not count or len(group) != 6 + count * 14:
                raise ValueError("Invalid Windows icon group")
            frames = {}
            for index in range(count):
                at = 6 + index * 14
                size, height = group[at] or 256, group[at + 1] or 256
                if size != height or size in frames:
                    raise ValueError("Invalid Windows icon dimensions")
                frames[size] = resource(3, struct.unpack_from("<H", group, at + 12)[0])
            groups.append(frames)
        return groups
    finally:
        kernel.FreeLibrary(module)


def verify_release(release, icon):
    expected = ico_frames(icon.read_bytes())
    records = []
    for name in ("gateway.exe", "gateway-ui.exe"):
        groups = read_exe_frames(release / name)
        if not groups or any(frames != expected for frames in groups):
            raise ValueError(f"{name}: embedded icon frames differ from the shared ICO")
        records.append({"executable": name, "iconGroups": len(groups), "frameSizes": sorted(expected)})
    return {"passed": True, "iconSha256": hashlib.sha256(icon.read_bytes()).hexdigest(), "binaries": records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-dir", type=Path, required=True)
    parser.add_argument("--icon", type=Path, required=True)
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    result = json.dumps(verify_release(args.release_dir, args.icon), indent=2)
    if args.json:
        args.json.write_text(result + "\n", encoding="utf-8")
    print(result)


if __name__ == "__main__":
    main()
