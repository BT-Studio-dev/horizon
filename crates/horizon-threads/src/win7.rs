//! Windows 7 SP1 x64 compatibility shims for post-Windows 7 OS entry points.
//!
//! Rust 1.78+ `std` and third-party crates (`chrono`, `getrandom`, `windows-core`,
//! `windows-result`) link directly against entry points and API-set DLLs added in
//! Windows 8 or Windows 10 (`GetSystemTimePreciseAsFileTime`, `ProcessPrng`,
//! `WaitOnAddress`, `WakeByAddressSingle`, `WakeByAddressAll`, `GetTempPath2W`,
//! `GetHostNameW`, `RoGetActivationFactory`, `RoOriginateErrorW`, etc.).
//!
//! Defining both the unprefixed symbol (`Foo`) and the `dllimport` IAT pointer
//! (`__imp_Foo`) in the binary's object files causes the MSVC linker (`link.exe`)
//! to resolve those references locally before scanning `.lib` import libraries,
//! keeping the post-Windows 7 DLLs and entry points out of the PE import table.
//! At runtime each shim probes the real OS export once via `GetProcAddress` and
//! falls back to the Windows 7 equivalent when running on Windows 7.

#![allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::complexity,
    clippy::manual_c_str_literals,
    clippy::missing_safety_doc,
    clippy::missing_transmute_annotations,
    clippy::perf,
    clippy::style,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::upper_case_acronyms
)]

/// Maximum number of UTF-16 code units `WriteConsoleW` accepts per call on
/// Windows 7 before failing with `ERROR_NOT_ENOUGH_MEMORY`.
pub const WIN7_WRITE_CONSOLE_MAX_CHARS: u32 = 8192;

/// Number of keyed `SRWLOCK` + `CONDITION_VARIABLE` buckets used by the
/// Windows 7 `WaitOnAddress` / `WakeByAddress*` fallback.
pub const WAIT_ADDRESS_BUCKETS: usize = 64;

/// Compute the `WaitOnAddress` bucket index (`0..WAIT_ADDRESS_BUCKETS`) for `addr`.
#[inline]
pub const fn wait_bucket_index(addr: usize) -> usize {
    let mixed = (addr >> 2) ^ (addr >> 9) ^ (addr >> 16);
    mixed & (WAIT_ADDRESS_BUCKETS - 1)
}

/// Clamp a `WriteConsoleW` request length to the Windows 7 safe chunk size when
/// running without native `GetSystemTimePreciseAsFileTime` (Windows 7).
#[inline]
pub const fn write_console_chunk_len(remaining: u32, is_win8_or_later: bool) -> u32 {
    if !is_win8_or_later && remaining > WIN7_WRITE_CONSOLE_MAX_CHARS {
        WIN7_WRITE_CONSOLE_MAX_CHARS
    } else {
        remaining
    }
}

/// Compare `size` bytes (`1`, `2`, `4`, or `8`) at `address` and `compare_address`.
#[cfg(any(target_os = "windows", test))]
#[inline]
unsafe fn address_bytes_equal(
    address: *const core::ffi::c_void,
    compare_address: *const core::ffi::c_void,
    size: usize,
) -> Option<bool> {
    if address.is_null() || compare_address.is_null() {
        return None;
    }
    match size {
        1 => Some(unsafe { *(address as *const u8) == *(compare_address as *const u8) }),
        2 => Some(unsafe {
            core::ptr::read_unaligned(address as *const u16)
                == core::ptr::read_unaligned(compare_address as *const u16)
        }),
        4 => Some(unsafe {
            core::ptr::read_unaligned(address as *const u32)
                == core::ptr::read_unaligned(compare_address as *const u32)
        }),
        8 => Some(unsafe {
            core::ptr::read_unaligned(address as *const u64)
                == core::ptr::read_unaligned(compare_address as *const u64)
        }),
        _ => None,
    }
}

/// Ensure the Windows 7 import shims remain linked into the final executable.
#[inline]
pub fn init() {
    #[cfg(target_os = "windows")]
    imp::init();
}

#[cfg(target_os = "windows")]
mod imp {
    use core::ffi::c_void;
    use core::ptr;
    use core::sync::atomic::AtomicUsize;
    use core::sync::atomic::Ordering;

    pub type BOOL = i32;
    pub type HRESULT = i32;
    pub type HANDLE = isize;
    pub type HMODULE = isize;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct FILETIME {
        pub dwLowDateTime: u32,
        pub dwHighDateTime: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct GUID {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    #[repr(C)]
    pub struct SRWLOCK {
        pub ptr: *mut c_void,
    }

    unsafe impl Sync for SRWLOCK {}

    #[repr(C)]
    pub struct CONDITION_VARIABLE {
        pub ptr: *mut c_void,
    }

    unsafe impl Sync for CONDITION_VARIABLE {}

    #[repr(C)]
    struct WaitBucket {
        lock: SRWLOCK,
        cv: CONDITION_VARIABLE,
    }

    impl WaitBucket {
        const fn new() -> Self {
            Self {
                lock: SRWLOCK {
                    ptr: ptr::null_mut(),
                },
                cv: CONDITION_VARIABLE {
                    ptr: ptr::null_mut(),
                },
            }
        }
    }

    static WAIT_TABLE: [WaitBucket; super::WAIT_ADDRESS_BUCKETS] =
        [const { WaitBucket::new() }; super::WAIT_ADDRESS_BUCKETS];

    const ERROR_INVALID_PARAMETER: u32 = 87;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const TOKEN_QUERY: u32 = 0x0008;
    const CURRENT_PROCESS_TOKEN: HANDLE = -4;
    const FILE_FS_DEVICE_INFORMATION: u32 = 4;
    const FILE_DEVICE_CONSOLE: u32 = 0x0000_0050;
    const CP_UTF8: u32 = 65001;
    const WSAEFAULT: i32 = 10014;
    const REGDB_E_CLASSNOTREG: HRESULT = 0x8004_0154u32 as i32;
    const E_NOINTERFACE: HRESULT = 0x8000_4002u32 as i32;

    const KERNEL32_W: [u16; 13] = [
        b'k' as u16,
        b'e' as u16,
        b'r' as u16,
        b'n' as u16,
        b'e' as u16,
        b'l' as u16,
        b'3' as u16,
        b'2' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const NTDLL_W: [u16; 10] = [
        b'n' as u16,
        b't' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const ADVAPI32_W: [u16; 13] = [
        b'a' as u16,
        b'd' as u16,
        b'v' as u16,
        b'a' as u16,
        b'p' as u16,
        b'i' as u16,
        b'3' as u16,
        b'2' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const BCRYPTPRIMITIVES_W: [u16; 21] = [
        b'b' as u16,
        b'c' as u16,
        b'r' as u16,
        b'y' as u16,
        b'p' as u16,
        b't' as u16,
        b'p' as u16,
        b'r' as u16,
        b'i' as u16,
        b'm' as u16,
        b'i' as u16,
        b't' as u16,
        b'i' as u16,
        b'v' as u16,
        b'e' as u16,
        b's' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const SYNCH_APISET_W: [u16; 32] = [
        b'a' as u16,
        b'p' as u16,
        b'i' as u16,
        b'-' as u16,
        b'm' as u16,
        b's' as u16,
        b'-' as u16,
        b'w' as u16,
        b'i' as u16,
        b'n' as u16,
        b'-' as u16,
        b'c' as u16,
        b'o' as u16,
        b'r' as u16,
        b'e' as u16,
        b'-' as u16,
        b's' as u16,
        b'y' as u16,
        b'n' as u16,
        b'c' as u16,
        b'h' as u16,
        b'-' as u16,
        b'l' as u16,
        b'1' as u16,
        b'-' as u16,
        b'2' as u16,
        b'-' as u16,
        b'0' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const USERENV_W: [u16; 12] = [
        b'u' as u16,
        b's' as u16,
        b'e' as u16,
        b'r' as u16,
        b'e' as u16,
        b'n' as u16,
        b'v' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const WS2_32_W: [u16; 11] = [
        b'w' as u16,
        b's' as u16,
        b'2' as u16,
        b'_' as u16,
        b'3' as u16,
        b'2' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const COMBASE_W: [u16; 12] = [
        b'c' as u16,
        b'o' as u16,
        b'm' as u16,
        b'b' as u16,
        b'a' as u16,
        b's' as u16,
        b'e' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];
    const OLE32_W: [u16; 10] = [
        b'o' as u16,
        b'l' as u16,
        b'e' as u16,
        b'3' as u16,
        b'2' as u16,
        b'.' as u16,
        b'd' as u16,
        b'l' as u16,
        b'l' as u16,
        0,
    ];

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> HMODULE;
        fn LoadLibraryW(lpLibFileName: *const u16) -> HMODULE;
        fn GetProcAddress(hModule: HMODULE, lpProcName: *const u8) -> *const c_void;
        fn GetSystemTimeAsFileTime(lpSystemTimeAsFileTime: *mut FILETIME);
        fn GetTempPathW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
        fn AcquireSRWLockExclusive(SRWLock: *mut SRWLOCK);
        fn ReleaseSRWLockExclusive(SRWLock: *mut SRWLOCK);
        fn SleepConditionVariableSRW(
            ConditionVariable: *mut CONDITION_VARIABLE,
            SRWLock: *mut SRWLOCK,
            dwMilliseconds: u32,
            Flags: u32,
        ) -> BOOL;
        fn WakeAllConditionVariable(ConditionVariable: *mut CONDITION_VARIABLE);
        fn GetLastError() -> u32;
        fn SetLastError(dwErrCode: u32);
        fn GetCurrentProcess() -> HANDLE;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
        fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut u32) -> BOOL;
        fn MultiByteToWideChar(
            CodePage: u32,
            dwFlags: u32,
            lpMultiByteStr: *const u8,
            cbMultiByte: i32,
            lpWideCharStr: *mut u16,
            cchWideChar: i32,
        ) -> i32;
    }

    const UNRESOLVED: usize = 0;
    const MISSING: usize = 1;

    #[inline]
    unsafe fn resolve_cached(
        slot: &AtomicUsize,
        dll_wide: &[u16],
        load_if_needed: bool,
        proc_name: &'static [u8],
    ) -> Option<usize> {
        let cached = slot.load(Ordering::Acquire);
        if cached == MISSING {
            return None;
        }
        if cached != UNRESOLVED {
            return Some(cached);
        }
        let mut module = unsafe { GetModuleHandleW(dll_wide.as_ptr()) };
        if module == 0 && load_if_needed {
            module = unsafe { LoadLibraryW(dll_wide.as_ptr()) };
        }
        let addr = if module != 0 {
            unsafe { GetProcAddress(module, proc_name.as_ptr()) as usize }
        } else {
            0
        };
        let stored = if addr == 0 { MISSING } else { addr };
        slot.store(stored, Ordering::Release);
        if addr == 0 { None } else { Some(addr) }
    }

    #[inline]
    fn has_precise_time() -> bool {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        unsafe {
            resolve_cached(
                &SLOT,
                &KERNEL32_W,
                false,
                b"GetSystemTimePreciseAsFileTime\0",
            )
            .is_some()
        }
    }

    macro_rules! define_shim {
        (
            $fn_name:ident,
            $imp_name:literal,
            $imp_static:ident,
            ($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $ret:ty)?
            $body:block
        ) => {
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $fn_name($($arg: $arg_ty),*) $(-> $ret)? {
                $body
            }

            #[used]
            #[unsafe(export_name = $imp_name)]
            pub static $imp_static: unsafe extern "system" fn($($arg_ty),*) $(-> $ret)? = $fn_name;
        };
    }

    define_shim!(
        GetSystemTimePreciseAsFileTime,
        "__imp_GetSystemTimePreciseAsFileTime",
        IMP_GET_SYSTEM_TIME_PRECISE_AS_FILE_TIME,
        (out: *mut FILETIME) {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &KERNEL32_W, false, b"GetSystemTimePreciseAsFileTime\0")
            } {
                let func: unsafe extern "system" fn(*mut FILETIME) =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(out) };
            } else {
                unsafe { GetSystemTimeAsFileTime(out) };
            }
        }
    );

    define_shim!(
        GetTempPath2W,
        "__imp_GetTempPath2W",
        IMP_GET_TEMP_PATH_2W,
        (buffer_len: u32, buffer: *mut u16) -> u32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &KERNEL32_W, false, b"GetTempPath2W\0")
            } {
                let func: unsafe extern "system" fn(u32, *mut u16) -> u32 =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(buffer_len, buffer) }
            } else {
                unsafe { GetTempPathW(buffer_len, buffer) }
            }
        }
    );

    define_shim!(
        ProcessPrng,
        "__imp_ProcessPrng",
        IMP_PROCESS_PRNG,
        (data: *mut u8, len: usize) -> BOOL {
            static PRNG_SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&PRNG_SLOT, &BCRYPTPRIMITIVES_W, true, b"ProcessPrng\0")
            } {
                let func: unsafe extern "system" fn(*mut u8, usize) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(data, len) };
            }
            static RTL_SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(rtl_addr) = (unsafe {
                resolve_cached(&RTL_SLOT, &ADVAPI32_W, true, b"SystemFunction036\0")
            }) else {
                return 0;
            };
            let rtl_gen_random: unsafe extern "system" fn(*mut c_void, u32) -> u8 =
                unsafe { core::mem::transmute(rtl_addr) };
            let mut offset = 0usize;
            while offset < len {
                let remaining = len - offset;
                let chunk = if remaining > u32::MAX as usize {
                    u32::MAX
                } else {
                    remaining as u32
                };
                let ok = unsafe { rtl_gen_random(data.add(offset).cast::<c_void>(), chunk) };
                if ok == 0 {
                    return 0;
                }
                offset += chunk as usize;
            }
            1
        }
    );

    define_shim!(
        WaitOnAddress,
        "__imp_WaitOnAddress",
        IMP_WAIT_ON_ADDRESS,
        (
            address: *const c_void,
            compare_address: *const c_void,
            address_size: usize,
            milliseconds: u32,
        ) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &SYNCH_APISET_W, true, b"WaitOnAddress\0")
            } {
                let func: unsafe extern "system" fn(*const c_void, *const c_void, usize, u32) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(address, compare_address, address_size, milliseconds) };
            }
            if !matches!(address_size, 1 | 2 | 4 | 8) || address.is_null() || compare_address.is_null() {
                unsafe { SetLastError(ERROR_INVALID_PARAMETER) };
                return 0;
            }
            let bucket = &WAIT_TABLE[super::wait_bucket_index(address as usize)];
            let lock_ptr = ptr::addr_of!(bucket.lock).cast_mut();
            let cv_ptr = ptr::addr_of!(bucket.cv).cast_mut();
            unsafe { AcquireSRWLockExclusive(lock_ptr) };
            let mut ok = 1;
            if unsafe { super::address_bytes_equal(address, compare_address, address_size) }
                == Some(true)
            {
                ok = unsafe { SleepConditionVariableSRW(cv_ptr, lock_ptr, milliseconds, 0) };
            }
            unsafe { ReleaseSRWLockExclusive(lock_ptr) };
            ok
        }
    );

    define_shim!(
        WakeByAddressSingle,
        "__imp_WakeByAddressSingle",
        IMP_WAKE_BY_ADDRESS_SINGLE,
        (address: *const c_void) {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &SYNCH_APISET_W, true, b"WakeByAddressSingle\0")
            } {
                let func: unsafe extern "system" fn(*const c_void) =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(address) };
                return;
            }
            let bucket = &WAIT_TABLE[super::wait_bucket_index(address as usize)];
            let lock_ptr = ptr::addr_of!(bucket.lock).cast_mut();
            let cv_ptr = ptr::addr_of!(bucket.cv).cast_mut();
            unsafe {
                AcquireSRWLockExclusive(lock_ptr);
                ReleaseSRWLockExclusive(lock_ptr);
                WakeAllConditionVariable(cv_ptr);
            }
        }
    );

    define_shim!(
        WakeByAddressAll,
        "__imp_WakeByAddressAll",
        IMP_WAKE_BY_ADDRESS_ALL,
        (address: *const c_void) {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &SYNCH_APISET_W, true, b"WakeByAddressAll\0")
            } {
                let func: unsafe extern "system" fn(*const c_void) =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(address) };
                return;
            }
            let bucket = &WAIT_TABLE[super::wait_bucket_index(address as usize)];
            let lock_ptr = ptr::addr_of!(bucket.lock).cast_mut();
            let cv_ptr = ptr::addr_of!(bucket.cv).cast_mut();
            unsafe {
                AcquireSRWLockExclusive(lock_ptr);
                ReleaseSRWLockExclusive(lock_ptr);
                WakeAllConditionVariable(cv_ptr);
            }
        }
    );

    define_shim!(
        GetHostNameW,
        "__imp_GetHostNameW",
        IMP_GET_HOST_NAME_W,
        (name: *mut u16, namelen: i32) -> i32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &WS2_32_W, true, b"GetHostNameW\0")
            } {
                let func: unsafe extern "system" fn(*mut u16, i32) -> i32 =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(name, namelen) };
            }
            static ANSI_SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(ansi_addr) = (unsafe {
                resolve_cached(&ANSI_SLOT, &WS2_32_W, true, b"gethostname\0")
            }) else {
                return -1;
            };
            let gethostname_fn: unsafe extern "system" fn(*mut u8, i32) -> i32 =
                unsafe { core::mem::transmute(ansi_addr) };
            let mut buf = [0u8; 256];
            let rc = unsafe { gethostname_fn(buf.as_mut_ptr(), buf.len() as i32) };
            if rc != 0 {
                return rc;
            }
            let written = unsafe {
                MultiByteToWideChar(CP_UTF8, 0, buf.as_ptr(), -1, name, namelen)
            };
            if written == 0 {
                unsafe { SetLastError(WSAEFAULT as u32) };
                -1
            } else {
                0
            }
        }
    );

    define_shim!(
        GetUserProfileDirectoryW,
        "__imp_GetUserProfileDirectoryW",
        IMP_GET_USER_PROFILE_DIRECTORY_W,
        (token: HANDLE, profile_dir: *mut u16, cch_size: *mut u32) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = (unsafe {
                resolve_cached(&SLOT, &USERENV_W, true, b"GetUserProfileDirectoryW\0")
            }) else {
                unsafe { SetLastError(ERROR_INVALID_PARAMETER) };
                return 0;
            };
            let func: unsafe extern "system" fn(HANDLE, *mut u16, *mut u32) -> BOOL =
                unsafe { core::mem::transmute(addr) };
            if token != CURRENT_PROCESS_TOKEN || has_precise_time() {
                let ok = unsafe { func(token, profile_dir, cch_size) };
                if ok != 0 || token != CURRENT_PROCESS_TOKEN {
                    return ok;
                }
                let err = unsafe { GetLastError() };
                if err == ERROR_INSUFFICIENT_BUFFER {
                    return 0;
                }
            }
            static OPEN_TOKEN_SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(open_addr) = (unsafe {
                resolve_cached(&OPEN_TOKEN_SLOT, &ADVAPI32_W, true, b"OpenProcessToken\0")
            }) else {
                return 0;
            };
            let open_process_token: unsafe extern "system" fn(HANDLE, u32, *mut HANDLE) -> BOOL =
                unsafe { core::mem::transmute(open_addr) };
            let mut real_token: HANDLE = 0;
            if unsafe { open_process_token(GetCurrentProcess(), TOKEN_QUERY, &mut real_token) } == 0 {
                return 0;
            }
            let ok = unsafe { func(real_token, profile_dir, cch_size) };
            let saved_error = unsafe { GetLastError() };
            unsafe {
                CloseHandle(real_token);
                SetLastError(saved_error);
            }
            ok
        }
    );

    define_shim!(
        NtQueryVolumeInformationFile,
        "__imp_NtQueryVolumeInformationFile",
        IMP_NT_QUERY_VOLUME_INFORMATION_FILE,
        (
            file_handle: HANDLE,
            io_status_block: *mut usize,
            fs_information: *mut u8,
            length: u32,
            fs_information_class: u32,
        ) -> i32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = (unsafe {
                resolve_cached(&SLOT, &NTDLL_W, false, b"NtQueryVolumeInformationFile\0")
            }) else {
                return -1;
            };
            let func: unsafe extern "system" fn(HANDLE, *mut usize, *mut u8, u32, u32) -> i32 =
                unsafe { core::mem::transmute(addr) };
            let status = unsafe {
                func(
                    file_handle,
                    io_status_block,
                    fs_information,
                    length,
                    fs_information_class,
                )
            };
            if status < 0
                && fs_information_class == FILE_FS_DEVICE_INFORMATION
                && length >= 8
                && !fs_information.is_null()
            {
                let mut mode = 0u32;
                if unsafe { GetConsoleMode(file_handle, &mut mode) } != 0 {
                    unsafe {
                        ptr::write_unaligned(fs_information as *mut u32, FILE_DEVICE_CONSOLE);
                        ptr::write_unaligned(fs_information.add(4) as *mut u32, 0);
                    }
                    return 0;
                }
            }
            status
        }
    );

    define_shim!(
        WriteConsoleW,
        "__imp_WriteConsoleW",
        IMP_WRITE_CONSOLE_W,
        (
            console_output: HANDLE,
            buffer: *const c_void,
            chars_to_write: u32,
            chars_written: *mut u32,
            reserved: *const c_void,
        ) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = (unsafe {
                resolve_cached(&SLOT, &KERNEL32_W, false, b"WriteConsoleW\0")
            }) else {
                return 0;
            };
            let func: unsafe extern "system" fn(
                HANDLE,
                *const c_void,
                u32,
                *mut u32,
                *const c_void,
            ) -> BOOL = unsafe { core::mem::transmute(addr) };
            let win8 = has_precise_time();
            if win8 || chars_to_write <= super::WIN7_WRITE_CONSOLE_MAX_CHARS || buffer.is_null() {
                return unsafe {
                    func(
                        console_output,
                        buffer,
                        chars_to_write,
                        chars_written,
                        reserved,
                    )
                };
            }
            let wide_ptr = buffer as *const u16;
            let mut total_written = 0u32;
            while total_written < chars_to_write {
                let remaining = chars_to_write - total_written;
                let chunk = super::write_console_chunk_len(remaining, false);
                let mut step_written = 0u32;
                let ok = unsafe {
                    func(
                        console_output,
                        wide_ptr.add(total_written as usize).cast::<c_void>(),
                        chunk,
                        &mut step_written,
                        reserved,
                    )
                };
                total_written += step_written;
                if !chars_written.is_null() {
                    unsafe { *chars_written = total_written };
                }
                if ok == 0 || step_written == 0 {
                    return ok;
                }
            }
            1
        }
    );

    define_shim!(
        RoOriginateErrorW,
        "__imp_RoOriginateErrorW",
        IMP_RO_ORIGINATE_ERROR_W,
        (error: HRESULT, cch_max: u32, message: *const u16) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &COMBASE_W, true, b"RoOriginateErrorW\0")
            } {
                let func: unsafe extern "system" fn(HRESULT, u32, *const u16) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(error, cch_max, message) }
            } else {
                0
            }
        }
    );

    define_shim!(
        RoOriginateError,
        "__imp_RoOriginateError",
        IMP_RO_ORIGINATE_ERROR,
        (error: HRESULT, message: isize) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &COMBASE_W, true, b"RoOriginateError\0")
            } {
                let func: unsafe extern "system" fn(HRESULT, isize) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(error, message) }
            } else {
                0
            }
        }
    );

    define_shim!(
        RoGetActivationFactory,
        "__imp_RoGetActivationFactory",
        IMP_RO_GET_ACTIVATION_FACTORY,
        (activatable_class_id: isize, iid: *const GUID, factory: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &COMBASE_W, true, b"RoGetActivationFactory\0")
            } {
                let func: unsafe extern "system" fn(isize, *const GUID, *mut *mut c_void) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(activatable_class_id, iid, factory) }
            } else {
                if !factory.is_null() {
                    unsafe { *factory = ptr::null_mut() };
                }
                REGDB_E_CLASSNOTREG
            }
        }
    );

    define_shim!(
        RoGetAgileReference,
        "__imp_RoGetAgileReference",
        IMP_RO_GET_AGILE_REFERENCE,
        (
            options: i32,
            riid: *const GUID,
            unk: *mut c_void,
            agile_reference: *mut *mut c_void,
        ) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &OLE32_W, true, b"RoGetAgileReference\0")
            } {
                let func: unsafe extern "system" fn(
                    i32,
                    *const GUID,
                    *mut c_void,
                    *mut *mut c_void,
                ) -> HRESULT = unsafe { core::mem::transmute(addr) };
                unsafe { func(options, riid, unk, agile_reference) }
            } else {
                if !agile_reference.is_null() {
                    unsafe { *agile_reference = ptr::null_mut() };
                }
                E_NOINTERFACE
            }
        }
    );

    define_shim!(
        CoIncrementMTAUsage,
        "__imp_CoIncrementMTAUsage",
        IMP_CO_INCREMENT_MTA_USAGE,
        (cookie: *mut isize) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &OLE32_W, true, b"CoIncrementMTAUsage\0")
            } {
                let func: unsafe extern "system" fn(*mut isize) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(cookie) };
            }
            if !cookie.is_null() {
                unsafe { *cookie = 1 };
            }
            static CO_INIT_SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&CO_INIT_SLOT, &OLE32_W, true, b"CoInitializeEx\0")
            } {
                let co_init: unsafe extern "system" fn(*const c_void, u32) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                let hr = unsafe { co_init(ptr::null(), 0) };
                if hr >= 0 {
                    return 0;
                }
            }
            0
        }
    );

    pub fn init() {
        let refs: [usize; 14] = [
            ptr::addr_of!(IMP_GET_SYSTEM_TIME_PRECISE_AS_FILE_TIME) as usize,
            ptr::addr_of!(IMP_GET_TEMP_PATH_2W) as usize,
            ptr::addr_of!(IMP_PROCESS_PRNG) as usize,
            ptr::addr_of!(IMP_WAIT_ON_ADDRESS) as usize,
            ptr::addr_of!(IMP_WAKE_BY_ADDRESS_SINGLE) as usize,
            ptr::addr_of!(IMP_WAKE_BY_ADDRESS_ALL) as usize,
            ptr::addr_of!(IMP_GET_HOST_NAME_W) as usize,
            ptr::addr_of!(IMP_GET_USER_PROFILE_DIRECTORY_W) as usize,
            ptr::addr_of!(IMP_NT_QUERY_VOLUME_INFORMATION_FILE) as usize,
            ptr::addr_of!(IMP_WRITE_CONSOLE_W) as usize,
            ptr::addr_of!(IMP_RO_ORIGINATE_ERROR_W) as usize,
            ptr::addr_of!(IMP_RO_ORIGINATE_ERROR) as usize,
            ptr::addr_of!(IMP_RO_GET_ACTIVATION_FACTORY) as usize,
            ptr::addr_of!(IMP_RO_GET_AGILE_REFERENCE) as usize
                ^ ptr::addr_of!(IMP_CO_INCREMENT_MTA_USAGE) as usize,
        ];
        core::hint::black_box(refs);
    }
}

#[cfg(test)]
mod tests {
    use core::ffi::c_void;
    use core::ptr;

    #[test]
    fn wait_bucket_indices_stay_in_bounds_and_spread_addresses() {
        for shift in 0..32 {
            let idx = super::wait_bucket_index(1usize << shift);
            assert!(idx < super::WAIT_ADDRESS_BUCKETS);
        }
        let a = super::wait_bucket_index(0x1000_0004);
        let b = super::wait_bucket_index(0x1000_0008);
        assert_ne!(a, b);
    }

    #[test]
    fn write_console_chunking_caps_windows_7_calls() {
        assert_eq!(
            super::write_console_chunk_len(20_000, false),
            super::WIN7_WRITE_CONSOLE_MAX_CHARS
        );
        assert_eq!(super::write_console_chunk_len(4096, false), 4096);
        assert_eq!(super::write_console_chunk_len(20_000, true), 20_000);
    }

    #[test]
    fn address_bytes_comparison_handles_all_supported_widths() {
        let a8 = 0x12u8;
        let b8 = 0x12u8;
        let c8 = 0x34u8;
        let a32 = 0xdead_beefu32;
        let b32 = 0xdead_beefu32;
        let c32 = 0xdead_beeeu32;
        let a64 = 0x0102_0304_0506_0708u64;
        let b64 = 0x0102_0304_0506_0708u64;
        unsafe {
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a8).cast::<c_void>(),
                    ptr::from_ref(&b8).cast::<c_void>(),
                    1,
                ),
                Some(true)
            );
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a8).cast::<c_void>(),
                    ptr::from_ref(&c8).cast::<c_void>(),
                    1,
                ),
                Some(false)
            );
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a32).cast::<c_void>(),
                    ptr::from_ref(&b32).cast::<c_void>(),
                    4,
                ),
                Some(true)
            );
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a32).cast::<c_void>(),
                    ptr::from_ref(&c32).cast::<c_void>(),
                    4,
                ),
                Some(false)
            );
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a64).cast::<c_void>(),
                    ptr::from_ref(&b64).cast::<c_void>(),
                    8,
                ),
                Some(true)
            );
            assert_eq!(
                super::address_bytes_equal(
                    ptr::from_ref(&a64).cast::<c_void>(),
                    ptr::from_ref(&b64).cast::<c_void>(),
                    3,
                ),
                None
            );
        }
        super::init();
    }
}
