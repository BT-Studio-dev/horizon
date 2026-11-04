from pathlib import Path
import struct
import tempfile
import unittest

from tools import check_imports


def build_pe32_plus(imports):
    """Build a minimal PE32+ image whose .idata section encodes `[(dll, [symbols_or_ordinals])]`."""
    pe_offset = 0x80
    optional_size = 240
    section_rva = 0x1000
    section_raw = 0x200

    idata = bytearray()

    def push(blob, align=1):
        if len(idata) % align:
            idata.extend(b"\0" * (align - (len(idata) % align)))
        rva = section_rva + len(idata)
        idata.extend(blob)
        return rva

    # Reserve space for (N + 1) 20-byte IMAGE_IMPORT_DESCRIPTORs at the start of .idata.
    descriptors_pos = len(idata)
    idata.extend(b"\0" * (20 * (len(imports) + 1)))

    for index, (dll, symbols) in enumerate(imports):
        dll_rva = push(dll.encode("ascii") + b"\0")
        thunk_entries = []
        for sym in symbols:
            if isinstance(sym, int):
                thunk_entries.append((1 << 63) | sym)
            else:
                hint_name_rva = push(struct.pack("<H", 0) + sym.encode("ascii") + b"\0", align=2)
                thunk_entries.append(hint_name_rva)
        thunk_entries.append(0)
        int_rva = push(b"".join(struct.pack("<Q", entry) for entry in thunk_entries), align=8)
        iat_rva = push(b"".join(struct.pack("<Q", entry) for entry in thunk_entries), align=8)
        struct.pack_into("<IIIII", idata, descriptors_pos + 20 * index, int_rva, 0, 0, dll_rva, iat_rva)

    raw_size = max(0x200, (len(idata) + 0x1FF) & ~0x1FF)
    idata.extend(b"\0" * (raw_size - len(idata)))

    image = bytearray(section_raw + raw_size)
    image[0:2] = b"MZ"
    struct.pack_into("<I", image, 0x3C, pe_offset)
    image[pe_offset:pe_offset + 4] = b"PE\0\0"
    # COFF File Header: Machine=AMD64 (0x8664), NumberOfSections=1, SizeOfOptionalHeader=240
    struct.pack_into("<HHIIIHH", image, pe_offset + 4, 0x8664, 1, 0, 0, 0, optional_size, 0x0022)
    optional = pe_offset + 24
    # PE32+ magic = 0x20B; data directories start at optional + 112.
    struct.pack_into("<H", image, optional, 0x20B)
    struct.pack_into("<I", image, optional + 108, 16)
    # Directory 1 (Import Table): RVA = section_rva, Size = 20 * (len(imports) + 1)
    struct.pack_into("<II", image, optional + 112 + 8 * 1, section_rva, 20 * (len(imports) + 1))
    # Section Header (.idata)
    section_hdr = optional + optional_size
    image[section_hdr:section_hdr + 8] = b".idata\0\0"
    struct.pack_into("<IIII", image, section_hdr + 8, len(idata), section_rva, raw_size, section_raw)
    image[section_raw:section_raw + raw_size] = idata
    return bytes(image)


class CheckImportsTests(unittest.TestCase):
    def write_pe(self, directory, name, imports):
        path = Path(directory, name)
        path.write_bytes(build_pe32_plus(imports))
        return path

    def test_win7_clean_binary_passes_both_default_and_win7_modes(self):
        with tempfile.TemporaryDirectory() as tmp:
            exe = self.write_pe(tmp, "horizon-launcher.exe", [
                ("kernel32.dll", ["GetSystemTimeAsFileTime", "GetProcAddress", "LoadLibraryW", 1]),
                ("user32.dll", ["CreateWindowExW", "GetDC", "GetSystemMetrics"]),
                ("gdi32.dll", ["GetDeviceCaps"]),
                ("d3d11.dll", ["D3D11CreateDevice"]),
                ("dxgi.dll", ["CreateDXGIFactory1"]),
                ("dwrite.dll", ["DWriteCreateFactory"]),
                ("uiautomationcore.dll", ["UiaReturnRawElementProvider"]),
                ("api-ms-win-core-processthreads-l1-1-0.dll", ["GetCurrentThreadId"]),
            ])
            self.assertEqual(check_imports.pe_imports(exe.read_bytes())[0], "kernel32.dll")
            self.assertTrue(check_imports.check(exe, win7=False))
            self.assertTrue(check_imports.check(exe, win7=True))

    def test_win7_mode_rejects_get_system_time_precise_as_file_time(self):
        with tempfile.TemporaryDirectory() as tmp:
            exe = self.write_pe(tmp, "broken-win8-time.exe", [
                ("kernel32.dll", ["GetSystemTimePreciseAsFileTime"]),
            ])
            # Default Windows 10/11 check allows kernel32.dll; --win7 catches the Win8+ symbol.
            self.assertTrue(check_imports.check(exe, win7=False))
            self.assertFalse(check_imports.check(exe, win7=True))

    def test_win7_mode_rejects_createfile2(self):
        with tempfile.TemporaryDirectory() as tmp:
            dll = self.write_pe(tmp, "libgallium_wgl.dll", [
                ("kernel32.dll", ["CreateFile2"]),
            ])
            self.assertTrue(check_imports.check(dll, win7=False))
            self.assertFalse(check_imports.check(dll, win7=True))

    def test_win7_mode_rejects_post_win7_dlls_and_apisets(self):
        bad_dlls = [
            ("dcomp.dll", ["DCompositionCreateDevice"]),
            ("bcryptprimitives.dll", ["ProcessPrng"]),
            ("icuuc.dll", ["u_strlen"]),
            ("shcore.dll", ["GetDpiForMonitor"]),
            ("api-ms-win-core-synch-l1-2-0.dll", ["WaitOnAddress"]),
            ("api-ms-win-core-winrt-l1-1-0.dll", ["RoGetActivationFactory"]),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            for index, entry in enumerate(bad_dlls):
                with self.subTest(dll=entry[0]):
                    exe = self.write_pe(tmp, f"bad-{index}.exe", [entry])
                    self.assertTrue(check_imports.check(exe, win7=False))
                    self.assertFalse(check_imports.check(exe, win7=True))

    def test_non_system_dll_fails_in_all_modes(self):
        with tempfile.TemporaryDirectory() as tmp:
            exe = self.write_pe(tmp, "vcruntime.exe", [
                ("VCRUNTIME140.dll", ["memcpy"]),
            ])
            self.assertFalse(check_imports.check(exe, win7=False))
            self.assertFalse(check_imports.check(exe, win7=True))

    def test_app_local_mode_allows_a_checked_dll_dependency(self):
        with tempfile.TemporaryDirectory() as tmp:
            loader = self.write_pe(tmp, "opengl32.dll", [
                ("kernel32.dll", ["LoadLibraryW"]),
                ("libgallium_wgl.dll", ["wglCreateContext"]),
            ])
            self.assertFalse(check_imports.check(loader, win7=True))
            self.assertTrue(check_imports.check(
                loader,
                win7=True,
                app_local=True,
                local_dlls={"libgallium_wgl.dll"},
            ))
            # App-local does not bless a DLL that was not part of the checked set.
            self.assertFalse(check_imports.check(
                loader,
                win7=True,
                app_local=True,
                local_dlls={"some_other.dll"},
            ))


if __name__ == "__main__":
    unittest.main()
