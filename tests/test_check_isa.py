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

    def test_census_decodes_canaries_and_vex(self):
        try:
            import capstone  # noqa: F401
        except ImportError:
            self.skipTest("capstone not installed")
        census = check_isa.census_code(POPCNT_RAX + CRC32_RAX + VMOVDQA + NOP_RET, 0x1000, 64)
        self.assertEqual(census["instructions"], 5)
        self.assertEqual(census["canaries"]["popcnt"], 1)
        self.assertEqual(census["canaries"]["crc32"], 1)
        self.assertEqual(census["vex"], 1)
        self.assertEqual(census["evex"], 0)
        self.assertEqual(census["examples"]["popcnt"], 0x1000)

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

            modern = Path(tmp, "modern.exe")
            modern.write_bytes(build_pe32_plus(POPCNT_RAX + NOP_RET))
            ok, _ = check_isa.check(modern, max_vex_percent=100.0)
            self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
