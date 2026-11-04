#!/usr/bin/env python3
"""Fail when a Legacy executable was compiled past a Core 2 Duo's x86-64.

The Legacy (Windows 7) bundle promises to run where the CPU stops at
SSE4.1: codegen for x86-64-v2 or newer ends such a run with 0xc000001d
(illegal instruction) instead of an error message. The build pins that
baseline (`-C target-cpu=generic`, MSVC `/arch:SSE2` for the C parts);
this check holds it by disassembling each PE's code sections and failing
on the instructions mis-targeted codegen emits across ordinary code:
POPCNT (the `count_ones` canary of v2+), CRC32, the SSE4.2 compare
instructions, and RDRAND/RDSEED — zero is the expectation, and the first
real census showed why: the bundled mimalloc ran `__popcnt64` unguarded,
exactly that crash on exactly that processor (the Legacy build now takes
the system allocator instead).

Two families get a budget instead of zero. MOVBE and the SSE4.2 string
instructions (pcmp*stri/strm) live in the statically linked CRT's
memcpy/memchr/strchr implementations, which MSVC ships as several
ISA-specific bodies behind a CPUID dispatch (`__isa_available`): the
SSE4.2/AVX bodies are never entered on a processor without them, and no
linker flag strips them out of a +crt-static binary. They are therefore
allowed only in small numbers and only concentrated in a handful of
functions (attributed through the PE's `.pdata` function table) — the
shape of dispatched library routines, not of ordinary code compiled for
the wrong CPU, which scatters them everywhere.

VEX/EVEX (AVX...) encodings are counted the same way: dispatched library
code legitimately carries AVX paths it only runs after a CPUID check,
so their share of decoded instructions is reported and only fails past
a ceiling a whole-build `-march` accident would blow through.
Instructions encodable as pre-BMI opcodes (tzcnt/bsf, lzcnt/bsr) cannot
be distinguished statically and are out of scope, as are dispatched AES /
carry-less-multiply paths. Needs the `capstone` package to disassemble;
without it this is an error, never a silent pass.
"""

import argparse
import bisect
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

# Never acceptable in these binaries: nothing linked into them needs
# these behind a dispatch, so any sighting is ordinary code (or an
# unguarded intrinsic, as bundled mimalloc's `__popcnt64` was) compiled
# past the Legacy floor.
ZERO_TOLERANCE = {"popcnt", "crc32", "pcmpgtq", "rdrand", "rdseed"}

# The CRT's CPU-dispatched string/memory bodies. Small, and confined to
# a few functions, or they are not the CRT's.
DISPATCHED = CANARY_MNEMONICS - ZERO_TOLERANCE
MAX_DISPATCHED_PER_MNEMONIC = 64
MAX_DISPATCHED_TOTAL = 128
MAX_DISPATCHED_FUNCTIONS = 24

# Executable section characteristics: IMAGE_SCN_MEM_EXECUTE.
EXECUTE = 0x2000_0000

# Data directory index of the exception (`.pdata`) table.
DIRECTORY_EXCEPTION = 3


def pe_sections(data):
    """The PE's bitness (32 or 64), its section table, and its data
    directories as (rva, size) pairs."""
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
    magic = struct.unpack_from("<H", data, optional)[0]
    directories = [(0, 0)] * 16
    # NumberOfRvaAndSizes then the directory array: PE32+ at 108/112,
    # PE32 at 92/96.
    count_at, array_at = {0x20B: (108, 112), 0x10B: (92, 96)}.get(magic, (None, None))
    if count_at is not None:
        count = min(struct.unpack_from("<I", data, optional + count_at)[0], 16)
        for i in range(count):
            directories[i] = struct.unpack_from("<II", data, optional + array_at + 8 * i)
    sections = []
    for i in range(sections_count):
        at = optional + optional_size + 40 * i
        name = data[at:at + 8].rstrip(b"\0").decode("ascii", "replace")
        vsize, address, raw_size, raw = struct.unpack_from("<IIII", data, at + 8)
        characteristics = struct.unpack_from("<I", data, at + 36)[0]
        sections.append({
            "name": name,
            "vsize": vsize,
            "rva": address,
            "raw_size": raw_size,
            "raw": raw,
            "characteristics": characteristics,
        })
    return mode, sections, directories


def pe_code_sections(data):
    """The PE's bitness (32 or 64) and its executable sections as
    (name, rva, bytes) entries."""
    mode, sections, _ = pe_sections(data)
    code = [
        (s["name"], s["rva"], data[s["raw"]:s["raw"] + s["raw_size"]])
        for s in sections
        if s["characteristics"] & EXECUTE or s["name"] == ".text"
    ]
    return mode, code


def _rva_to_offset(sections, rva):
    for s in sections:
        span = max(s["vsize"], s["raw_size"])
        if s["rva"] <= rva < s["rva"] + span:
            delta = rva - s["rva"]
            return s["raw"] + delta if delta < s["raw_size"] else None
    return None


def pe_function_ranges(data):
    """Sorted (begin_rva, end_rva) function ranges from the 64-bit
    `.pdata` exception table; empty when the PE carries none."""
    try:
        mode, sections, directories = pe_sections(data)
        if mode != 64:
            return []
        rva, size = directories[DIRECTORY_EXCEPTION]
        offset = _rva_to_offset(sections, rva) if rva and size >= 12 else None
        if offset is None:
            return []
        ranges = set()
        for i in range(size // 12):
            begin, end, _unwind = struct.unpack_from("<III", data, offset + 12 * i)
            if begin < end:
                ranges.add((begin, end))
        return sorted(ranges)
    except (ValueError, struct.error):
        return []


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
        "canary_sites": {},
        # Filled by attribute_canaries: how many distinct functions the
        # dispatched-family canaries sit in (unattributed sites count one
        # each, the worst case), and the groups themselves for the log.
        "dispatched_functions": None,
        "dispatched_groups": [],
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
            census["canary_sites"].setdefault(insn.mnemonic, []).append(insn.address)
    return census


def attribute_canaries(census, ranges):
    """Attribute dispatched-family canary sites to `.pdata` functions.

    A site outside every known function counts as a function of its own:
    with no function table, concentration cannot be claimed."""
    begins = [begin for begin, _ in ranges]
    groups = {}
    unattributed = 0
    for mnemonic in DISPATCHED:
        for address in census["canary_sites"].get(mnemonic, []):
            i = bisect.bisect_right(begins, address) - 1
            if i >= 0 and ranges[i][0] <= address < ranges[i][1]:
                groups.setdefault(ranges[i], Counter())[mnemonic] += 1
            else:
                unattributed += 1
    census["dispatched_groups"] = [
        (begin, end, counts) for (begin, end), counts in sorted(groups.items())
    ]
    census["dispatched_functions"] = len(groups) + unattributed
    return census


def census_pe(data):
    mode, sections = pe_code_sections(data)
    census = empty_census()
    for _, rva, code in sections:
        census_code(code, rva, mode, census)
    return attribute_canaries(census, pe_function_ranges(data))


def vex_percent(census):
    return 100.0 * (census["vex"] + census["evex"]) / max(census["instructions"], 1)


def dispatched_counts(census):
    return {
        m: c for m, c in census["canaries"].items() if m in DISPATCHED and c > 0
    }


def census_problems(path, census, max_vex_percent):
    """Why this census breaks the legacy CPU promise, if it does."""
    problems = []
    if census["instructions"] == 0:
        problems.append(f"{path}: no instructions decoded (no code sections?)")
    for mnemonic, count in sorted(census["canaries"].items()):
        if mnemonic not in ZERO_TOLERANCE or count == 0:
            continue
        first = census["examples"][mnemonic]
        problems.append(
            f"{path}: {count} {mnemonic} instruction(s), first at RVA {first:#x}; "
            "a Core 2 Duo (SSE4.1 max) faults on these (0xc000001d)"
        )
    dispatched = dispatched_counts(census)
    if dispatched:
        for mnemonic, count in sorted(dispatched.items()):
            if count > MAX_DISPATCHED_PER_MNEMONIC:
                first = census["examples"][mnemonic]
                problems.append(
                    f"{path}: {count} {mnemonic} instruction(s), first at RVA "
                    f"{first:#x}, over the {MAX_DISPATCHED_PER_MNEMONIC} a "
                    "CPU-dispatched CRT string/memory routine may carry; "
                    "ordinary code was compiled past the x86-64 baseline"
                )
        total = sum(dispatched.values())
        if total > MAX_DISPATCHED_TOTAL:
            problems.append(
                f"{path}: {total} dispatched-family canaries in all, over the "
                f"{MAX_DISPATCHED_TOTAL} the CRT's dispatched routines may carry"
            )
        functions = census.get("dispatched_functions")
        if functions is None:
            functions = total
        if functions > MAX_DISPATCHED_FUNCTIONS:
            problems.append(
                f"{path}: dispatched-family canaries spread across {functions} "
                f"functions, over the {MAX_DISPATCHED_FUNCTIONS} a handful of "
                "CPU-dispatched CRT routines occupy; ordinary code was "
                "compiled past the x86-64 baseline"
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
    if dispatched_counts(census):
        groups = "; ".join(
            f"RVA {begin:#x}..{end:#x} " +
            ",".join(f"{m}={c}" for m, c in sorted(counts.items()))
            for begin, end, counts in census["dispatched_groups"]
        ) or "no .pdata function holds them"
        summary += f", dispatched in {census['dispatched_functions']} function(s)"
        print(f"::notice title=Dispatched ISA routines::{path}: {groups}")
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
