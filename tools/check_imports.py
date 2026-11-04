#!/usr/bin/env python3
"""Fail when a release binary needs a library its players may not have.

Windows .exe: every DLL in the import and delay-import tables must ship with
Windows (no VCRUNTIME140.dll, vulkan-1.dll, ...): Vulkan and the like are
loaded at run time, so a missing driver is an error message, not a dialog.
`--win7`: also fail on DLLs or imported symbols added after Windows 7 SP1.
`--app-local`: imports between the DLLs named on the command line are part of
the checked bundle; every other import still has to be a Windows system DLL.
Linux ELF: the loader must be the standard one (not a /nix/store path) and the
needed libraries the ones every desktop distribution has. `--server`: a server
build, which may not need ALSA or udev either (minimal server images lack them).
"""

import argparse
import struct
import sys
from pathlib import Path

# DLLs that are part of Windows 10 and 11. api-ms-win-* and ext-ms-win-* API
# sets resolve inside Windows too. uiautomationcore.dll (UI Automation, in System32
# since Windows 7) is what the launcher's accessibility (accesskit_windows) imports.
WINDOWS_SYSTEM = {
    "advapi32.dll", "bcrypt.dll", "bcryptprimitives.dll", "cfgmgr32.dll", "combase.dll",
    "comctl32.dll", "comdlg32.dll", "crypt32.dll", "d2d1.dll", "d3d11.dll", "d3d12.dll",
    "d3dcompiler_47.dll", "dbghelp.dll", "dcomp.dll", "dwmapi.dll", "dwrite.dll", "dxgi.dll",
    "gdi32.dll", "hid.dll", "icuuc.dll", "imm32.dll", "iphlpapi.dll", "kernel32.dll",
    "kernelbase.dll", "ktmw32.dll", "mfplat.dll", "mmdevapi.dll", "msvcrt.dll", "ncrypt.dll",
    "ntdll.dll", "ole32.dll", "oleaut32.dll", "opengl32.dll", "propsys.dll", "rpcrt4.dll",
    "secur32.dll", "setupapi.dll", "shcore.dll", "shell32.dll", "shlwapi.dll", "ucrtbase.dll",
    "user32.dll", "userenv.dll", "uiautomationcore.dll", "uxtheme.dll", "version.dll",
    "winmm.dll", "ws2_32.dll", "xinput1_4.dll",
}
# DLLs added in Windows 8, 8.1, or 10 that are absent on a clean Windows 7 SP1 x64 install.
WINDOWS_POST_WIN7_DLLS = {
    "bcryptprimitives.dll", "combase.dll", "d3d12.dll", "d3dcompiler_47.dll", "dcomp.dll",
    "icuuc.dll", "shcore.dll", "ucrtbase.dll", "xinput1_4.dll",
}
# Imported symbols added after Windows 7 SP1 that must be shimmed or loaded dynamically.
WINDOWS_POST_WIN7_SYMBOLS = {
    "ActivateAudioInterfaceAsync",
    "AdjustWindowRectExForDpi",
    "CoIncrementMTAUsage",
    "CopyFile2",
    "CreateDXGIFactory2",
    "CreateDirect3D11DeviceFromDXGIDevice",
    "CreateDirect3D11SurfaceFromDXGISurface",
    "CreateDispatcherQueueController",
    "CreateFile2",
    "DCompositionCreateDevice",
    "DXGIGetDebugInterface1",
    "EnableNonClientDpiScaling",
    "GetCurrentPackageFullName",
    "GetDpiForMonitor",
    "GetDpiForSystem",
    "GetDpiForWindow",
    "GetHostNameW",
    "GetPointerType",
    "GetSystemMetricsForDpi",
    "GetSystemTimePreciseAsFileTime",
    "GetTempPath2W",
    "ProcessPrng",
    "RegisterSuspendResumeNotification",
    "RoActivateInstance",
    "RoGetActivationFactory",
    "RoGetAgileReference",
    "RoInitialize",
    "RoOriginateError",
    "RoOriginateErrorW",
    "RoUninitialize",
    "SetProcessDpiAwareness",
    "SetProcessDpiAwarenessContext",
    "UnregisterSuspendResumeNotification",
    "WaitOnAddress",
    "WakeByAddressAll",
    "WakeByAddressSingle",
    "WindowsCreateString",
    "WindowsCreateStringReference",
    "WindowsDeleteString",
    "WindowsDuplicateString",
    "WindowsGetStringLen",
    "WindowsGetStringRawBuffer",
    "WindowsIsStringEmpty",
    "u_strlen",
}
LINUX_LOADER = "/lib64/ld-linux-x86-64.so.2"
LINUX_SYSTEM = {
    "libc.so.6", "libm.so.6", "libgcc_s.so.1", "libdl.so.2", "libpthread.so.0", "librt.so.1",
    "ld-linux-x86-64.so.2", "libasound.so.2", "libudev.so.1",
}
# The game's sound (cpal) and controllers (gilrs): a desktop has them, a server image may not.
LINUX_DESKTOP_ONLY = {"libasound.so.2", "libudev.so.1"}


def windows_allowed(name):
    name = name.lower()
    return name in WINDOWS_SYSTEM or name.startswith(("api-ms-win-", "ext-ms-win-"))


def windows7_allowed(name):
    name = name.lower()
    if name in WINDOWS_POST_WIN7_DLLS:
        return False
    if name in WINDOWS_SYSTEM:
        return True
    # Windows 7 ships only the v1.0 core/security API-set forwarders (l1-1-0.dll), never WinRT or l1-2-0.
    if name.startswith(("api-ms-win-core-winrt-", "api-ms-win-crt-", "api-ms-win-shcore-", "ext-ms-win-")):
        return False
    return name.startswith(("api-ms-win-core-", "api-ms-win-security-")) and name.endswith("-l1-1-0.dll")


def pe_import_tables(data):
    """(dll_name, [symbol_names]) entries in a PE file's import and delay-import tables."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    sections_count, optional_size = struct.unpack_from("<H12xH", data, pe + 6)
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    is_pe32_plus = magic == 0x20B
    directories = optional + (112 if is_pe32_plus else 96)
    sections = [struct.unpack_from("<8xIIII", data, optional + optional_size + 40 * i)
                for i in range(sections_count)]

    def offset(rva):
        for size, address, raw_size, raw in sections:
            if address <= rva < address + max(size, raw_size):
                return rva - address + raw
        raise ValueError(f"RVA {rva:#x} outside every section")

    def name_at(rva):
        start = offset(rva)
        return data[start:data.index(b"\0", start)].decode("ascii")

    thunk_fmt = "<Q" if is_pe32_plus else "<I"
    thunk_size = 8 if is_pe32_plus else 4
    ordinal_flag = 1 << (63 if is_pe32_plus else 31)

    def read_thunks(thunk_rva):
        if not thunk_rva:
            return []
        symbols = []
        pos = offset(thunk_rva)
        while True:
            entry = struct.unpack_from(thunk_fmt, data, pos)[0]
            if not entry:
                break
            if not (entry & ordinal_flag):
                hint_name_rva = entry & 0x7FFF_FFFF
                symbols.append(name_at(hint_name_rva + 2))
            pos += thunk_size
        return symbols

    tables = []
    # Directory 1: imports (20-byte descriptors: INT at +0, name at +12, IAT at +16).
    rva = struct.unpack_from("<I", data, directories + 8 * 1)[0]
    if rva:
        at = offset(rva)
        while any(data[at:at + 20]):
            int_rva, _, _, name_rva, iat_rva = struct.unpack_from("<IIIII", data, at)
            tables.append((name_at(name_rva), read_thunks(int_rva or iat_rva)))
            at += 20
    # Directory 13: delay imports (32-byte descriptors: attrs at +0, name at +4, INT at +16).
    rva = struct.unpack_from("<I", data, directories + 8 * 13)[0]
    if rva:
        at = offset(rva)
        while any(data[at:at + 32]):
            attrs, name_rva = struct.unpack_from("<II", data, at)
            int_rva = struct.unpack_from("<I", data, at + 16)[0] if (attrs & 1) else 0
            tables.append((name_at(name_rva), read_thunks(int_rva)))
            at += 32
    return tables


def pe_imports(data):
    """DLL names in a PE32+ file's import and delay-import tables."""
    return [dll for dll, _ in pe_import_tables(data)]


def elf_needs(data):
    """The interpreter and DT_NEEDED libraries of a little-endian ELF64 file."""
    if data[:4] != b"\x7fELF" or data[4] != 2:
        raise ValueError("not an ELF64 file")
    phoff, phentsize, phnum = struct.unpack_from("<Q", data, 32)[0], *struct.unpack_from("<HH", data, 54)
    interpreter, dynamic, loads = None, None, []
    for i in range(phnum):
        kind, _, offset, vaddr, _, filesz = struct.unpack_from("<IIQQQQ", data, phoff + i * phentsize)
        if kind == 3:
            interpreter = data[offset:offset + filesz].rstrip(b"\0").decode()
        elif kind == 2:
            dynamic = (offset, filesz)
        elif kind == 1:
            loads.append((vaddr, offset, filesz))

    def file_offset(address):
        for vaddr, offset, size in loads:
            if vaddr <= address < vaddr + size:
                return address - vaddr + offset
        raise ValueError(f"address {address:#x} outside every segment")

    entries = []
    if dynamic:
        for at in range(dynamic[0], dynamic[0] + dynamic[1], 16):
            tag, value = struct.unpack_from("<qQ", data, at)
            if tag == 0:
                break
            entries.append((tag, value))
    strtab = next((file_offset(v) for t, v in entries if t == 5), None)

    def string(index):
        start = strtab + index
        return data[start:data.index(b"\0", start)].decode()

    needed = [string(v) for t, v in entries if t == 1]
    runpath = [string(v) for t, v in entries if t in (15, 29)]
    return interpreter, needed, runpath


def check(path, server=False, win7=False, app_local=False, local_dlls=None):
    data = Path(path).read_bytes()
    problems = []
    local = {name.lower() for name in local_dlls or ()}
    if data[:2] == b"MZ":
        tables = pe_import_tables(data)
        names = [dll for dll, _ in tables]
        for dll, symbols in tables:
            if app_local and dll.lower() in local:
                continue
            detail = f"{dll} ({', '.join(symbols)})" if symbols else dll
            if not windows_allowed(dll):
                problems.append(f"imports {detail}, which Windows does not ship")
            elif win7 and not windows7_allowed(dll):
                problems.append(f"imports {detail}, which Windows 7 SP1 does not ship")
            if win7:
                for sym in symbols:
                    if sym in WINDOWS_POST_WIN7_SYMBOLS:
                        problems.append(f"imports {dll}!{sym}, which Windows 7 SP1 does not export")
        summary = ", ".join(sorted(set(n.lower() for n in names)))
    else:
        interpreter, needed, runpath = elf_needs(data)
        if interpreter not in (None, LINUX_LOADER):
            problems.append(f"loader is {interpreter}, not {LINUX_LOADER}")
        problems += [f"needs {n}, which not every distribution has" for n in needed if n not in LINUX_SYSTEM]
        if server:
            problems += [f"needs {n}, which a server may not have" for n in needed if n in LINUX_DESKTOP_ONLY]
        problems += [f"has a run path to {p}" for p in runpath if "/nix/store" in p]
        summary = f"{interpreter}; " + ", ".join(needed)
    print(f"{path}: {summary}")
    for problem in problems:
        print(f"  {path} {problem}", file=sys.stderr)
    return not problems


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("binaries", nargs="+")
    parser.add_argument("--server", action="store_true",
                        help="a server build: not even ALSA or udev (the game's sound and controllers)")
    parser.add_argument("--win7", action="store_true",
                        help="require Windows 7 SP1 x64 compatibility (no Windows 8+/10+ DLLs or entry points)")
    parser.add_argument("--app-local", action="store_true",
                        help="allow imports between the DLLs named on the command line")
    args = parser.parse_args()
    local_dlls = {Path(path).name for path in args.binaries if Path(path).suffix.lower() == ".dll"}
    ok = all([
        check(path, args.server, args.win7, args.app_local, local_dlls)
        for path in args.binaries
    ])
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
