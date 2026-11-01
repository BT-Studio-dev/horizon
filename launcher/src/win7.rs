//! Windows 7 SP1 x64 compatibility layer for `horizon-launcher`.
//!
//! Bridges post-Windows 7 Win32, WinRT, DXGI 1.2–1.6, DirectComposition,
//! DirectManipulation, HiDPI, ICU, and DirectWrite 1.1–1.5 entry points onto
//! Windows 7 SP1 x64 APIs while delegating directly to the native OS exports
//! whenever running on Windows 8, 10, or 11.

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

use core::ffi::c_void;

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

/// Count UTF-16 code units up to the first NUL terminator (`icuuc.dll!u_strlen` replacement).
#[inline]
pub unsafe fn utf16_strlen(ptr: *const u16) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    len as i32
}

/// Scale a system metric from `system_dpi` to `target_dpi`.
#[inline]
pub const fn scale_metric_for_dpi(metric: i32, target_dpi: u32, system_dpi: u32) -> i32 {
    if system_dpi == 0 || target_dpi == 0 || target_dpi == system_dpi {
        return metric;
    }
    let num = metric as i64 * target_dpi as i64;
    let half = system_dpi as i64 / 2;
    let adjusted = if num >= 0 { num + half } else { num - half };
    (adjusted / system_dpi as i64) as i32
}

/// Compare `size` bytes (`1`, `2`, `4`, or `8`) at `address` and `compare_address`.
#[inline]
pub unsafe fn address_bytes_equal(
    address: *const c_void,
    compare_address: *const c_void,
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

pub mod com_abi {
    use core::ffi::c_void;

    pub type BOOL = i32;
    pub type HRESULT = i32;
    pub type HANDLE = isize;
    pub type HWND = isize;
    pub type HMONITOR = isize;
    pub type HMODULE = isize;

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct GUID {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    impl GUID {
        pub const fn from_u128(uuid: u128) -> Self {
            Self {
                data1: (uuid >> 96) as u32,
                data2: ((uuid >> 80) & 0xffff) as u16,
                data3: ((uuid >> 64) & 0xffff) as u16,
                data4: (uuid as u64).to_be_bytes(),
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct FILETIME {
        pub dwLowDateTime: u32,
        pub dwHighDateTime: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct LUID {
        pub LowPart: u32,
        pub HighPart: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct Vector2 {
        pub X: f32,
        pub Y: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_RATIONAL {
        pub Numerator: u32,
        pub Denominator: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_SAMPLE_DESC {
        pub Count: u32,
        pub Quality: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_MODE_DESC {
        pub Width: u32,
        pub Height: u32,
        pub RefreshRate: DXGI_RATIONAL,
        pub Format: u32,
        pub ScanlineOrdering: i32,
        pub Scaling: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_SWAP_CHAIN_DESC {
        pub BufferDesc: DXGI_MODE_DESC,
        pub SampleDesc: DXGI_SAMPLE_DESC,
        pub BufferUsage: u32,
        pub BufferCount: u32,
        pub OutputWindow: HWND,
        pub Windowed: BOOL,
        pub SwapEffect: i32,
        pub Flags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_SWAP_CHAIN_DESC1 {
        pub Width: u32,
        pub Height: u32,
        pub Format: u32,
        pub Stereo: BOOL,
        pub SampleDesc: DXGI_SAMPLE_DESC,
        pub BufferUsage: u32,
        pub BufferCount: u32,
        pub Scaling: i32,
        pub SwapEffect: i32,
        pub AlphaMode: i32,
        pub Flags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DXGI_RGBA {
        pub r: f32,
        pub g: f32,
        pub b: f32,
        pub a: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DWRITE_FONT_METRICS {
        pub designUnitsPerEm: u16,
        pub ascent: u16,
        pub descent: u16,
        pub lineGap: i16,
        pub capHeight: u16,
        pub xHeight: u16,
        pub underlinePosition: i16,
        pub underlineThickness: u16,
        pub strikethroughPosition: i16,
        pub strikethroughThickness: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DWRITE_FONT_METRICS1 {
        pub base: DWRITE_FONT_METRICS,
        pub glyphBoxLeft: i16,
        pub glyphBoxTop: i16,
        pub glyphBoxRight: i16,
        pub glyphBoxBottom: i16,
        pub subscriptPositionX: i16,
        pub subscriptPositionY: i16,
        pub subscriptSizeX: i16,
        pub subscriptSizeY: i16,
        pub superscriptPositionX: i16,
        pub superscriptPositionY: i16,
        pub superscriptSizeX: i16,
        pub superscriptSizeY: i16,
        pub hasTypographicMetrics: BOOL,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DWRITE_MATRIX {
        pub m11: f32,
        pub m12: f32,
        pub m21: f32,
        pub m22: f32,
        pub dx: f32,
        pub dy: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DWRITE_TEXT_RANGE {
        pub startPosition: u32,
        pub length: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DWRITE_GLYPH_RUN {
        pub fontFace: *mut c_void,
        pub fontEmSize: f32,
        pub glyphCount: u32,
        pub glyphIndices: *const u16,
        pub glyphAdvances: *const f32,
        pub glyphOffsets: *const c_void,
        pub isSideways: BOOL,
        pub bidiLevel: u32,
    }

    #[repr(C)]
    pub struct IUnknown_Vtbl {
        pub QueryInterface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
        pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    pub struct IDXGIObject_Vtbl {
        pub base: IUnknown_Vtbl,
        pub SetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void) -> HRESULT,
        pub SetPrivateDataInterface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut c_void) -> HRESULT,
        pub GetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut u32, *mut c_void) -> HRESULT,
        pub GetParent:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIDevice_Vtbl {
        pub base: IDXGIObject_Vtbl,
        pub GetAdapter: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateSurface: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            u32,
            u32,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub QueryResourceResidency:
            unsafe extern "system" fn(*mut c_void, *const *mut c_void, *mut i32, u32) -> HRESULT,
        pub SetGPUThreadPriority: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub GetGPUThreadPriority: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIDeviceSubObject_Vtbl {
        pub base: IDXGIObject_Vtbl,
        pub GetDevice:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIFactory_Vtbl {
        pub base: IDXGIObject_Vtbl,
        pub EnumAdapters: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub MakeWindowAssociation: unsafe extern "system" fn(*mut c_void, HWND, u32) -> HRESULT,
        pub GetWindowAssociation: unsafe extern "system" fn(*mut c_void, *mut HWND) -> HRESULT,
        pub CreateSwapChain: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const DXGI_SWAP_CHAIN_DESC,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateSoftwareAdapter:
            unsafe extern "system" fn(*mut c_void, HMODULE, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIFactory1_Vtbl {
        pub base: IDXGIFactory_Vtbl,
        pub EnumAdapters1: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub IsCurrent: unsafe extern "system" fn(*mut c_void) -> BOOL,
    }

    #[repr(C)]
    pub struct IDXGIFactory2_Vtbl {
        pub base: IDXGIFactory1_Vtbl,
        pub IsWindowedStereoEnabled: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub CreateSwapChainForHwnd: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            HWND,
            *const DXGI_SWAP_CHAIN_DESC1,
            *const c_void,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateSwapChainForCoreWindow: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            *const DXGI_SWAP_CHAIN_DESC1,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub GetSharedResourceAdapterLuid:
            unsafe extern "system" fn(*mut c_void, HANDLE, *mut LUID) -> HRESULT,
        pub RegisterStereoStatusWindow:
            unsafe extern "system" fn(*mut c_void, HWND, u32, *mut u32) -> HRESULT,
        pub RegisterStereoStatusEvent:
            unsafe extern "system" fn(*mut c_void, HANDLE, *mut u32) -> HRESULT,
        pub UnregisterStereoStatus: unsafe extern "system" fn(*mut c_void, u32),
        pub RegisterOcclusionStatusWindow:
            unsafe extern "system" fn(*mut c_void, HWND, u32, *mut u32) -> HRESULT,
        pub RegisterOcclusionStatusEvent:
            unsafe extern "system" fn(*mut c_void, HANDLE, *mut u32) -> HRESULT,
        pub UnregisterOcclusionStatus: unsafe extern "system" fn(*mut c_void, u32),
        pub CreateSwapChainForComposition: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const DXGI_SWAP_CHAIN_DESC1,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIFactory3_Vtbl {
        pub base: IDXGIFactory2_Vtbl,
        pub GetCreationFlags: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    pub struct IDXGIFactory4_Vtbl {
        pub base: IDXGIFactory3_Vtbl,
        pub EnumAdapterByLuid:
            unsafe extern "system" fn(*mut c_void, LUID, *const GUID, *mut *mut c_void) -> HRESULT,
        pub EnumWarpAdapter:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIFactory5_Vtbl {
        pub base: IDXGIFactory4_Vtbl,
        pub CheckFeatureSupport:
            unsafe extern "system" fn(*mut c_void, i32, *mut c_void, u32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIFactory6_Vtbl {
        pub base: IDXGIFactory5_Vtbl,
        pub EnumAdapterByGpuPreference: unsafe extern "system" fn(
            *mut c_void,
            u32,
            i32,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGISwapChain_Vtbl {
        pub base: IDXGIDeviceSubObject_Vtbl,
        pub Present: unsafe extern "system" fn(*mut c_void, u32, u32) -> HRESULT,
        pub GetBuffer:
            unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> HRESULT,
        pub SetFullscreenState:
            unsafe extern "system" fn(*mut c_void, BOOL, *mut c_void) -> HRESULT,
        pub GetFullscreenState:
            unsafe extern "system" fn(*mut c_void, *mut BOOL, *mut *mut c_void) -> HRESULT,
        pub GetDesc: unsafe extern "system" fn(*mut c_void, *mut DXGI_SWAP_CHAIN_DESC) -> HRESULT,
        pub ResizeBuffers:
            unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32, u32) -> HRESULT,
        pub ResizeTarget: unsafe extern "system" fn(*mut c_void, *const DXGI_MODE_DESC) -> HRESULT,
        pub GetContainingOutput: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFrameStatistics: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub GetLastPresentCount: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGISwapChain1_Vtbl {
        pub base: IDXGISwapChain_Vtbl,
        pub GetDesc1: unsafe extern "system" fn(*mut c_void, *mut DXGI_SWAP_CHAIN_DESC1) -> HRESULT,
        pub GetFullscreenDesc: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub GetHwnd: unsafe extern "system" fn(*mut c_void, *mut HWND) -> HRESULT,
        pub GetCoreWindow:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub Present1:
            unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void) -> HRESULT,
        pub IsTemporaryMonoSupported: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetRestrictToOutput:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub SetBackgroundColor: unsafe extern "system" fn(*mut c_void, *const DXGI_RGBA) -> HRESULT,
        pub GetBackgroundColor: unsafe extern "system" fn(*mut c_void, *mut DXGI_RGBA) -> HRESULT,
        pub SetRotation: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub GetRotation: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetSystemFontCollection:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void, BOOL) -> HRESULT,
        pub CreateCustomFontCollection: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            u32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub RegisterFontCollectionLoader:
            unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub UnregisterFontCollectionLoader:
            unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub CreateFontFileReference: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            *const FILETIME,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateCustomFontFileReference: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            u32,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateFontFace: unsafe extern "system" fn(
            *mut c_void,
            i32,
            u32,
            *const *mut c_void,
            u32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateRenderingParams:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateMonitorRenderingParams:
            unsafe extern "system" fn(*mut c_void, HMONITOR, *mut *mut c_void) -> HRESULT,
        pub CreateCustomRenderingParams: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            i32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub RegisterFontFileLoader: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub UnregisterFontFileLoader:
            unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub CreateTextFormat: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            *mut c_void,
            i32,
            i32,
            i32,
            f32,
            *const u16,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateTypography: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetGdiInterop: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateTextLayout: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            u32,
            *mut c_void,
            f32,
            f32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateGdiCompatibleTextLayout: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            u32,
            *mut c_void,
            f32,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateEllipsisTrimmingSign:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateTextAnalyzer:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateNumberSubstitution:
            unsafe extern "system" fn(*mut c_void, i32, *const u16, BOOL, *mut *mut c_void) -> HRESULT,
        pub CreateGlyphRunAnalysis: unsafe extern "system" fn(
            *mut c_void,
            *const DWRITE_GLYPH_RUN,
            f32,
            *const DWRITE_MATRIX,
            i32,
            i32,
            f32,
            f32,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory1_Vtbl {
        pub base: IDWriteFactory_Vtbl,
        pub GetEudcFontCollection:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void, BOOL) -> HRESULT,
        pub CreateCustomRenderingParams1: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            f32,
            i32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory2_Vtbl {
        pub base: IDWriteFactory1_Vtbl,
        pub GetSystemFontFallback:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateFontFallbackBuilder:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub TranslateColorGlyphRun2: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *const DWRITE_GLYPH_RUN,
            *const c_void,
            i32,
            *const DWRITE_MATRIX,
            u32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateCustomRenderingParams2: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            f32,
            i32,
            i32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateGlyphRunAnalysis2: unsafe extern "system" fn(
            *mut c_void,
            *const DWRITE_GLYPH_RUN,
            *const DWRITE_MATRIX,
            i32,
            i32,
            i32,
            i32,
            f32,
            f32,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory3_Vtbl {
        pub base: IDWriteFactory2_Vtbl,
        pub CreateGlyphRunAnalysis3: unsafe extern "system" fn(
            *mut c_void,
            *const DWRITE_GLYPH_RUN,
            *const DWRITE_MATRIX,
            i32,
            i32,
            i32,
            i32,
            f32,
            f32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateCustomRenderingParams3: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            f32,
            i32,
            i32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateFontFaceReference: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            u32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateFontFaceReference2: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            *const FILETIME,
            u32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub GetSystemFontSet: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateFontSetBuilder3:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateFontCollectionFromFontSet:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetSystemFontCollection3:
            unsafe extern "system" fn(*mut c_void, BOOL, *mut *mut c_void, BOOL) -> HRESULT,
        pub GetFontDownloadQueue:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory4_Vtbl {
        pub base: IDWriteFactory3_Vtbl,
        pub TranslateColorGlyphRun4: unsafe extern "system" fn(
            *mut c_void,
            Vector2,
            *const DWRITE_GLYPH_RUN,
            *const c_void,
            u32,
            i32,
            *const DWRITE_MATRIX,
            u32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub ComputeGlyphOrigins: unsafe extern "system" fn(
            *mut c_void,
            *const DWRITE_GLYPH_RUN,
            Vector2,
            *mut Vector2,
        ) -> HRESULT,
        pub ComputeGlyphOrigins2: unsafe extern "system" fn(
            *mut c_void,
            *const DWRITE_GLYPH_RUN,
            i32,
            Vector2,
            *const DWRITE_MATRIX,
            *mut Vector2,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFactory5_Vtbl {
        pub base: IDWriteFactory4_Vtbl,
        pub CreateFontSetBuilder5:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateInMemoryFontFileLoader:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateHttpFontFileLoader: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            *const u16,
            *mut *mut c_void,
        ) -> HRESULT,
        pub AnalyzeContainerType:
            unsafe extern "system" fn(*mut c_void, *const c_void, u32) -> i32,
        pub UnpackFontFile: unsafe extern "system" fn(
            *mut c_void,
            i32,
            *const c_void,
            u32,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontCollection_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetFontFamilyCount: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetFontFamily: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub FindFamilyName:
            unsafe extern "system" fn(*mut c_void, *const u16, *mut u32, *mut BOOL) -> HRESULT,
        pub GetFontFromFontFace:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontCollection1_Vtbl {
        pub base: IDWriteFontCollection_Vtbl,
        pub GetFontSet: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFontFamily1:
            unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontList_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetFontCollection: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFontCount: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetFont: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFamily_Vtbl {
        pub base: IDWriteFontList_Vtbl,
        pub GetFamilyNames: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFirstMatchingFont:
            unsafe extern "system" fn(*mut c_void, i32, i32, i32, *mut *mut c_void) -> HRESULT,
        pub GetMatchingFonts:
            unsafe extern "system" fn(*mut c_void, i32, i32, i32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFamily1_Vtbl {
        pub base: IDWriteFontFamily_Vtbl,
        pub GetFontLocality: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        pub GetFont1: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub GetFontFaceReference:
            unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFont_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetFontFamily: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetWeight: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetStretch: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetStyle: unsafe extern "system" fn(*mut c_void) -> i32,
        pub IsSymbolFont: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetFaceNames: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetInformationalStrings:
            unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void, *mut BOOL) -> HRESULT,
        pub GetSimulations: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetMetrics: unsafe extern "system" fn(*mut c_void, *mut DWRITE_FONT_METRICS),
        pub HasCharacter: unsafe extern "system" fn(*mut c_void, u32, *mut BOOL) -> HRESULT,
        pub CreateFontFace: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFace_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetType: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFiles:
            unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut c_void) -> HRESULT,
        pub GetIndex: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetSimulations: unsafe extern "system" fn(*mut c_void) -> i32,
        pub IsSymbolFont: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetMetrics: unsafe extern "system" fn(*mut c_void, *mut DWRITE_FONT_METRICS),
        pub GetGlyphCount: unsafe extern "system" fn(*mut c_void) -> u16,
        pub GetDesignGlyphMetrics:
            unsafe extern "system" fn(*mut c_void, *const u16, u32, *mut c_void, BOOL) -> HRESULT,
        pub GetGlyphIndices:
            unsafe extern "system" fn(*mut c_void, *const u32, u32, *mut u16) -> HRESULT,
        pub TryGetFontTable: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut *const c_void,
            *mut u32,
            *mut *mut c_void,
            *mut BOOL,
        ) -> HRESULT,
        pub ReleaseFontTable: unsafe extern "system" fn(*mut c_void, *const c_void),
        pub GetGlyphRunOutline: unsafe extern "system" fn(
            *mut c_void,
            f32,
            *const u16,
            *const f32,
            *const c_void,
            u32,
            BOOL,
            BOOL,
            *mut c_void,
        ) -> HRESULT,
        pub GetRecommendedRenderingMode: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            i32,
            *mut c_void,
            *mut i32,
        ) -> HRESULT,
        pub GetGdiCompatibleMetrics: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *const DWRITE_MATRIX,
            *mut DWRITE_FONT_METRICS,
        ) -> HRESULT,
        pub GetGdiCompatibleGlyphMetrics: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            *const u16,
            u32,
            *mut c_void,
            BOOL,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFace1_Vtbl {
        pub base: IDWriteFontFace_Vtbl,
        pub GetMetrics1: unsafe extern "system" fn(*mut c_void, *mut DWRITE_FONT_METRICS1),
        pub GetGdiCompatibleMetrics1: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *const DWRITE_MATRIX,
            *mut DWRITE_FONT_METRICS1,
        ) -> HRESULT,
        pub GetCaretMetrics: unsafe extern "system" fn(*mut c_void, *mut c_void),
        pub GetUnicodeRanges:
            unsafe extern "system" fn(*mut c_void, u32, *mut c_void, *mut u32) -> HRESULT,
        pub IsMonospacedFont: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetDesignGlyphAdvances:
            unsafe extern "system" fn(*mut c_void, u32, *const u16, *mut i32, BOOL) -> HRESULT,
        pub GetGdiCompatibleGlyphAdvances: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            BOOL,
            u32,
            *const u16,
            *mut i32,
        ) -> HRESULT,
        pub GetKerningPairAdjustments:
            unsafe extern "system" fn(*mut c_void, u32, *const u16, *mut i32) -> HRESULT,
        pub HasKerningPairs: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetRecommendedRenderingMode1: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            i32,
            i32,
            *mut i32,
        ) -> HRESULT,
        pub GetVerticalGlyphVariants:
            unsafe extern "system" fn(*mut c_void, u32, *const u16, *mut u16) -> HRESULT,
        pub HasVerticalGlyphVariants: unsafe extern "system" fn(*mut c_void) -> BOOL,
    }

    #[repr(C)]
    pub struct IDWriteFontFace2_Vtbl {
        pub base: IDWriteFontFace1_Vtbl,
        pub IsColorFont: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub GetColorPaletteCount: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetPaletteEntryCount: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetPaletteEntries:
            unsafe extern "system" fn(*mut c_void, u32, u32, u32, *mut c_void) -> HRESULT,
        pub GetRecommendedRenderingMode2: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            i32,
            i32,
            *mut c_void,
            *mut i32,
            *mut i32,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFace3_Vtbl {
        pub base: IDWriteFontFace2_Vtbl,
        pub GetFontFaceReference: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetPanose: unsafe extern "system" fn(*mut c_void, *mut c_void),
        pub GetWeight: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetStretch: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetStyle: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFamilyNames: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFaceNames: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetInformationalStrings:
            unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void, *mut BOOL) -> HRESULT,
        pub HasCharacter: unsafe extern "system" fn(*mut c_void, u32) -> BOOL,
        pub GetRecommendedRenderingMode3: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            f32,
            *const DWRITE_MATRIX,
            BOOL,
            i32,
            i32,
            *mut c_void,
            *mut i32,
            *mut i32,
        ) -> HRESULT,
        pub IsCharacterLocal: unsafe extern "system" fn(*mut c_void, u32) -> BOOL,
        pub IsGlyphLocal: unsafe extern "system" fn(*mut c_void, u16) -> BOOL,
        pub AreCharactersLocal:
            unsafe extern "system" fn(*mut c_void, *const u16, u32, BOOL, *mut BOOL) -> HRESULT,
        pub AreGlyphsLocal:
            unsafe extern "system" fn(*mut c_void, *const u16, u32, BOOL, *mut BOOL) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFaceReference_Vtbl {
        pub base: IUnknown_Vtbl,
        pub CreateFontFace: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateFontFaceWithSimulations:
            unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT,
        pub Equals: unsafe extern "system" fn(*mut c_void, *mut c_void) -> BOOL,
        pub GetFontFaceIndex: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetSimulations: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFontFile: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetLocalFileSize: unsafe extern "system" fn(*mut c_void) -> u64,
        pub GetFileSize: unsafe extern "system" fn(*mut c_void) -> u64,
        pub GetFileTime: unsafe extern "system" fn(*mut c_void, *mut FILETIME) -> HRESULT,
        pub GetLocality: unsafe extern "system" fn(*mut c_void) -> i32,
        pub EnqueueFontDownloadRequest: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub EnqueueCharacterDownloadRequest:
            unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
        pub EnqueueGlyphDownloadRequest:
            unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
        pub EnqueueFileFragmentDownloadRequest:
            unsafe extern "system" fn(*mut c_void, u64, u64) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontSet_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetFontCount: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetFontFaceReference:
            unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub FindFontFaceReference:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32, *mut BOOL) -> HRESULT,
        pub FindFontFace:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32, *mut BOOL) -> HRESULT,
        pub GetPropertyValues:
            unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT,
        pub GetPropertyValues2:
            unsafe extern "system" fn(*mut c_void, i32, *const u16, *mut *mut c_void) -> HRESULT,
        pub GetPropertyValues3: unsafe extern "system" fn(
            *mut c_void,
            u32,
            i32,
            *mut BOOL,
            *mut *mut c_void,
        ) -> HRESULT,
        pub GetPropertyOccurrenceCount:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut u32) -> HRESULT,
        pub GetMatchingFonts: unsafe extern "system" fn(
            *mut c_void,
            *const u16,
            i32,
            i32,
            i32,
            *mut *mut c_void,
        ) -> HRESULT,
        pub GetMatchingFonts2:
            unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontSetBuilder_Vtbl {
        pub base: IUnknown_Vtbl,
        pub AddFontFaceReference:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_void, u32) -> HRESULT,
        pub AddFontFaceReference2: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub AddFontSet: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub CreateFontSet: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontSetBuilder1_Vtbl {
        pub base: IDWriteFontSetBuilder_Vtbl,
        pub AddFontFile: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFileLoader_Vtbl {
        pub base: IUnknown_Vtbl,
        pub CreateStreamFromKey:
            unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteInMemoryFontFileLoader_Vtbl {
        pub base: IDWriteFontFileLoader_Vtbl,
        pub CreateInMemoryFontFileReference: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            u32,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub GetFileCount: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    pub struct IDWriteFontFileStream_Vtbl {
        pub base: IUnknown_Vtbl,
        pub ReadFileFragment: unsafe extern "system" fn(
            *mut c_void,
            *mut *const c_void,
            u64,
            u64,
            *mut *mut c_void,
        ) -> HRESULT,
        pub ReleaseFileFragment: unsafe extern "system" fn(*mut c_void, *mut c_void),
        pub GetFileSize: unsafe extern "system" fn(*mut c_void, *mut u64) -> HRESULT,
        pub GetLastWriteTime: unsafe extern "system" fn(*mut c_void, *mut u64) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontCollectionLoader_Vtbl {
        pub base: IUnknown_Vtbl,
        pub CreateEnumeratorFromKey: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            u32,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFileEnumerator_Vtbl {
        pub base: IUnknown_Vtbl,
        pub MoveNext: unsafe extern "system" fn(*mut c_void, *mut BOOL) -> HRESULT,
        pub GetCurrentFontFile: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteTextFormat_Vtbl {
        pub base: IUnknown_Vtbl,
        pub SetTextAlignment: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetParagraphAlignment: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetWordWrapping: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetReadingDirection: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetFlowDirection: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetIncrementalTabStop: unsafe extern "system" fn(*mut c_void, f32) -> HRESULT,
        pub SetTrimming:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut c_void) -> HRESULT,
        pub SetLineSpacing: unsafe extern "system" fn(*mut c_void, i32, f32, f32) -> HRESULT,
        pub GetTextAlignment: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetParagraphAlignment: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetWordWrapping: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetReadingDirection: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFlowDirection: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetIncrementalTabStop: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetTrimming:
            unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetLineSpacing:
            unsafe extern "system" fn(*mut c_void, *mut i32, *mut f32, *mut f32) -> HRESULT,
        pub GetFontCollection: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub GetFontFamilyNameLength: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetFontFamilyName: unsafe extern "system" fn(*mut c_void, *mut u16, u32) -> HRESULT,
        pub GetFontWeight: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFontStyle: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFontStretch: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetFontSize: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetLocaleNameLength: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetLocaleName: unsafe extern "system" fn(*mut c_void, *mut u16, u32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteTextFormat1_Vtbl {
        pub base: IDWriteTextFormat_Vtbl,
        pub SetVerticalGlyphOrientation: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub GetVerticalGlyphOrientation: unsafe extern "system" fn(*mut c_void) -> i32,
        pub SetLastLineWrapping: unsafe extern "system" fn(*mut c_void, BOOL) -> HRESULT,
        pub GetLastLineWrapping: unsafe extern "system" fn(*mut c_void) -> BOOL,
        pub SetOpticalAlignment: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub GetOpticalAlignment: unsafe extern "system" fn(*mut c_void) -> i32,
        pub SetFontFallback: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub GetFontFallback: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteTextLayout_Vtbl {
        pub base: IDWriteTextFormat_Vtbl,
        pub SetMaxWidth: unsafe extern "system" fn(*mut c_void, f32) -> HRESULT,
        pub SetMaxHeight: unsafe extern "system" fn(*mut c_void, f32) -> HRESULT,
        pub SetFontCollection:
            unsafe extern "system" fn(*mut c_void, *mut c_void, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetFontFamilyName:
            unsafe extern "system" fn(*mut c_void, *const u16, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetFontWeight:
            unsafe extern "system" fn(*mut c_void, i32, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetFontStyle:
            unsafe extern "system" fn(*mut c_void, i32, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetFontStretch:
            unsafe extern "system" fn(*mut c_void, i32, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetFontSize:
            unsafe extern "system" fn(*mut c_void, f32, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetUnderline:
            unsafe extern "system" fn(*mut c_void, BOOL, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetStrikethrough:
            unsafe extern "system" fn(*mut c_void, BOOL, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetDrawingEffect:
            unsafe extern "system" fn(*mut c_void, *mut c_void, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetInlineObject:
            unsafe extern "system" fn(*mut c_void, *mut c_void, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetTypography:
            unsafe extern "system" fn(*mut c_void, *mut c_void, DWRITE_TEXT_RANGE) -> HRESULT,
        pub SetLocaleName:
            unsafe extern "system" fn(*mut c_void, *const u16, DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetMaxWidth: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetMaxHeight: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetFontCollectionLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut *mut c_void,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetFontFamilyNameLengthLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut u32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetFontFamilyNameLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut u16,
            u32,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetFontWeightLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut i32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetFontStyleLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut i32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetFontStretchLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut i32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetFontSizeLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut f32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetUnderlineLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut BOOL,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetStrikethroughLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut BOOL,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetDrawingEffectLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut *mut c_void,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetInlineObjectLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut *mut c_void,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetTypographyLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut *mut c_void,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub GetLocaleNameLengthLayout:
            unsafe extern "system" fn(*mut c_void, u32, *mut u32, *mut DWRITE_TEXT_RANGE) -> HRESULT,
        pub GetLocaleNameLayout: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut u16,
            u32,
            *mut DWRITE_TEXT_RANGE,
        ) -> HRESULT,
        pub Draw: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            *mut c_void,
            f32,
            f32,
        ) -> HRESULT,
        pub GetLineMetrics:
            unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> HRESULT,
        pub GetMetrics: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub GetOverhangMetrics: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub GetClusterMetrics:
            unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> HRESULT,
        pub DetermineMinWidth: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
        pub HitTestPoint: unsafe extern "system" fn(
            *mut c_void,
            f32,
            f32,
            *mut BOOL,
            *mut BOOL,
            *mut c_void,
        ) -> HRESULT,
        pub HitTestTextPosition: unsafe extern "system" fn(
            *mut c_void,
            u32,
            BOOL,
            *mut f32,
            *mut f32,
            *mut c_void,
        ) -> HRESULT,
        pub HitTestTextRange: unsafe extern "system" fn(
            *mut c_void,
            u32,
            u32,
            f32,
            f32,
            *mut c_void,
            u32,
            *mut u32,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWritePixelSnapping_Vtbl {
        pub base: IUnknown_Vtbl,
        pub IsPixelSnappingDisabled:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut BOOL) -> HRESULT,
        pub GetCurrentTransform:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut DWRITE_MATRIX) -> HRESULT,
        pub GetPixelsPerDip:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut f32) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteTextRenderer_Vtbl {
        pub base: IDWritePixelSnapping_Vtbl,
        pub DrawGlyphRun: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            f32,
            f32,
            i32,
            *const DWRITE_GLYPH_RUN,
            *const c_void,
            *mut c_void,
        ) -> HRESULT,
        pub DrawUnderline: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            f32,
            f32,
            *const c_void,
            *mut c_void,
        ) -> HRESULT,
        pub DrawStrikethrough: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            f32,
            f32,
            *const c_void,
            *mut c_void,
        ) -> HRESULT,
        pub DrawInlineObject: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            f32,
            f32,
            *mut c_void,
            BOOL,
            BOOL,
            *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteRenderingParams_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetGamma: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetEnhancedContrast: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetClearTypeLevel: unsafe extern "system" fn(*mut c_void) -> f32,
        pub GetPixelGeometry: unsafe extern "system" fn(*mut c_void) -> i32,
        pub GetRenderingMode: unsafe extern "system" fn(*mut c_void) -> i32,
    }

    #[repr(C)]
    pub struct IDWriteRenderingParams1_Vtbl {
        pub base: IDWriteRenderingParams_Vtbl,
        pub GetGrayscaleEnhancedContrast: unsafe extern "system" fn(*mut c_void) -> f32,
    }

    #[repr(C)]
    pub struct IDWriteGlyphRunAnalysis_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetAlphaTextureBounds:
            unsafe extern "system" fn(*mut c_void, i32, *mut RECT) -> HRESULT,
        pub CreateAlphaTexture:
            unsafe extern "system" fn(*mut c_void, i32, *const RECT, *mut u8, u32) -> HRESULT,
        pub GetAlphaBlendParams: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *mut f32,
            *mut f32,
            *mut f32,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFallback_Vtbl {
        pub base: IUnknown_Vtbl,
        pub MapCharacters: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            u32,
            u32,
            *mut c_void,
            *const u16,
            i32,
            i32,
            i32,
            f32,
            *mut u32,
            *mut *mut c_void,
            *mut f32,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDWriteFontFallbackBuilder_Vtbl {
        pub base: IUnknown_Vtbl,
        pub AddMapping: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            u32,
            *const *const u16,
            u32,
            *mut c_void,
            *const u16,
            *const u16,
            f32,
        ) -> HRESULT,
        pub AddMappings: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub CreateFontFallback:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDirectManipulationManager_Vtbl {
        pub base: IUnknown_Vtbl,
        pub Activate: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
        pub Deactivate: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
        pub RegisterHitTestTarget:
            unsafe extern "system" fn(*mut c_void, HWND, HWND, i32) -> HRESULT,
        pub ProcessInput:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut BOOL) -> HRESULT,
        pub GetUpdateManager:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub CreateViewport: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            HWND,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateContent: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const GUID,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDirectManipulationUpdateManager_Vtbl {
        pub base: IUnknown_Vtbl,
        pub RegisterWaitHandleCallback:
            unsafe extern "system" fn(*mut c_void, HANDLE, *mut c_void, *mut u32) -> HRESULT,
        pub UnregisterWaitHandleCallback: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
        pub Update: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDirectManipulationViewport_Vtbl {
        pub base: IUnknown_Vtbl,
        pub Enable: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub Disable: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub SetContact: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
        pub ReleaseContact: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
        pub ReleaseAllContacts: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub GetStatus: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
        pub GetTag: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            *mut *mut c_void,
            *mut u32,
        ) -> HRESULT,
        pub SetTag: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
        pub GetViewportRect: unsafe extern "system" fn(*mut c_void, *mut RECT) -> HRESULT,
        pub SetViewportRect: unsafe extern "system" fn(*mut c_void, *const RECT) -> HRESULT,
        pub ZoomToRect:
            unsafe extern "system" fn(*mut c_void, f32, f32, f32, f32, BOOL) -> HRESULT,
        pub SetViewportTransform:
            unsafe extern "system" fn(*mut c_void, *const f32, u32) -> HRESULT,
        pub SyncDisplayTransform:
            unsafe extern "system" fn(*mut c_void, *const f32, u32) -> HRESULT,
        pub GetPrimaryContent:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub AddContent: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub RemoveContent: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
        pub SetViewportOptions: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub AddConfiguration: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub RemoveConfiguration: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub ActivateConfiguration: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetManualGesture: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetChaining: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub AddEventHandler:
            unsafe extern "system" fn(*mut c_void, HWND, *mut c_void, *mut u32) -> HRESULT,
        pub RemoveEventHandler: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
        pub SetInputMode: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub SetUpdateMode: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
        pub Stop: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub Abandon: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    }
}

/// Initialize the Windows 7 runtime shims and configure DirectComposition fallback.
#[inline]
pub fn init() {
    #[cfg(target_os = "windows")]
    imp::init();
}

#[rustfmt::skip]
#[cfg(target_os = "windows")]
#[path = "win7/dwrite.rs"]
mod dwrite;

#[cfg(target_os = "windows")]
mod imp {
    use super::com_abi::*;
    use super::{
        WAIT_ADDRESS_BUCKETS, WIN7_WRITE_CONSOLE_MAX_CHARS, address_bytes_equal, dwrite,
        scale_metric_for_dpi, utf16_strlen, wait_bucket_index, write_console_chunk_len,
    };
    use core::ffi::c_void;
    use core::ptr;
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

    pub(super) const S_OK: HRESULT = 0;
    pub(super) const E_NOTIMPL: HRESULT = 0x8000_4001u32 as i32;
    pub(super) const E_NOINTERFACE: HRESULT = 0x8000_4002u32 as i32;
    pub(super) const E_POINTER: HRESULT = 0x8000_4003u32 as i32;
    pub(super) const E_INVALIDARG: HRESULT = 0x8007_0057u32 as i32;
    const REGDB_E_CLASSNOTREG: HRESULT = 0x8004_0154u32 as i32;
    const DXGI_ERROR_NOT_FOUND: HRESULT = 0x887A_0002u32 as i32;

    const ERROR_INVALID_PARAMETER: u32 = 87;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;
    const TOKEN_QUERY: u32 = 0x0008;
    const CURRENT_PROCESS_TOKEN: HANDLE = -4;
    const FILE_FS_DEVICE_INFORMATION: u32 = 4;
    const FILE_DEVICE_CONSOLE: u32 = 0x0000_0050;
    const CP_UTF8: u32 = 65001;
    const WSAEFAULT: i32 = 10014;
    const LOGPIXELSX: i32 = 88;
    const USER_DEFAULT_SCREEN_DPI: u32 = 96;

    const D3D_DRIVER_TYPE_WARP: i32 = 2;
    const D3D_FEATURE_LEVEL_11_0: u32 = 0xb000;
    const D3D_FEATURE_LEVEL_10_1: u32 = 0xa100;
    const D3D11_CREATE_DEVICE_BGRA_SUPPORT: u32 = 0x20;
    const D3D11_SDK_VERSION: u32 = 7;
    const DIRECTMANIPULATION_READY: i32 = 1;

    pub(super) const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);

    const IID_IDXGI_OBJECT: GUID = GUID::from_u128(0xaec22fb8_76f3_4639_9be0_28eb43a67a2e);
    const IID_IDXGI_DEVICE_SUB_OBJECT: GUID =
        GUID::from_u128(0x3d3e0379_f9de_4d58_bb6c_18d62992f1a6);
    const IID_IDXGI_DEVICE: GUID = GUID::from_u128(0x54ec77fa_1377_44e6_8c32_88fd5f44c84c);
    const IID_IDXGI_ADAPTER1: GUID = GUID::from_u128(0x29038f61_3839_4626_91fd_086879011a05);
    const IID_IDXGI_FACTORY: GUID = GUID::from_u128(0x7b7166ec_21c7_44ae_b21a_c9ae321ae369);
    const IID_IDXGI_FACTORY1: GUID = GUID::from_u128(0x770aae78_f26f_4dba_a829_253c83d1b387);
    const IID_IDXGI_FACTORY2: GUID = GUID::from_u128(0x50c83a1c_e072_4c48_87b0_3630fa36a6d0);
    const IID_IDXGI_FACTORY3: GUID = GUID::from_u128(0x25483823_cd46_4c7d_86ca_47aa95b837bd);
    const IID_IDXGI_FACTORY4: GUID = GUID::from_u128(0x1bc6ea02_ef36_464f_bf0c_21ca39e5168a);
    const IID_IDXGI_FACTORY5: GUID = GUID::from_u128(0x7632e1f5_ee65_4dca_87fd_84cd75f8838d);
    const IID_IDXGI_FACTORY6: GUID = GUID::from_u128(0xc1b6694f_ff09_44a9_b03c_77900a0a1d17);
    const IID_IDXGI_SWAP_CHAIN: GUID = GUID::from_u128(0x310d36a0_d2e7_4c0a_aa04_6a9d23b8886a);
    const IID_IDXGI_SWAP_CHAIN1: GUID = GUID::from_u128(0x790a45f7_0d42_4876_983a_0a55cfe6f4aa);

    const CLSID_DIRECT_MANIPULATION_MANAGER: GUID =
        GUID::from_u128(0x54e211b6_3650_4f75_8334_fa359598e1c5);
    const IID_IDIRECT_MANIPULATION_MANAGER: GUID =
        GUID::from_u128(0xfbf5d3b4_70c7_4163_9322_5a6f660d6fbc);
    const IID_IDIRECT_MANIPULATION_UPDATE_MANAGER: GUID =
        GUID::from_u128(0xb0ae62fd_be34_46e7_9caa_d361facbb9cc);
    const IID_IDIRECT_MANIPULATION_VIEWPORT: GUID =
        GUID::from_u128(0x28b85a3d_60a0_48bd_9ba1_5ce8d9ea3a6d);

    #[repr(C)]
    struct SRWLOCK {
        ptr: *mut c_void,
    }
    unsafe impl Sync for SRWLOCK {}

    #[repr(C)]
    struct CONDITION_VARIABLE {
        ptr: *mut c_void,
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

    static WAIT_TABLE: [WaitBucket; WAIT_ADDRESS_BUCKETS] =
        [const { WaitBucket::new() }; WAIT_ADDRESS_BUCKETS];

    macro_rules! wide_lit {
        ($($ch:literal),+ $(,)?) => {
            [$($ch as u16),+, 0]
        };
    }

    const KERNEL32_W: [u16; 13] =
        wide_lit!('k', 'e', 'r', 'n', 'e', 'l', '3', '2', '.', 'd', 'l', 'l');
    const NTDLL_W: [u16; 10] = wide_lit!('n', 't', 'd', 'l', 'l', '.', 'd', 'l', 'l');
    const ADVAPI32_W: [u16; 13] =
        wide_lit!('a', 'd', 'v', 'a', 'p', 'i', '3', '2', '.', 'd', 'l', 'l');
    const USER32_W: [u16; 11] = wide_lit!('u', 's', 'e', 'r', '3', '2', '.', 'd', 'l', 'l');
    const BCRYPTPRIMITIVES_W: [u16; 21] = wide_lit!(
        'b', 'c', 'r', 'y', 'p', 't', 'p', 'r', 'i', 'm', 'i', 't', 'i', 'v', 'e', 's', '.', 'd',
        'l', 'l'
    );
    const SYNCH_APISET_W: [u16; 33] = wide_lit!(
        'a', 'p', 'i', '-', 'm', 's', '-', 'w', 'i', 'n', '-', 'c', 'o', 'r', 'e', '-', 's', 'y',
        'n', 'c', 'h', '-', 'l', '1', '-', '2', '-', '0', '.', 'd', 'l', 'l'
    );
    const USERENV_W: [u16; 12] = wide_lit!('u', 's', 'e', 'r', 'e', 'n', 'v', '.', 'd', 'l', 'l');
    const WS2_32_W: [u16; 11] = wide_lit!('w', 's', '2', '_', '3', '2', '.', 'd', 'l', 'l');
    const COMBASE_W: [u16; 12] = wide_lit!('c', 'o', 'm', 'b', 'a', 's', 'e', '.', 'd', 'l', 'l');
    const OLE32_W: [u16; 10] = wide_lit!('o', 'l', 'e', '3', '2', '.', 'd', 'l', 'l');
    const SHCORE_W: [u16; 11] = wide_lit!('s', 'h', 'c', 'o', 'r', 'e', '.', 'd', 'l', 'l');
    const DCOMP_W: [u16; 10] = wide_lit!('d', 'c', 'o', 'm', 'p', '.', 'd', 'l', 'l');
    const DXGI_W: [u16; 9] = wide_lit!('d', 'x', 'g', 'i', '.', 'd', 'l', 'l');
    const D3D11_W: [u16; 10] = wide_lit!('d', '3', 'd', '1', '1', '.', 'd', 'l', 'l');
    pub(super) const DWRITE_W: [u16; 11] =
        wide_lit!('d', 'w', 'r', 'i', 't', 'e', '.', 'd', 'l', 'l');
    const UIA_W: [u16; 21] = wide_lit!(
        'u', 'i', 'a', 'u', 't', 'o', 'm', 'a', 't', 'i', 'o', 'n', 'c', 'o', 'r', 'e', '.', 'd',
        'l', 'l'
    );

    #[link(name = "kernel32")]
    extern "system" {
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

    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hWnd: HWND) -> HANDLE;
        fn ReleaseDC(hWnd: HWND, hDC: HANDLE) -> i32;
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetDeviceCaps(hdc: HANDLE, index: i32) -> i32;
    }

    pub(super) const UNRESOLVED: usize = 0;
    const MISSING: usize = 1;

    #[inline]
    pub(super) unsafe fn resolve_cached(
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

    #[inline]
    fn has_dcomp() -> bool {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        unsafe { resolve_cached(&SLOT, &DCOMP_W, true, b"DCompositionCreateDevice\0").is_some() }
    }

    #[inline]
    fn system_screen_dpi() -> u32 {
        unsafe {
            let hdc = GetDC(0);
            if hdc == 0 {
                return USER_DEFAULT_SCREEN_DPI;
            }
            let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
            ReleaseDC(0, hdc);
            if dpi > 0 {
                dpi as u32
            } else {
                USER_DEFAULT_SCREEN_DPI
            }
        }
    }

    #[inline]
    pub(super) unsafe fn com_vtbl<V>(obj: *mut c_void) -> *const V {
        unsafe { *(obj as *const *const V) }
    }

    #[inline]
    pub(super) unsafe fn com_add_ref(obj: *mut c_void) {
        if !obj.is_null() {
            unsafe { ((*com_vtbl::<IUnknown_Vtbl>(obj)).AddRef)(obj) };
        }
    }

    #[inline]
    pub(super) unsafe fn com_release(obj: *mut c_void) {
        if !obj.is_null() {
            unsafe { ((*com_vtbl::<IUnknown_Vtbl>(obj)).Release)(obj) };
        }
    }

    #[inline]
    pub(super) unsafe fn com_qi(obj: *mut c_void, iid: &GUID) -> Option<*mut c_void> {
        if obj.is_null() {
            return None;
        }
        let mut out = ptr::null_mut();
        let hr = unsafe { ((*com_vtbl::<IUnknown_Vtbl>(obj)).QueryInterface)(obj, iid, &mut out) };
        if hr >= 0 && !out.is_null() {
            Some(out)
        } else {
            None
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
            #[no_mangle]
            pub unsafe extern "system" fn $fn_name($($arg: $arg_ty),*) $(-> $ret)? {
                $body
            }

            #[used]
            #[export_name = $imp_name]
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
            let bucket = &WAIT_TABLE[wait_bucket_index(address as usize)];
            let lock_ptr = ptr::addr_of!(bucket.lock).cast_mut();
            let cv_ptr = ptr::addr_of!(bucket.cv).cast_mut();
            unsafe { AcquireSRWLockExclusive(lock_ptr) };
            let mut ok = 1;
            if unsafe { address_bytes_equal(address, compare_address, address_size) } == Some(true) {
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
            let bucket = &WAIT_TABLE[wait_bucket_index(address as usize)];
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
            let bucket = &WAIT_TABLE[wait_bucket_index(address as usize)];
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
            if win8 || chars_to_write <= WIN7_WRITE_CONSOLE_MAX_CHARS || buffer.is_null() {
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
                let chunk = write_console_chunk_len(remaining, false);
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
                let _ = unsafe { co_init(ptr::null(), 0) };
            }
            S_OK
        }
    );

    #[no_mangle]
    pub unsafe extern "C" fn u_strlen(s: *const u16) -> i32 {
        unsafe { utf16_strlen(s) }
    }

    #[used]
    #[export_name = "__imp_u_strlen"]
    pub static IMP_U_STRLEN: unsafe extern "C" fn(*const u16) -> i32 = u_strlen;

    define_shim!(
        GetCurrentPackageFullName,
        "__imp_GetCurrentPackageFullName",
        IMP_GET_CURRENT_PACKAGE_FULL_NAME,
        (len: *mut u32, name: *mut u16) -> i32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &KERNEL32_W, false, b"GetCurrentPackageFullName\0")
            } {
                let func: unsafe extern "system" fn(*mut u32, *mut u16) -> i32 =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(len, name) }
            } else {
                APPMODEL_ERROR_NO_PACKAGE
            }
        }
    );

    define_shim!(
        GetDpiForMonitor,
        "__imp_GetDpiForMonitor",
        IMP_GET_DPI_FOR_MONITOR,
        (hmonitor: HMONITOR, dpi_type: i32, dpi_x: *mut u32, dpi_y: *mut u32) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &SHCORE_W, true, b"GetDpiForMonitor\0")
            } {
                let func: unsafe extern "system" fn(HMONITOR, i32, *mut u32, *mut u32) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(hmonitor, dpi_type, dpi_x, dpi_y) };
            }
            if dpi_x.is_null() || dpi_y.is_null() {
                return E_INVALIDARG;
            }
            let dpi = system_screen_dpi();
            unsafe {
                *dpi_x = dpi;
                *dpi_y = dpi;
            }
            S_OK
        }
    );

    define_shim!(
        GetDpiForWindow,
        "__imp_GetDpiForWindow",
        IMP_GET_DPI_FOR_WINDOW,
        (hwnd: HWND) -> u32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &USER32_W, false, b"GetDpiForWindow\0")
            } {
                let func: unsafe extern "system" fn(HWND) -> u32 =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(hwnd) }
            } else {
                system_screen_dpi()
            }
        }
    );

    define_shim!(
        GetSystemMetricsForDpi,
        "__imp_GetSystemMetricsForDpi",
        IMP_GET_SYSTEM_METRICS_FOR_DPI,
        (index: i32, dpi: u32) -> i32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &USER32_W, false, b"GetSystemMetricsForDpi\0")
            } {
                let func: unsafe extern "system" fn(i32, u32) -> i32 =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(index, dpi) }
            } else {
                let base = unsafe { GetSystemMetrics(index) };
                scale_metric_for_dpi(base, dpi, system_screen_dpi())
            }
        }
    );

    define_shim!(
        GetPointerType,
        "__imp_GetPointerType",
        IMP_GET_POINTER_TYPE,
        (pointer_id: u32, pointer_type: *mut i32) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &USER32_W, false, b"GetPointerType\0")
            } {
                let func: unsafe extern "system" fn(u32, *mut i32) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(pointer_id, pointer_type) }
            } else {
                0
            }
        }
    );

    define_shim!(
        RegisterSuspendResumeNotification,
        "__imp_RegisterSuspendResumeNotification",
        IMP_REGISTER_SUSPEND_RESUME_NOTIFICATION,
        (recipient: HANDLE, flags: u32) -> HANDLE {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &USER32_W, false, b"RegisterSuspendResumeNotification\0")
            } {
                let func: unsafe extern "system" fn(HANDLE, u32) -> HANDLE =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(recipient, flags) }
            } else {
                0
            }
        }
    );

    define_shim!(
        UnregisterSuspendResumeNotification,
        "__imp_UnregisterSuspendResumeNotification",
        IMP_UNREGISTER_SUSPEND_RESUME_NOTIFICATION,
        (handle: HANDLE) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &USER32_W, false, b"UnregisterSuspendResumeNotification\0")
            } {
                let func: unsafe extern "system" fn(HANDLE) -> BOOL =
                    unsafe { core::mem::transmute(addr) };
                unsafe { func(handle) }
            } else {
                1
            }
        }
    );

    define_shim!(
        LocaleNameToLCID,
        "__imp_LocaleNameToLCID",
        IMP_LOCALE_NAME_TO_LCID,
        (name: *const u16, flags: u32) -> u32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = (unsafe {
                resolve_cached(&SLOT, &KERNEL32_W, false, b"LocaleNameToLCID\0")
            }) else {
                return 0;
            };
            let func: unsafe extern "system" fn(*const u16, u32) -> u32 =
                unsafe { core::mem::transmute(addr) };
            let lcid = unsafe { func(name, flags) };
            if lcid == 0 && flags != 0 {
                unsafe { func(name, 0) }
            } else {
                lcid
            }
        }
    );

    #[repr(C)]
    struct StaticUnknown {
        vtbl: *const IUnknown_Vtbl,
    }
    unsafe impl Sync for StaticUnknown {}

    unsafe extern "system" fn static_unk_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        if !iid.is_null() && unsafe { *iid } == IID_IUNKNOWN {
            unsafe { *out = this };
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }
    pub(super) unsafe extern "system" fn static_unk_add_ref(_: *mut c_void) -> u32 {
        1
    }
    pub(super) unsafe extern "system" fn static_unk_release(_: *mut c_void) -> u32 {
        1
    }

    static STATIC_UNKNOWN_VTBL: IUnknown_Vtbl = IUnknown_Vtbl {
        QueryInterface: static_unk_qi,
        AddRef: static_unk_add_ref,
        Release: static_unk_release,
    };
    static NOT_SUPPORTED_SENTINEL: StaticUnknown = StaticUnknown {
        vtbl: &STATIC_UNKNOWN_VTBL,
    };
    static MIXED_ATTR_SENTINEL: StaticUnknown = StaticUnknown {
        vtbl: &STATIC_UNKNOWN_VTBL,
    };

    define_shim!(
        UiaGetReservedNotSupportedValue,
        "__imp_UiaGetReservedNotSupportedValue",
        IMP_UIA_GET_RESERVED_NOT_SUPPORTED_VALUE,
        (out: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &UIA_W, true, b"UiaGetReservedNotSupportedValue\0")
            } {
                let func: unsafe extern "system" fn(*mut *mut c_void) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(out) };
            }
            if out.is_null() {
                return E_POINTER;
            }
            unsafe { *out = ptr::addr_of!(NOT_SUPPORTED_SENTINEL).cast_mut().cast() };
            S_OK
        }
    );

    define_shim!(
        UiaGetReservedMixedAttributeValue,
        "__imp_UiaGetReservedMixedAttributeValue",
        IMP_UIA_GET_RESERVED_MIXED_ATTRIBUTE_VALUE,
        (out: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &UIA_W, true, b"UiaGetReservedMixedAttributeValue\0")
            } {
                let func: unsafe extern "system" fn(*mut *mut c_void) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(out) };
            }
            if out.is_null() {
                return E_POINTER;
            }
            unsafe { *out = ptr::addr_of!(MIXED_ATTR_SENTINEL).cast_mut().cast() };
            S_OK
        }
    );

    define_shim!(
        DCompositionCreateDevice,
        "__imp_DCompositionCreateDevice",
        IMP_DCOMPOSITION_CREATE_DEVICE,
        (dxgi_device: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &DCOMP_W, true, b"DCompositionCreateDevice\0")
            } {
                let func: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(dxgi_device, iid, out) };
            }
            if !out.is_null() {
                unsafe { *out = ptr::null_mut() };
            }
            E_NOINTERFACE
        }
    );

    define_shim!(
        DXGIGetDebugInterface1,
        "__imp_DXGIGetDebugInterface1",
        IMP_DXGI_GET_DEBUG_INTERFACE_1,
        (flags: u32, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = unsafe {
                resolve_cached(&SLOT, &DXGI_W, true, b"DXGIGetDebugInterface1\0")
            } {
                let func: unsafe extern "system" fn(u32, *const GUID, *mut *mut c_void) -> HRESULT =
                    unsafe { core::mem::transmute(addr) };
                return unsafe { func(flags, iid, out) };
            }
            if !out.is_null() {
                unsafe { *out = ptr::null_mut() };
            }
            E_NOINTERFACE
        }
    );

    #[repr(C)]
    pub(super) struct StaticCom<V: 'static> {
        pub(super) vtbl: &'static V,
    }
    unsafe impl<V: Sync + 'static> Sync for StaticCom<V> {}

    unsafe extern "system" fn dm_mgr_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN || g == IID_IDIRECT_MANIPULATION_MANAGER {
            unsafe { *out = this };
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }
    unsafe extern "system" fn dm_upd_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN || g == IID_IDIRECT_MANIPULATION_UPDATE_MANAGER {
            unsafe { *out = this };
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }
    unsafe extern "system" fn dm_vp_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN || g == IID_IDIRECT_MANIPULATION_VIEWPORT {
            unsafe { *out = this };
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }
    unsafe extern "system" fn dm_ok_hwnd(_: *mut c_void, _: HWND) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_reg_ht(_: *mut c_void, _: HWND, _: HWND, _: i32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_proc_in(
        _: *mut c_void,
        _: *const c_void,
        handled: *mut BOOL,
    ) -> HRESULT {
        if !handled.is_null() {
            unsafe { *handled = 0 };
        }
        S_OK
    }
    unsafe extern "system" fn dm_get_upd(
        _: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            dm_upd_qi(
                ptr::addr_of!(NOOP_DM_UPDATE_MANAGER).cast_mut().cast(),
                iid,
                out,
            )
        }
    }
    unsafe extern "system" fn dm_create_vp(
        _: *mut c_void,
        _: *mut c_void,
        _: HWND,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            dm_vp_qi(
                ptr::addr_of!(NOOP_DM_VIEWPORT).cast_mut().cast(),
                iid,
                out,
            )
        }
    }
    unsafe extern "system" fn dm_create_content(
        _: *mut c_void,
        _: *mut c_void,
        _: *const GUID,
        _: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn dm_upd_reg(
        _: *mut c_void,
        _: HANDLE,
        _: *mut c_void,
        cookie: *mut u32,
    ) -> HRESULT {
        if !cookie.is_null() {
            unsafe { *cookie = 1 };
        }
        S_OK
    }
    unsafe extern "system" fn dm_upd_unreg(_: *mut c_void, _: u32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_upd_update(_: *mut c_void, _: *mut c_void) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_ok0(_: *mut c_void) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_u32(_: *mut c_void, _: u32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_i32(_: *mut c_void, _: i32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_ptr(_: *mut c_void, _: *mut c_void) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_status(_: *mut c_void, status: *mut i32) -> HRESULT {
        if !status.is_null() {
            unsafe { *status = DIRECTMANIPULATION_READY };
        }
        S_OK
    }
    unsafe extern "system" fn dm_vp_get_tag(
        _: *mut c_void,
        _: *const GUID,
        obj: *mut *mut c_void,
        id: *mut u32,
    ) -> HRESULT {
        if !obj.is_null() {
            unsafe { *obj = ptr::null_mut() };
        }
        if !id.is_null() {
            unsafe { *id = 0 };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn dm_vp_set_tag(_: *mut c_void, _: *mut c_void, _: u32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_get_rect(_: *mut c_void, rect: *mut RECT) -> HRESULT {
        if !rect.is_null() {
            unsafe { *rect = RECT::default() };
        }
        S_OK
    }
    unsafe extern "system" fn dm_vp_set_rect(_: *mut c_void, _: *const RECT) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_zoom(
        _: *mut c_void,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: BOOL,
    ) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_xform(_: *mut c_void, _: *const f32, _: u32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn dm_vp_primary(
        _: *mut c_void,
        _: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn dm_vp_add_eh(
        _: *mut c_void,
        _: HWND,
        _: *mut c_void,
        cookie: *mut u32,
    ) -> HRESULT {
        if !cookie.is_null() {
            unsafe { *cookie = 1 };
        }
        S_OK
    }

    static NOOP_DM_MANAGER_VTBL: IDirectManipulationManager_Vtbl =
        IDirectManipulationManager_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: dm_mgr_qi,
                AddRef: static_unk_add_ref,
                Release: static_unk_release,
            },
            Activate: dm_ok_hwnd,
            Deactivate: dm_ok_hwnd,
            RegisterHitTestTarget: dm_reg_ht,
            ProcessInput: dm_proc_in,
            GetUpdateManager: dm_get_upd,
            CreateViewport: dm_create_vp,
            CreateContent: dm_create_content,
        };

    static NOOP_DM_UPDATE_MANAGER_VTBL: IDirectManipulationUpdateManager_Vtbl =
        IDirectManipulationUpdateManager_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: dm_upd_qi,
                AddRef: static_unk_add_ref,
                Release: static_unk_release,
            },
            RegisterWaitHandleCallback: dm_upd_reg,
            UnregisterWaitHandleCallback: dm_upd_unreg,
            Update: dm_upd_update,
        };

    static NOOP_DM_VIEWPORT_VTBL: IDirectManipulationViewport_Vtbl =
        IDirectManipulationViewport_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: dm_vp_qi,
                AddRef: static_unk_add_ref,
                Release: static_unk_release,
            },
            Enable: dm_vp_ok0,
            Disable: dm_vp_ok0,
            SetContact: dm_vp_u32,
            ReleaseContact: dm_vp_u32,
            ReleaseAllContacts: dm_vp_ok0,
            GetStatus: dm_vp_status,
            GetTag: dm_vp_get_tag,
            SetTag: dm_vp_set_tag,
            GetViewportRect: dm_vp_get_rect,
            SetViewportRect: dm_vp_set_rect,
            ZoomToRect: dm_vp_zoom,
            SetViewportTransform: dm_vp_xform,
            SyncDisplayTransform: dm_vp_xform,
            GetPrimaryContent: dm_vp_primary,
            AddContent: dm_vp_ptr,
            RemoveContent: dm_vp_ptr,
            SetViewportOptions: dm_vp_i32,
            AddConfiguration: dm_vp_i32,
            RemoveConfiguration: dm_vp_i32,
            ActivateConfiguration: dm_vp_i32,
            SetManualGesture: dm_vp_i32,
            SetChaining: dm_vp_i32,
            AddEventHandler: dm_vp_add_eh,
            RemoveEventHandler: dm_vp_u32,
            SetInputMode: dm_vp_i32,
            SetUpdateMode: dm_vp_i32,
            Stop: dm_vp_ok0,
            Abandon: dm_vp_ok0,
        };

    static NOOP_DM_MANAGER: StaticCom<IDirectManipulationManager_Vtbl> = StaticCom {
        vtbl: &NOOP_DM_MANAGER_VTBL,
    };
    static NOOP_DM_UPDATE_MANAGER: StaticCom<IDirectManipulationUpdateManager_Vtbl> = StaticCom {
        vtbl: &NOOP_DM_UPDATE_MANAGER_VTBL,
    };
    static NOOP_DM_VIEWPORT: StaticCom<IDirectManipulationViewport_Vtbl> = StaticCom {
        vtbl: &NOOP_DM_VIEWPORT_VTBL,
    };

    define_shim!(
        CoCreateInstance,
        "__imp_CoCreateInstance",
        IMP_CO_CREATE_INSTANCE,
        (
            rclsid: *const GUID,
            punk_outer: *mut c_void,
            cls_context: u32,
            riid: *const GUID,
            ppv: *mut *mut c_void,
        ) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = (unsafe {
                resolve_cached(&SLOT, &OLE32_W, true, b"CoCreateInstance\0")
            }) else {
                return REGDB_E_CLASSNOTREG;
            };
            let func: unsafe extern "system" fn(
                *const GUID,
                *mut c_void,
                u32,
                *const GUID,
                *mut *mut c_void,
            ) -> HRESULT = unsafe { core::mem::transmute(addr) };
            let hr = unsafe { func(rclsid, punk_outer, cls_context, riid, ppv) };
            if hr >= 0 {
                return hr;
            }
            if !rclsid.is_null()
                && !riid.is_null()
                && unsafe { *rclsid } == CLSID_DIRECT_MANIPULATION_MANAGER
            {
                return unsafe {
                    dm_mgr_qi(ptr::addr_of!(NOOP_DM_MANAGER).cast_mut().cast(), riid, ppv)
                };
            }
            hr
        }
    );

    static SYNTHETIC_WARP_ADAPTER: AtomicUsize = AtomicUsize::new(0);

    type D3D11CreateDeviceFn = unsafe extern "system" fn(
        *mut c_void,
        i32,
        HMODULE,
        u32,
        *const u32,
        u32,
        u32,
        *mut *mut c_void,
        *mut u32,
        *mut *mut c_void,
    ) -> HRESULT;

    unsafe fn raw_d3d11_create_device() -> Option<D3D11CreateDeviceFn> {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        unsafe {
            resolve_cached(&SLOT, &D3D11_W, true, b"D3D11CreateDevice\0")
                .map(|addr| core::mem::transmute(addr))
        }
    }

    define_shim!(
        D3D11CreateDevice,
        "__imp_D3D11CreateDevice",
        IMP_D3D11_CREATE_DEVICE,
        (
            adapter: *mut c_void,
            driver_type: i32,
            software: HMODULE,
            flags: u32,
            feature_levels: *const u32,
            num_feature_levels: u32,
            sdk_version: u32,
            device: *mut *mut c_void,
            feature_level: *mut u32,
            immediate_context: *mut *mut c_void,
        ) -> HRESULT {
            let Some(func) = (unsafe { raw_d3d11_create_device() }) else {
                return E_NOINTERFACE;
            };
            let warp_ptr = SYNTHETIC_WARP_ADAPTER.load(Ordering::Acquire);
            let (actual_adapter, actual_driver) =
                if !adapter.is_null() && warp_ptr != 0 && adapter as usize == warp_ptr {
                    (ptr::null_mut(), D3D_DRIVER_TYPE_WARP)
                } else {
                    (adapter, driver_type)
                };
            let hr = unsafe {
                func(
                    actual_adapter,
                    actual_driver,
                    software,
                    flags,
                    feature_levels,
                    num_feature_levels,
                    sdk_version,
                    device,
                    feature_level,
                    immediate_context,
                )
            };
            if hr != E_INVALIDARG || feature_levels.is_null() || num_feature_levels == 0 {
                return hr;
            }
            let slice =
                unsafe { core::slice::from_raw_parts(feature_levels, num_feature_levels as usize) };
            let filtered: Vec<u32> = slice
                .iter()
                .copied()
                .filter(|&lvl| lvl <= D3D_FEATURE_LEVEL_11_0)
                .collect();
            if filtered.is_empty() || filtered.len() == slice.len() {
                return hr;
            }
            unsafe {
                func(
                    actual_adapter,
                    actual_driver,
                    software,
                    flags,
                    filtered.as_ptr(),
                    filtered.len() as u32,
                    sdk_version,
                    device,
                    feature_level,
                    immediate_context,
                )
            }
        }
    );

    unsafe fn synthesize_warp_adapter() -> Option<*mut c_void> {
        let func = unsafe { raw_d3d11_create_device()? };
        let levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1];
        let mut device = ptr::null_mut();
        let mut chosen = 0u32;
        let hr = unsafe {
            func(
                ptr::null_mut(),
                D3D_DRIVER_TYPE_WARP,
                0,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                levels.as_ptr(),
                levels.len() as u32,
                D3D11_SDK_VERSION,
                &mut device,
                &mut chosen,
                ptr::null_mut(),
            )
        };
        if hr < 0 || device.is_null() {
            return None;
        }
        let dxgi_dev = unsafe { com_qi(device, &IID_IDXGI_DEVICE) };
        unsafe { com_release(device) };
        let dxgi_dev = dxgi_dev?;
        let mut raw_adapter = ptr::null_mut();
        let hr = unsafe {
            ((*com_vtbl::<IDXGIDevice_Vtbl>(dxgi_dev)).GetAdapter)(dxgi_dev, &mut raw_adapter)
        };
        unsafe { com_release(dxgi_dev) };
        if hr < 0 || raw_adapter.is_null() {
            return None;
        }
        let adapter1 = unsafe { com_qi(raw_adapter, &IID_IDXGI_ADAPTER1) };
        unsafe { com_release(raw_adapter) };
        let adapter1 = adapter1?;
        SYNTHETIC_WARP_ADAPTER.store(adapter1 as usize, Ordering::Release);
        Some(adapter1)
    }

    #[repr(C)]
    struct Win7DxgiSwapChain1 {
        vtbl: *const IDXGISwapChain1_Vtbl,
        refs: AtomicU32,
        raw: *mut c_void,
        hwnd: HWND,
    }

    unsafe extern "system" fn sc1_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN
            || g == IID_IDXGI_OBJECT
            || g == IID_IDXGI_DEVICE_SUB_OBJECT
            || g == IID_IDXGI_SWAP_CHAIN
            || g == IID_IDXGI_SWAP_CHAIN1
        {
            unsafe { sc1_add_ref(this) };
            unsafe { *out = this };
            return S_OK;
        }
        let me = unsafe { &*(this as *const Win7DxgiSwapChain1) };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(me.raw)).QueryInterface)(me.raw, iid, out) }
    }
    unsafe extern "system" fn sc1_add_ref(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7DxgiSwapChain1) };
        me.refs.fetch_add(1, Ordering::Relaxed) + 1
    }
    unsafe extern "system" fn sc1_release(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7DxgiSwapChain1) };
        let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let boxed = unsafe { Box::from_raw(this as *mut Win7DxgiSwapChain1) };
            unsafe { com_release(boxed.raw) };
            0
        } else {
            prev - 1
        }
    }
    unsafe extern "system" fn sc1_set_priv(
        this: *mut c_void,
        name: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateData)(raw, name, size, data) }
    }
    unsafe extern "system" fn sc1_set_priv_if(
        this: *mut c_void,
        name: *const GUID,
        unk: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateDataInterface)(raw, name, unk) }
    }
    unsafe extern "system" fn sc1_get_priv(
        this: *mut c_void,
        name: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).GetPrivateData)(raw, name, size, data) }
    }
    unsafe extern "system" fn sc1_get_parent(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).GetParent)(raw, iid, out) }
    }
    unsafe extern "system" fn sc1_get_device(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGIDeviceSubObject_Vtbl>(raw)).GetDevice)(raw, iid, out) }
    }
    unsafe extern "system" fn sc1_present(this: *mut c_void, sync: u32, flags: u32) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).Present)(raw, sync, flags) }
    }
    unsafe extern "system" fn sc1_get_buffer(
        this: *mut c_void,
        buf: u32,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetBuffer)(raw, buf, iid, out) }
    }
    unsafe extern "system" fn sc1_set_fs(
        this: *mut c_void,
        fs: BOOL,
        target: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).SetFullscreenState)(raw, fs, target) }
    }
    unsafe extern "system" fn sc1_get_fs(
        this: *mut c_void,
        fs: *mut BOOL,
        target: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetFullscreenState)(raw, fs, target) }
    }
    unsafe extern "system" fn sc1_get_desc(
        this: *mut c_void,
        desc: *mut DXGI_SWAP_CHAIN_DESC,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetDesc)(raw, desc) }
    }
    unsafe extern "system" fn sc1_resize_buffers(
        this: *mut c_void,
        _count: u32,
        w: u32,
        h: u32,
        fmt: u32,
        flags: u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).ResizeBuffers)(raw, 0, w, h, fmt, flags) }
    }
    unsafe extern "system" fn sc1_resize_target(
        this: *mut c_void,
        params: *const DXGI_MODE_DESC,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).ResizeTarget)(raw, params) }
    }
    unsafe extern "system" fn sc1_get_output(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetContainingOutput)(raw, out) }
    }
    unsafe extern "system" fn sc1_get_stats(this: *mut c_void, stats: *mut c_void) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetFrameStatistics)(raw, stats) }
    }
    unsafe extern "system" fn sc1_get_last_present(this: *mut c_void, cnt: *mut u32) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiSwapChain1)).raw };
        unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).GetLastPresentCount)(raw, cnt) }
    }
    unsafe extern "system" fn sc1_get_desc1(
        this: *mut c_void,
        out: *mut DXGI_SWAP_CHAIN_DESC1,
    ) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        let mut legacy = DXGI_SWAP_CHAIN_DESC::default();
        let hr = unsafe { sc1_get_desc(this, &mut legacy) };
        if hr < 0 {
            return hr;
        }
        unsafe {
            *out = DXGI_SWAP_CHAIN_DESC1 {
                Width: legacy.BufferDesc.Width,
                Height: legacy.BufferDesc.Height,
                Format: legacy.BufferDesc.Format,
                Stereo: 0,
                SampleDesc: legacy.SampleDesc,
                BufferUsage: legacy.BufferUsage,
                BufferCount: legacy.BufferCount,
                Scaling: 0,
                SwapEffect: legacy.SwapEffect,
                AlphaMode: 0,
                Flags: legacy.Flags,
            };
        }
        S_OK
    }
    unsafe extern "system" fn sc1_notimpl_ptr(_: *mut c_void, _: *mut c_void) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn sc1_get_hwnd(this: *mut c_void, out: *mut HWND) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        unsafe { *out = (*(this as *const Win7DxgiSwapChain1)).hwnd };
        S_OK
    }
    unsafe extern "system" fn sc1_get_core_win(
        _: *mut c_void,
        _: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOINTERFACE
    }
    unsafe extern "system" fn sc1_present1(
        this: *mut c_void,
        sync: u32,
        flags: u32,
        _: *const c_void,
    ) -> HRESULT {
        unsafe { sc1_present(this, sync, flags) }
    }
    unsafe extern "system" fn sc1_is_mono(_: *mut c_void) -> BOOL {
        0
    }
    unsafe extern "system" fn sc1_get_restrict(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn sc1_set_bg(_: *mut c_void, _: *const DXGI_RGBA) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn sc1_get_bg(_: *mut c_void, out: *mut DXGI_RGBA) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = DXGI_RGBA::default() };
        }
        S_OK
    }
    unsafe extern "system" fn sc1_set_rot(_: *mut c_void, _: i32) -> HRESULT {
        S_OK
    }
    unsafe extern "system" fn sc1_get_rot(_: *mut c_void, out: *mut i32) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = 1 };
        }
        S_OK
    }

    static SWAP_CHAIN1_VTBL: IDXGISwapChain1_Vtbl = IDXGISwapChain1_Vtbl {
        base: IDXGISwapChain_Vtbl {
            base: IDXGIDeviceSubObject_Vtbl {
                base: IDXGIObject_Vtbl {
                    base: IUnknown_Vtbl {
                        QueryInterface: sc1_qi,
                        AddRef: sc1_add_ref,
                        Release: sc1_release,
                    },
                    SetPrivateData: sc1_set_priv,
                    SetPrivateDataInterface: sc1_set_priv_if,
                    GetPrivateData: sc1_get_priv,
                    GetParent: sc1_get_parent,
                },
                GetDevice: sc1_get_device,
            },
            Present: sc1_present,
            GetBuffer: sc1_get_buffer,
            SetFullscreenState: sc1_set_fs,
            GetFullscreenState: sc1_get_fs,
            GetDesc: sc1_get_desc,
            ResizeBuffers: sc1_resize_buffers,
            ResizeTarget: sc1_resize_target,
            GetContainingOutput: sc1_get_output,
            GetFrameStatistics: sc1_get_stats,
            GetLastPresentCount: sc1_get_last_present,
        },
        GetDesc1: sc1_get_desc1,
        GetFullscreenDesc: sc1_notimpl_ptr,
        GetHwnd: sc1_get_hwnd,
        GetCoreWindow: sc1_get_core_win,
        Present1: sc1_present1,
        IsTemporaryMonoSupported: sc1_is_mono,
        GetRestrictToOutput: sc1_get_restrict,
        SetBackgroundColor: sc1_set_bg,
        GetBackgroundColor: sc1_get_bg,
        SetRotation: sc1_set_rot,
        GetRotation: sc1_get_rot,
    };

    #[repr(C)]
    struct Win7DxgiFactory6 {
        vtbl: *const IDXGIFactory6_Vtbl,
        refs: AtomicU32,
        raw: *mut c_void,
        warp_slot: AtomicU32,
    }

    unsafe extern "system" fn f6_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN
            || g == IID_IDXGI_OBJECT
            || g == IID_IDXGI_FACTORY
            || g == IID_IDXGI_FACTORY1
            || g == IID_IDXGI_FACTORY2
            || g == IID_IDXGI_FACTORY3
            || g == IID_IDXGI_FACTORY4
            || g == IID_IDXGI_FACTORY5
            || g == IID_IDXGI_FACTORY6
        {
            unsafe { f6_add_ref(this) };
            unsafe { *out = this };
            return S_OK;
        }
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(me.raw)).QueryInterface)(me.raw, iid, out) }
    }
    unsafe extern "system" fn f6_add_ref(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        me.refs.fetch_add(1, Ordering::Relaxed) + 1
    }
    unsafe extern "system" fn f6_release(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let boxed = unsafe { Box::from_raw(this as *mut Win7DxgiFactory6) };
            unsafe { com_release(boxed.raw) };
            0
        } else {
            prev - 1
        }
    }
    unsafe extern "system" fn f6_set_priv(
        this: *mut c_void,
        name: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateData)(raw, name, size, data) }
    }
    unsafe extern "system" fn f6_set_priv_if(
        this: *mut c_void,
        name: *const GUID,
        unk: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateDataInterface)(raw, name, unk) }
    }
    unsafe extern "system" fn f6_get_priv(
        this: *mut c_void,
        name: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).GetPrivateData)(raw, name, size, data) }
    }
    unsafe extern "system" fn f6_get_parent(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).GetParent)(raw, iid, out) }
    }
    unsafe extern "system" fn f6_enum_adapters(
        this: *mut c_void,
        index: u32,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        let hr = unsafe {
            ((*com_vtbl::<IDXGIFactory_Vtbl>(me.raw)).EnumAdapters)(me.raw, index, out)
        };
        if hr >= 0 {
            return hr;
        }
        let slot = me.warp_slot.load(Ordering::Acquire);
        if hr == DXGI_ERROR_NOT_FOUND && (slot == u32::MAX || slot == index) {
            me.warp_slot.store(index, Ordering::Release);
            if let Some(warp_adapter) = unsafe { synthesize_warp_adapter() } {
                unsafe { *out = warp_adapter };
                return S_OK;
            }
        }
        hr
    }
    unsafe extern "system" fn f6_make_win_assoc(
        this: *mut c_void,
        hwnd: HWND,
        flags: u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory_Vtbl>(raw)).MakeWindowAssociation)(raw, hwnd, flags) }
    }
    unsafe extern "system" fn f6_get_win_assoc(this: *mut c_void, hwnd: *mut HWND) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory_Vtbl>(raw)).GetWindowAssociation)(raw, hwnd) }
    }
    unsafe extern "system" fn f6_create_sc(
        this: *mut c_void,
        device: *mut c_void,
        desc: *const DXGI_SWAP_CHAIN_DESC,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory_Vtbl>(raw)).CreateSwapChain)(raw, device, desc, out) }
    }
    unsafe extern "system" fn f6_create_sw(
        this: *mut c_void,
        module: HMODULE,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory_Vtbl>(raw)).CreateSoftwareAdapter)(raw, module, out) }
    }
    unsafe extern "system" fn f6_enum_adapters1(
        this: *mut c_void,
        index: u32,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory1_Vtbl>(raw)).EnumAdapters1)(raw, index, out) }
    }
    unsafe extern "system" fn f6_is_current(this: *mut c_void) -> BOOL {
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        unsafe { ((*com_vtbl::<IDXGIFactory1_Vtbl>(raw)).IsCurrent)(raw) }
    }
    unsafe extern "system" fn f6_stereo_enabled(_: *mut c_void) -> BOOL {
        0
    }
    unsafe extern "system" fn f6_create_sc_for_hwnd(
        this: *mut c_void,
        device: *mut c_void,
        hwnd: HWND,
        desc: *const DXGI_SWAP_CHAIN_DESC1,
        fullscreen_desc: *const c_void,
        restrict_output: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || desc.is_null() {
            return E_INVALIDARG;
        }
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        if has_dcomp() {
            if let Some(f2) = unsafe { com_qi(me.raw, &IID_IDXGI_FACTORY2) } {
                let hr = unsafe {
                    ((*com_vtbl::<IDXGIFactory2_Vtbl>(f2)).CreateSwapChainForHwnd)(
                        f2,
                        device,
                        hwnd,
                        desc,
                        fullscreen_desc,
                        restrict_output,
                        out,
                    )
                };
                unsafe { com_release(f2) };
                if hr >= 0 {
                    return hr;
                }
            }
        }
        let d = unsafe { *desc };
        let legacy = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: d.Width,
                Height: d.Height,
                RefreshRate: DXGI_RATIONAL {
                    Numerator: 0,
                    Denominator: 0,
                },
                Format: d.Format,
                ScanlineOrdering: 0,
                Scaling: 0,
            },
            SampleDesc: d.SampleDesc,
            BufferUsage: d.BufferUsage,
            BufferCount: 1,
            OutputWindow: hwnd,
            Windowed: 1,
            SwapEffect: 0,
            Flags: d.Flags,
        };
        let mut raw_sc = ptr::null_mut();
        let hr = unsafe {
            ((*com_vtbl::<IDXGIFactory_Vtbl>(me.raw)).CreateSwapChain)(
                me.raw,
                device,
                &legacy,
                &mut raw_sc,
            )
        };
        if hr < 0 || raw_sc.is_null() {
            unsafe { *out = ptr::null_mut() };
            return hr;
        }
        let wrapped = Box::into_raw(Box::new(Win7DxgiSwapChain1 {
            vtbl: &SWAP_CHAIN1_VTBL,
            refs: AtomicU32::new(1),
            raw: raw_sc,
            hwnd,
        }));
        unsafe { *out = wrapped.cast() };
        S_OK
    }
    unsafe extern "system" fn f6_create_sc_core(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: *const DXGI_SWAP_CHAIN_DESC1,
        _: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_shared_luid(
        _: *mut c_void,
        _: HANDLE,
        _: *mut LUID,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_reg_win(
        _: *mut c_void,
        _: HWND,
        _: u32,
        _: *mut u32,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_reg_evt(
        _: *mut c_void,
        _: HANDLE,
        _: *mut u32,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_unreg(_: *mut c_void, _: u32) {}
    unsafe extern "system" fn f6_create_sc_comp(
        _: *mut c_void,
        _: *mut c_void,
        _: *const DXGI_SWAP_CHAIN_DESC1,
        _: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_creation_flags(_: *mut c_void) -> u32 {
        0
    }
    unsafe extern "system" fn f6_enum_luid(
        _: *mut c_void,
        _: LUID,
        _: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_enum_warp(
        _: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || iid.is_null() {
            return E_INVALIDARG;
        }
        if let Some(warp) = unsafe { synthesize_warp_adapter() } {
            let hr = unsafe { ((*com_vtbl::<IUnknown_Vtbl>(warp)).QueryInterface)(warp, iid, out) };
            unsafe { com_release(warp) };
            hr
        } else {
            unsafe { *out = ptr::null_mut() };
            DXGI_ERROR_NOT_FOUND
        }
    }
    unsafe extern "system" fn f6_check_feat(
        _: *mut c_void,
        _: i32,
        _: *mut c_void,
        _: u32,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn f6_enum_pref(
        this: *mut c_void,
        index: u32,
        _pref: i32,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || iid.is_null() {
            return E_INVALIDARG;
        }
        let mut adapter = ptr::null_mut();
        let hr = unsafe { f6_enum_adapters(this, index, &mut adapter) };
        if hr < 0 || adapter.is_null() {
            unsafe { *out = ptr::null_mut() };
            return hr;
        }
        let qi_hr =
            unsafe { ((*com_vtbl::<IUnknown_Vtbl>(adapter)).QueryInterface)(adapter, iid, out) };
        unsafe { com_release(adapter) };
        qi_hr
    }

    static FACTORY6_VTBL: IDXGIFactory6_Vtbl = IDXGIFactory6_Vtbl {
        base: IDXGIFactory5_Vtbl {
            base: IDXGIFactory4_Vtbl {
                base: IDXGIFactory3_Vtbl {
                    base: IDXGIFactory2_Vtbl {
                        base: IDXGIFactory1_Vtbl {
                            base: IDXGIFactory_Vtbl {
                                base: IDXGIObject_Vtbl {
                                    base: IUnknown_Vtbl {
                                        QueryInterface: f6_qi,
                                        AddRef: f6_add_ref,
                                        Release: f6_release,
                                    },
                                    SetPrivateData: f6_set_priv,
                                    SetPrivateDataInterface: f6_set_priv_if,
                                    GetPrivateData: f6_get_priv,
                                    GetParent: f6_get_parent,
                                },
                                EnumAdapters: f6_enum_adapters,
                                MakeWindowAssociation: f6_make_win_assoc,
                                GetWindowAssociation: f6_get_win_assoc,
                                CreateSwapChain: f6_create_sc,
                                CreateSoftwareAdapter: f6_create_sw,
                            },
                            EnumAdapters1: f6_enum_adapters1,
                            IsCurrent: f6_is_current,
                        },
                        IsWindowedStereoEnabled: f6_stereo_enabled,
                        CreateSwapChainForHwnd: f6_create_sc_for_hwnd,
                        CreateSwapChainForCoreWindow: f6_create_sc_core,
                        GetSharedResourceAdapterLuid: f6_shared_luid,
                        RegisterStereoStatusWindow: f6_reg_win,
                        RegisterStereoStatusEvent: f6_reg_evt,
                        UnregisterStereoStatus: f6_unreg,
                        RegisterOcclusionStatusWindow: f6_reg_win,
                        RegisterOcclusionStatusEvent: f6_reg_evt,
                        UnregisterOcclusionStatus: f6_unreg,
                        CreateSwapChainForComposition: f6_create_sc_comp,
                    },
                    GetCreationFlags: f6_creation_flags,
                },
                EnumAdapterByLuid: f6_enum_luid,
                EnumWarpAdapter: f6_enum_warp,
            },
            CheckFeatureSupport: f6_check_feat,
        },
        EnumAdapterByGpuPreference: f6_enum_pref,
    };

    define_shim!(
        CreateDXGIFactory2,
        "__imp_CreateDXGIFactory2",
        IMP_CREATE_DXGI_FACTORY_2,
        (flags: u32, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
            if out.is_null() || iid.is_null() {
                return E_INVALIDARG;
            }
            if has_dcomp() {
                static SLOT2: AtomicUsize = AtomicUsize::new(UNRESOLVED);
                if let Some(addr) = unsafe {
                    resolve_cached(&SLOT2, &DXGI_W, true, b"CreateDXGIFactory2\0")
                } {
                    let func: unsafe extern "system" fn(u32, *const GUID, *mut *mut c_void) -> HRESULT =
                        unsafe { core::mem::transmute(addr) };
                    let hr = unsafe { func(flags, iid, out) };
                    if hr >= 0 {
                        return hr;
                    }
                }
            }
            static SLOT1: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr1) = (unsafe {
                resolve_cached(&SLOT1, &DXGI_W, true, b"CreateDXGIFactory1\0")
            }) else {
                unsafe { *out = ptr::null_mut() };
                return E_NOINTERFACE;
            };
            let create1: unsafe extern "system" fn(*const GUID, *mut *mut c_void) -> HRESULT =
                unsafe { core::mem::transmute(addr1) };
            let mut raw_f1 = ptr::null_mut();
            let hr = unsafe { create1(&IID_IDXGI_FACTORY1, &mut raw_f1) };
            if hr < 0 || raw_f1.is_null() {
                unsafe { *out = ptr::null_mut() };
                return hr;
            }
            let wrapped = Box::into_raw(Box::new(Win7DxgiFactory6 {
                vtbl: &FACTORY6_VTBL,
                refs: AtomicU32::new(1),
                raw: raw_f1,
                warp_slot: AtomicU32::new(u32::MAX),
            }));
            let qi_hr = unsafe { f6_qi(wrapped.cast(), iid, out) };
            unsafe { f6_release(wrapped.cast()) };
            qi_hr
        }
    );

    pub fn init() {
        let refs: [usize; 31] = [
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
            ptr::addr_of!(IMP_RO_GET_AGILE_REFERENCE) as usize,
            ptr::addr_of!(IMP_CO_INCREMENT_MTA_USAGE) as usize,
            ptr::addr_of!(IMP_U_STRLEN) as usize,
            ptr::addr_of!(IMP_GET_CURRENT_PACKAGE_FULL_NAME) as usize,
            ptr::addr_of!(IMP_GET_DPI_FOR_MONITOR) as usize,
            ptr::addr_of!(IMP_GET_DPI_FOR_WINDOW) as usize,
            ptr::addr_of!(IMP_GET_SYSTEM_METRICS_FOR_DPI) as usize,
            ptr::addr_of!(IMP_GET_POINTER_TYPE) as usize,
            ptr::addr_of!(IMP_REGISTER_SUSPEND_RESUME_NOTIFICATION) as usize,
            ptr::addr_of!(IMP_UNREGISTER_SUSPEND_RESUME_NOTIFICATION) as usize,
            ptr::addr_of!(IMP_LOCALE_NAME_TO_LCID) as usize,
            ptr::addr_of!(IMP_UIA_GET_RESERVED_NOT_SUPPORTED_VALUE) as usize,
            ptr::addr_of!(IMP_UIA_GET_RESERVED_MIXED_ATTRIBUTE_VALUE) as usize,
            ptr::addr_of!(IMP_DCOMPOSITION_CREATE_DEVICE) as usize,
            ptr::addr_of!(IMP_DXGI_GET_DEBUG_INTERFACE_1) as usize,
            ptr::addr_of!(IMP_CO_CREATE_INSTANCE) as usize,
            ptr::addr_of!(IMP_D3D11_CREATE_DEVICE) as usize,
            ptr::addr_of!(IMP_CREATE_DXGI_FACTORY_2) as usize ^ dwrite::shim_anchor(),
        ];
        core::hint::black_box(refs);
        if !has_dcomp() && std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none() {
            std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::com_abi::*;
    use super::{
        WAIT_ADDRESS_BUCKETS, WIN7_WRITE_CONSOLE_MAX_CHARS, address_bytes_equal, init,
        scale_metric_for_dpi, utf16_strlen, wait_bucket_index, write_console_chunk_len,
    };
    use core::ffi::c_void;
    use core::mem::size_of;
    use core::ptr;

    #[test]
    fn utf16_strlen_counts_until_nul_and_handles_null() {
        let empty = [0u16];
        let hello = [b'H' as u16, b'i' as u16, 0xD83D, 0xDE80, 0, b'!' as u16];
        unsafe {
            assert_eq!(utf16_strlen(ptr::null()), 0);
            assert_eq!(utf16_strlen(empty.as_ptr()), 0);
            assert_eq!(utf16_strlen(hello.as_ptr()), 4);
        }
    }

    #[test]
    fn dpi_scaling_and_console_chunking_behave_accurately() {
        assert_eq!(scale_metric_for_dpi(8, 96, 96), 8);
        assert_eq!(scale_metric_for_dpi(8, 144, 96), 12);
        assert_eq!(scale_metric_for_dpi(8, 192, 96), 16);
        assert_eq!(
            write_console_chunk_len(32_768, false),
            WIN7_WRITE_CONSOLE_MAX_CHARS
        );
        assert_eq!(write_console_chunk_len(1024, false), 1024);
        assert_eq!(write_console_chunk_len(32_768, true), 32_768);
    }

    #[test]
    fn wait_buckets_and_byte_equality_cover_all_widths() {
        for shift in 0..32 {
            assert!(wait_bucket_index(1usize << shift) < WAIT_ADDRESS_BUCKETS);
        }
        let x = 0xCAFE_BABEu32;
        let y = 0xCAFE_BABEu32;
        let z = 0xCAFE_BABFu32;
        unsafe {
            assert_eq!(
                address_bytes_equal(
                    ptr::from_ref(&x).cast::<c_void>(),
                    ptr::from_ref(&y).cast::<c_void>(),
                    4,
                ),
                Some(true)
            );
            assert_eq!(
                address_bytes_equal(
                    ptr::from_ref(&x).cast::<c_void>(),
                    ptr::from_ref(&z).cast::<c_void>(),
                    4,
                ),
                Some(false)
            );
            assert_eq!(
                address_bytes_equal(ptr::null(), ptr::from_ref(&x).cast::<c_void>(), 4),
                None
            );
        }
        init();
    }

    #[test]
    fn com_vtable_slot_counts_match_windows_sdk_definitions() {
        let word = size_of::<usize>();
        assert_eq!(size_of::<IUnknown_Vtbl>(), 3 * word);
        assert_eq!(size_of::<IDXGIObject_Vtbl>(), 7 * word);
        assert_eq!(size_of::<IDXGIDevice_Vtbl>(), 12 * word);
        assert_eq!(size_of::<IDXGIDeviceSubObject_Vtbl>(), 8 * word);
        assert_eq!(size_of::<IDXGIFactory_Vtbl>(), 12 * word);
        assert_eq!(size_of::<IDXGIFactory1_Vtbl>(), 14 * word);
        assert_eq!(size_of::<IDXGIFactory2_Vtbl>(), 25 * word);
        assert_eq!(size_of::<IDXGIFactory6_Vtbl>(), 30 * word);
        assert_eq!(size_of::<IDXGISwapChain_Vtbl>(), 18 * word);
        assert_eq!(size_of::<IDXGISwapChain1_Vtbl>(), 29 * word);
        assert_eq!(size_of::<IDWriteFactory_Vtbl>(), 24 * word);
        assert_eq!(size_of::<IDWriteFactory1_Vtbl>(), 26 * word);
        assert_eq!(size_of::<IDWriteFactory2_Vtbl>(), 31 * word);
        assert_eq!(size_of::<IDWriteFactory3_Vtbl>(), 40 * word);
        assert_eq!(size_of::<IDWriteFactory4_Vtbl>(), 43 * word);
        assert_eq!(size_of::<IDWriteFactory5_Vtbl>(), 48 * word);
        assert_eq!(size_of::<IDWriteFontCollection_Vtbl>(), 7 * word);
        assert_eq!(size_of::<IDWriteFontCollection1_Vtbl>(), 9 * word);
        assert_eq!(size_of::<IDWriteFontList_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDWriteFontFamily_Vtbl>(), 9 * word);
        assert_eq!(size_of::<IDWriteFontFamily1_Vtbl>(), 12 * word);
        assert_eq!(size_of::<IDWriteFont_Vtbl>(), 14 * word);
        assert_eq!(size_of::<IDWriteFontFace_Vtbl>(), 18 * word);
        assert_eq!(size_of::<IDWriteFontFace1_Vtbl>(), 30 * word);
        assert_eq!(size_of::<IDWriteFontFace2_Vtbl>(), 35 * word);
        assert_eq!(size_of::<IDWriteFontFace3_Vtbl>(), 49 * word);
        assert_eq!(size_of::<IDWriteFontFaceReference_Vtbl>(), 17 * word);
        assert_eq!(size_of::<IDWriteFontSet_Vtbl>(), 13 * word);
        assert_eq!(size_of::<IDWriteFontSetBuilder_Vtbl>(), 7 * word);
        assert_eq!(size_of::<IDWriteFontSetBuilder1_Vtbl>(), 8 * word);
        assert_eq!(size_of::<IDWriteFontFileLoader_Vtbl>(), 4 * word);
        assert_eq!(size_of::<IDWriteInMemoryFontFileLoader_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDWriteFontFileStream_Vtbl>(), 7 * word);
        assert_eq!(size_of::<IDWriteFontCollectionLoader_Vtbl>(), 4 * word);
        assert_eq!(size_of::<IDWriteFontFileEnumerator_Vtbl>(), 5 * word);
        assert_eq!(size_of::<IDWriteTextFormat_Vtbl>(), 28 * word);
        assert_eq!(size_of::<IDWriteTextFormat1_Vtbl>(), 36 * word);
        assert_eq!(size_of::<IDWriteTextLayout_Vtbl>(), 67 * word);
        assert_eq!(size_of::<IDWritePixelSnapping_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDWriteTextRenderer_Vtbl>(), 10 * word);
        assert_eq!(size_of::<IDWriteRenderingParams_Vtbl>(), 8 * word);
        assert_eq!(size_of::<IDWriteRenderingParams1_Vtbl>(), 9 * word);
        assert_eq!(size_of::<IDWriteGlyphRunAnalysis_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDWriteFontFallback_Vtbl>(), 4 * word);
        assert_eq!(size_of::<IDWriteFontFallbackBuilder_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDirectManipulationManager_Vtbl>(), 10 * word);
        assert_eq!(size_of::<IDirectManipulationUpdateManager_Vtbl>(), 6 * word);
        assert_eq!(size_of::<IDirectManipulationViewport_Vtbl>(), 31 * word);
    }
}
