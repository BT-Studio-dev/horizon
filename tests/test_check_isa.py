from pathlib import Path
import struct
import tempfile
import unittest

from tools import check_isa


def build_pe32_plus(code):
    """Build a minimal PE32+ image with one executable `.text` section."""
    pe_offset = 0x80
    optional_size = 240
    section_rva = 0x1000
    section_raw = 0x200
    raw = bytes(code)
    raw_size = max(0x200, (len(raw) + 0x1FF) & ~0x1FF)
    raw += b"\0" * (raw_size - len(raw))

    image = bytearray(section_raw + raw_size)
    image[0:2] = b"MZ"
    struct.pack_into("<I", image, 0x3C, pe_offset)
    image[pe_offset:pe_offset + 4] = b"PE\0\0"
    # COFF File Header: Machine=AMD64 (0x8664), NumberOfSections=1, SizeOfOptionalHeader=240
    struct.pack_into("<HHIIIHH", image, pe_offset + 4, 0x8664, 1, 0, 0, 0, optional_size, 0x0022)
    optional = pe_offset + 24
    struct.pack_into("<H", image, optional, 0x20B)
    section = optional + optional_size
    image[section:section + 8] = b".text\0\0\0"
    struct.pack_into("<IIII", image, section + 8, len(raw), section_rva, raw_size, section_raw)
    # IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ
    struct.pack_into("<I", image, section + 36, 0x6000_0020)
    image[section_raw:section_raw + raw_size] = raw
    return bytes(image)


POPCNT_RAX = b"\xf3\x48\x0f\xb8\xc0"
CRC32_RAX = b"\xf2\x48\x0f\x38\xf1\xc0"
MOVBE_RAX = b"\x48\x0f\x38\xf0\x00"  # movbe rax, [rax]
VMOVDQA = b"\xc5\xf9\x6f\xc1"
NOP_RET = b"\x90\xc3"


class CheckIsaTests(unittest.TestCase):
    def test_pe_code_sections_finds_text(self):
        mode, sections = check_isa.pe_code_sections(build_pe32_plus(NOP_RET))
        self.assertEqual(mode, 64)
        self.assertEqual(len(sections), 1)
        name, rva, code = sections[0]
        self.assertEqual(name, ".text")
        self.assertEqual(rva, 0x1000)
        self.assertTrue(code.startswith(NOP_RET))
        # The synthetic image carries no exception directory.
        self.assertEqual(check_isa.pe_function_ranges(build_pe32_plus(NOP_RET)), [])

    def test_encoding_classes_are_64_bit_only(self):
        self.assertEqual(check_isa.encoding_class(0xC5), "vex")
        self.assertEqual(check_isa.encoding_class(0xC4), "vex")
        self.assertEqual(check_isa.encoding_class(0x62), "evex")
        self.assertIsNone(check_isa.encoding_class(0x90))
        self.assertIsNone(check_isa.encoding_class(0xC5, mode=32))

    def test_problems_flag_canaries_and_vex_share(self):
        census = check_isa.empty_census()
        census["instructions"] = 100
        census["vex"] = 2
        census["canaries"]["popcnt"] = 3
        census["examples"]["popcnt"] = 0x1010
        problems = check_isa.census_problems("game.exe", census, 5.0)
        self.assertEqual(len(problems), 1)
        self.assertIn("3 popcnt", problems[0])
        self.assertIn("0xc000001d", problems[0])

        census = check_isa.empty_census()
        census["instructions"] = 100
        census["vex"] = 30
        problems = check_isa.census_problems("game.exe", census, 5.0)
        self.assertEqual(len(problems), 1)
        self.assertIn("30.00%", problems[0])

        census = check_isa.empty_census()
        census["instructions"] = 100
        census["vex"] = 1
        self.assertEqual(check_isa.census_problems("game.exe", census, 5.0), [])

    def test_problems_budget_the_dispatched_family(self):
        def census_with(mnemonic, count, functions):
            census = check_isa.empty_census()
            census["instructions"] = 1000
            census["canaries"][mnemonic] = count
            census["examples"][mnemonic] = 0x1010
            census["dispatched_functions"] = functions
            return census

        # The CRT's dispatched bodies: few instructions, few functions.
        self.assertEqual(
            check_isa.census_problems("game.exe", census_with("movbe", 12, 3), 5.0),
            [],
        )
        # Scattered everywhere is ordinary code compiled for a newer CPU.
        problems = check_isa.census_problems(
            "game.exe", census_with("movbe", 12, 99), 5.0
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("spread across 99 functions", problems[0])
        # And no single dispatched family is this large.
        problems = check_isa.census_problems(
            "game.exe", census_with("pcmpistri", 65, 3), 5.0
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("65 pcmpistri", problems[0])

    def test_attribute_canaries_groups_by_function(self):
        census = check_isa.empty_census()
        census["canary_sites"]["movbe"] = [0x1010, 0x1020, 0x2005, 0x3000]
        census["canary_sites"]["pcmpistri"] = [0x1018]
        census["canary_sites"]["popcnt"] = [0x1010]  # not a dispatched family
        check_isa.attribute_canaries(census, [(0x1000, 0x1100), (0x2000, 0x2100)])
        # Two attributed functions plus the site at 0x3000, which no
        # function holds and so counts against concentration on its own.
        self.assertEqual(census["dispatched_functions"], 3)
        self.assertEqual(len(census["dispatched_groups"]), 2)
        begin, end, counts = census["dispatched_groups"][0]
        self.assertEqual((begin, end), (0x1000, 0x1100))
        self.assertEqual(counts["movbe"], 2)
        self.assertEqual(counts["pcmpistri"], 1)

    def test_census_decodes_canaries_and_vex(self):
        try:
            import capstone  # noqa: F401
        except ImportError:
            self.skipTest("capstone not installed")
        census = check_isa.census_code(
            POPCNT_RAX + CRC32_RAX + MOVBE_RAX + VMOVDQA + NOP_RET, 0x1000, 64
        )
        self.assertEqual(census["instructions"], 6)
        self.assertEqual(census["canaries"]["popcnt"], 1)
        self.assertEqual(census["canaries"]["crc32"], 1)
        self.assertEqual(census["canaries"]["movbe"], 1)
        self.assertEqual(census["vex"], 1)
        self.assertEqual(census["evex"], 0)
        self.assertEqual(census["examples"]["popcnt"], 0x1000)
        self.assertEqual(len(census["canary_sites"]["movbe"]), 1)

    def test_check_verdict_on_synthetic_binaries(self):
        try:
            import capstone  # noqa: F401
        except ImportError:
            self.skipTest("capstone not installed")
        with tempfile.TemporaryDirectory() as tmp:
            legacy = Path(tmp, "legacy.exe")
            legacy.write_bytes(build_pe32_plus(NOP_RET))
            ok, _ = check_isa.check(legacy, max_vex_percent=100.0)
            self.assertTrue(ok)

            # One movbe is within the dispatched-CRT budget...
            dispatched = Path(tmp, "dispatched.exe")
            dispatched.write_bytes(build_pe32_plus(MOVBE_RAX + NOP_RET))
            ok, _ = check_isa.check(dispatched, max_vex_percent=100.0)
            self.assertTrue(ok)

            # ...but popcnt has no budget at all.
            modern = Path(tmp, "modern.exe")
            modern.write_bytes(build_pe32_plus(POPCNT_RAX + NOP_RET))
            ok, _ = check_isa.check(modern, max_vex_percent=100.0)
            self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
