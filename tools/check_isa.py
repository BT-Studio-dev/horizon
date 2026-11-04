#!/usr/bin/env python3
"""Fail when a Legacy executable was compiled past a Core 2 Duo's x86-64.

The Legacy (Windows 7) bundle promises to run where the CPU stops at
SSE4.1: codegen for x86-64-v2 or newer ends such a run with 0xc000001d
(illegal instruction) instead of an error message. The build pins that
baseline (`-C target-cpu=generic`, MSVC `/arch:SSE2` for the C parts);
this check holds it by disassembling each PE's code sections and failing
on the instructions mis-targeted codegen emits across ordinary code:
POPCNT (the `count_ones` canary of v2+), MOVBE, CRC32, the SSE4.2
compare/string instructions, and RDRAND/RDSEED. None occur in the
CPUID-dispatched library code this build links (ring, mimalloc, the CRT),
which is what makes zero the right expectation for them.

VEX/EVEX (AVX...) encodings are counted, not banned: dispatched library
code legitimately carries AVX paths it only runs after a CPUID check,
so their share of decoded instructions is reported and only fails past
a ceiling a whole-build `-march` accident would blow through.
Instructions encodable as pre-BMI opcodes (tzcnt/bsf, lzcnt/bsr) cannot
be distinguished statically and are out of scope, as are dispatched AES /
carry-less-multiply paths. Needs the `capstone` package to disassemble;
without it this is an error, never a silent pass.
"""

import argparse
import struct
import sys
from collections import Counter
from pathlib import Path

# Instructions ordinary v2+ codegen emits that a Penryn (SSE4.1, no
# POPCNT) raises #UD on. Capstone mnemonics, lower case.
CANARY_MNEMONICS = {
    "popcnt",
    "movbe",
    "crc32",
    "pcmpgtq",
    "pcmpestri",
    "pcmpestrm",
    "pcmpistri",
    "pcmpistrm",
    "rdrand",
    "rdseed",
}

# Executable section characteristics: IMAGE_SCN_MEM_EXECUTE.
EXECUTE = 0x2000_0000


def pe_code_sections(data):
    """The PE's bitness (32 or 64) and its executable sections as
    (name, rva, bytes) entries."""
    if len(data) < 0x40 or data[:2] != b"MZ":
        raise ValueError("not a PE file")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    machine, sections_count, optional_size = struct.unpack_from("<HH12xH", data, pe + 4)
    mode = {0x8664: 64, 0x14C: 32}.get(machine)
    if mode is None:
        raise ValueError(f"unsupported PE machine {machine:#x}")
    optional = pe + 24
    sections = []
    for i in range(sections_count):
        at = optional + optional_size + 40 * i
        name = data[at:at + 8].rstrip(b"\0").decode("ascii", "replace")
        _, address, raw_size, raw = struct.unpack_from("<IIII", data, at + 8)
        characteristics = struct.unpack_from("<I", data, at + 36)[0]
        if characteristics & EXECUTE or name == ".text":
            sections.append((name, address, data[raw:raw + raw_size]))
    return mode, sections


def encoding_class(first_byte, mode=64):
    """VEX/EVEX class of an instruction's first byte. Only unambiguous in
    64-bit mode (the Legacy target), where C4/C5 can only open a VEX prefix
    and 62 is EVEX, BOUND not being encodable there; in 32-bit code those
    bytes also encode LES/LDS/BOUND, so nothing is claimed."""
    if mode != 64:
        return None
    if first_byte == 0x62:
        return "evex"
    if first_byte in (0xC4, 0xC5):
        return "vex"
    return None


def empty_census():
    return {
        "instructions": 0,
        "vex": 0,
        "evex": 0,
        "canaries": Counter(),
        "examples": {},
    }


def census_code(code, base_rva, mode, census=None):
    """Decode `code` and add it to the census. Needs capstone."""
    from capstone import CS_ARCH_X86, CS_MODE_32, CS_MODE_64, Cs

    if census is None:
        census = empty_census()
    md = Cs(CS_ARCH_X86, CS_MODE_64 if mode == 64 else CS_MODE_32)
    md.skipdata = True
    for insn in md.disasm(code, base_rva):
        if insn.mnemonic == ".byte":
            continue
        census["instructions"] += 1
        kind = encoding_class(insn.bytes[0], mode) if insn.bytes else None
        if kind is not None:
            census[kind] += 1
        if insn.mnemonic in CANARY_MNEMONICS:
            census["canaries"][insn.mnemonic] += 1
            census["examples"].setdefault(insn.mnemonic, insn.address)
    return census


def census_pe(data):
    mode, sections = pe_code_sections(data)
    census = empty_census()
    for _, rva, code in sections:
        census_code(code, rva, mode, census)
    return census


def vex_percent(census):
    return 100.0 * (census["vex"] + census["evex"]) / max(census["instructions"], 1)


def census_problems(path, census, max_vex_percent):
    """Why this census breaks the legacy CPU promise, if it does."""
    problems = []
    if census["instructions"] == 0:
        problems.append(f"{path}: no instructions decoded (no code sections?)")
    for mnemonic, count in sorted(census["canaries"].items()):
        first = census["examples"][mnemonic]
        problems.append(
            f"{path}: {count} {mnemonic} instruction(s), first at RVA {first:#x}; "
            "a Core 2 Duo (SSE4.1 max) faults on these (0xc000001d)"
        )
    share = vex_percent(census)
    if share > max_vex_percent:
        problems.append(
            f"{path}: AVX-class (VEX/EVEX) instructions are {share:.2f}% of "
            f"{census['instructions']} decoded instructions, over the "
            f"{max_vex_percent}% ceiling for dispatched library code; "
            "ordinary code was compiled past the x86-64 baseline"
        )
    return problems


def check(path, max_vex_percent=5.0):
    """(ok, census) for one executable; prints its census either way."""
    data = Path(path).read_bytes()
    census = census_pe(data)
    problems = census_problems(path, census, max_vex_percent)
    canaries = ", ".join(
        f"{name}={count}" for name, count in sorted(census["canaries"].items())
    ) or "none"
    summary = (
        f"{path}: {census['instructions']} instructions, canaries {canaries}, "
        f"VEX {census['vex']} + EVEX {census['evex']} ({vex_percent(census):.3f}%)"
    )
    print(summary)
    print(f"::notice title=ISA census::{summary}")
    for problem in problems:
        print(f"::error title=CPU baseline::{problem}")
    return not problems, census


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--max-vex-percent",
        type=float,
        default=5.0,
        help="fail when VEX/EVEX pass this share of decoded instructions",
    )
    parser.add_argument("paths", nargs="+", help="PE executables (or DLLs) to check")
    args = parser.parse_args(argv)
    try:
        import capstone  # noqa: F401
    except ImportError:
        print(
            "::error title=CPU baseline::check_isa.py needs the capstone "
            "package (python -m pip install capstone)",
            file=sys.stderr,
        )
        return 2
    ok = True
    for path in args.paths:
        try:
            passed, _ = check(path, args.max_vex_percent)
        except (OSError, ValueError, struct.error) as error:
            print(f"::error title=CPU baseline::{path}: {error}")
            passed = False
        ok = passed and ok
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
