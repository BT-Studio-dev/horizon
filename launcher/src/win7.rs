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
    #[derive(Clone, Copy)]
    pub struct DXGI_ADAPTER_DESC {
        pub Description: [u16; 128],
        pub VendorId: u32,
        pub DeviceId: u32,
        pub SubSysId: u32,
        pub Revision: u32,
        pub DedicatedVideoMemory: usize,
        pub DedicatedSystemMemory: usize,
        pub SharedSystemMemory: usize,
        pub AdapterLuid: LUID,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DXGI_ADAPTER_DESC1 {
        pub Description: [u16; 128],
        pub VendorId: u32,
        pub DeviceId: u32,
        pub SubSysId: u32,
        pub Revision: u32,
        pub DedicatedVideoMemory: usize,
        pub DedicatedSystemMemory: usize,
        pub SharedSystemMemory: usize,
        pub AdapterLuid: LUID,
        pub Flags: u32,
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
    #[derive(Clone, Copy)]
    pub struct D3D11_BUFFER_DESC {
        pub ByteWidth: u32,
        pub Usage: u32,
        pub BindFlags: u32,
        pub CPUAccessFlags: u32,
        pub MiscFlags: u32,
        pub StructureByteStride: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct D3D11_TEXTURE2D_DESC {
        pub Width: u32,
        pub Height: u32,
        pub MipLevels: u32,
        pub ArraySize: u32,
        pub Format: u32,
        pub SampleDesc: DXGI_SAMPLE_DESC,
        pub Usage: u32,
        pub BindFlags: u32,
        pub CPUAccessFlags: u32,
        pub MiscFlags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct D3D11_SHADER_RESOURCE_VIEW_DESC {
        pub Format: u32,
        pub ViewDimension: u32,
        pub u: [u32; 4],
    }

    /// Parses a DXBC container's `RDEF` resource table and returns the ASCII binding name of any
    /// `D3D_SIT_STRUCTURED` (`type == 5`) resource bound to `register(t1)` (`bind_point == 1`).
    pub fn dxbc_structured_t1_name(dxbc: &[u8]) -> Option<&[u8]> {
        if dxbc.len() < 32 || &dxbc[0..4] != b"DXBC" {
            return None;
        }
        let chunk_count = u32::from_le_bytes(dxbc[28..32].try_into().ok()?) as usize;
        let table_end = 32usize.checked_add(chunk_count.checked_mul(4)?)?;
        if table_end > dxbc.len() {
            return None;
        }
        for i in 0..chunk_count {
            let off_pos = 32 + i * 4;
            let chunk_off = u32::from_le_bytes(dxbc[off_pos..off_pos + 4].try_into().ok()?) as usize;
            let hdr = dxbc.get(chunk_off..chunk_off.checked_add(8)?)?;
            if &hdr[0..4] != b"RDEF" {
                continue;
            }
            let chunk_size = u32::from_le_bytes(hdr[4..8].try_into().ok()?) as usize;
            let payload = dxbc.get(chunk_off + 8..(chunk_off + 8).checked_add(chunk_size)?)?;
            if payload.len() < 28 {
                return None;
            }
            let res_count = u32::from_le_bytes(payload[8..12].try_into().ok()?) as usize;
            let res_off = u32::from_le_bytes(payload[12..16].try_into().ok()?) as usize;
            let target_ver = u16::from_le_bytes(payload[16..18].try_into().ok()?);
            let stride = if target_ver >= 0x0501 { 40usize } else { 32usize };
            for r in 0..res_count {
                let pos = res_off.checked_add(r.checked_mul(stride)?)?;
                let rec = payload.get(pos..pos.checked_add(32)?)?;
                let name_off = u32::from_le_bytes(rec[0..4].try_into().ok()?) as usize;
                let res_type = u32::from_le_bytes(rec[4..8].try_into().ok()?);
                let bind_point = u32::from_le_bytes(rec[20..24].try_into().ok()?);
                if res_type == 5 && bind_point == 1 {
                    let tail = payload.get(name_off..)?;
                    let nul = tail.iter().position(|&b| b == 0)?;
                    return Some(&tail[..nul]);
                }
            }
        }
        None
    }

    #[repr(C)]
    pub struct IDXGIAdapter_Vtbl {
        pub base: IDXGIObject_Vtbl,
        pub EnumOutputs: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub GetDesc: unsafe extern "system" fn(*mut c_void, *mut DXGI_ADAPTER_DESC) -> HRESULT,
        pub CheckInterfaceSupport:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut i64) -> HRESULT,
    }

    #[repr(C)]
    pub struct IDXGIAdapter1_Vtbl {
        pub base: IDXGIAdapter_Vtbl,
        pub GetDesc1: unsafe extern "system" fn(*mut c_void, *mut DXGI_ADAPTER_DESC1) -> HRESULT,
    }

    #[repr(C)]
    pub struct ID3D11DeviceChild_Vtbl {
        pub base: IUnknown_Vtbl,
        pub GetDevice: unsafe extern "system" fn(*mut c_void, *mut *mut c_void),
        pub GetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut u32, *mut c_void) -> HRESULT,
        pub SetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void) -> HRESULT,
        pub SetPrivateDataInterface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    pub struct ID3D11Resource_Vtbl {
        pub base: ID3D11DeviceChild_Vtbl,
        pub GetType: unsafe extern "system" fn(*mut c_void, *mut i32),
        pub SetEvictionPriority: unsafe extern "system" fn(*mut c_void, u32),
        pub GetEvictionPriority: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    pub struct ID3D11Buffer_Vtbl {
        pub base: ID3D11Resource_Vtbl,
        pub GetDesc: unsafe extern "system" fn(*mut c_void, *mut D3D11_BUFFER_DESC),
    }

    #[repr(C)]
    pub struct ID3D11Device_Vtbl {
        pub base: IUnknown_Vtbl,
        pub CreateBuffer: unsafe extern "system" fn(
            *mut c_void,
            *const D3D11_BUFFER_DESC,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateTexture1D: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateTexture2D: unsafe extern "system" fn(
            *mut c_void,
            *const D3D11_TEXTURE2D_DESC,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateTexture3D: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateShaderResourceView: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const D3D11_SHADER_RESOURCE_VIEW_DESC,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateUnorderedAccessView: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateRenderTargetView: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateDepthStencilView: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateInputLayout: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            u32,
            *const c_void,
            usize,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateVertexShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateGeometryShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateGeometryShaderWithStreamOutput: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *const c_void,
            u32,
            *const u32,
            u32,
            u32,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreatePixelShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateHullShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateDomainShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateComputeShader: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            usize,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CreateClassLinkage:
            unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
        pub CreateBlendState:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateDepthStencilState:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateRasterizerState:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateSamplerState:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateQuery:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreatePredicate:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateCounter:
            unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
        pub CreateDeferredContext:
            unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
        pub OpenSharedResource: unsafe extern "system" fn(
            *mut c_void,
            HANDLE,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT,
        pub CheckFormatSupport:
            unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HRESULT,
        pub CheckMultisampleQualityLevels:
            unsafe extern "system" fn(*mut c_void, u32, u32, *mut u32) -> HRESULT,
        pub CheckCounterInfo: unsafe extern "system" fn(*mut c_void, *mut c_void),
        pub CheckCounter: unsafe extern "system" fn(
            *mut c_void,
            *const c_void,
            *mut i32,
            *mut u32,
            *mut u8,
            *mut u32,
            *mut u8,
            *mut u32,
            *mut u8,
            *mut u32,
        ) -> HRESULT,
        pub CheckFeatureSupport:
            unsafe extern "system" fn(*mut c_void, i32, *mut c_void, u32) -> HRESULT,
        pub GetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut u32, *mut c_void) -> HRESULT,
        pub SetPrivateData:
            unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void) -> HRESULT,
        pub SetPrivateDataInterface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut c_void) -> HRESULT,
        pub GetFeatureLevel: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetCreationFlags: unsafe extern "system" fn(*mut c_void) -> u32,
        pub GetDeviceRemovedReason: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        pub GetImmediateContext: unsafe extern "system" fn(*mut c_void, *mut *mut c_void),
        pub SetExceptionMode: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
        pub GetExceptionMode: unsafe extern "system" fn(*mut c_void) -> u32,
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
    use core::mem::size_of;
    use core::ptr;
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

    pub(super) const S_OK: HRESULT = 0;
    pub(super) const E_NOTIMPL: HRESULT = 0x8000_4001u32 as i32;
    pub(super) const E_NOINTERFACE: HRESULT = 0x8000_4002u32 as i32;
    pub(super) const E_POINTER: HRESULT = 0x8000_4003u32 as i32;
    const E_OUTOFMEMORY: HRESULT = 0x8007_000Eu32 as i32;
    pub(super) const E_INVALIDARG: HRESULT = 0x8007_0057u32 as i32;
    const REGDB_E_CLASSNOTREG: HRESULT = 0x8004_0154u32 as i32;
    const DXGI_ERROR_NOT_FOUND: HRESULT = 0x887A_0002u32 as i32;
    const HSTRING_REFERENCE_FLAG: u32 = 1;
    const IS_LEGACY_BUILD: bool = match option_env!("HORIZON_LAUNCHER_LEGACY") {
        Some(v) => v.len() == 1 && v.as_bytes()[0] == b'1',
        None => false,
    };

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

    const D3D_DRIVER_TYPE_HARDWARE: i32 = 1;
    const D3D_DRIVER_TYPE_SOFTWARE: i32 = 4;
    const D3D_DRIVER_TYPE_WARP: i32 = 5;
    const D3D_FEATURE_LEVEL_11_0: u32 = 0xb000;
    const D3D_FEATURE_LEVEL_10_1: u32 = 0xa100;
    const D3D11_CREATE_DEVICE_BGRA_SUPPORT: u32 = 0x20;
    const D3D11_SDK_VERSION: u32 = 7;
    const D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS: i32 = 4;
    const D3D11_BIND_SHADER_RESOURCE: u32 = 0x8;
    const D3D11_RESOURCE_MISC_BUFFER_STRUCTURED: u32 = 0x40;
    const DXGI_FORMAT_R32G32_UINT: u32 = 17;
    const D3D_SRV_DIMENSION_BUFFER: u32 = 1;
    const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;
    const DXGI_MWA_NO_ALT_ENTER: u32 = 2;
    const DIRECTMANIPULATION_READY: i32 = 1;

    pub(super) const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
    const IID_ID3D11_DEVICE: GUID = GUID::from_u128(0xdb6f6ddb_ac77_4e88_8253_819df9bbf140);
    const IID_ID3D11_BUFFER: GUID = GUID::from_u128(0x48570b85_d1ee_4fcd_a250_eb350722b037);

    const IID_IDXGI_OBJECT: GUID = GUID::from_u128(0xaec22fb8_76f3_4639_9be0_28eb43a67a2e);
    const IID_IDXGI_DEVICE_SUB_OBJECT: GUID =
        GUID::from_u128(0x3d3e0379_f9de_4d58_bb6c_18d62992f1a6);
    const IID_IDXGI_DEVICE: GUID = GUID::from_u128(0x54ec77fa_1377_44e6_8c32_88fd5f44c84c);
    const IID_IDXGI_ADAPTER: GUID = GUID::from_u128(0x2411e7e1_12ac_4ccf_bd14_9798e8534dc0);
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
    const MMDEVAPI_W: [u16; 13] =
        wide_lit!('m', 'm', 'd', 'e', 'v', 'a', 'p', 'i', '.', 'd', 'l', 'l');
    const COREMESSAGING_W: [u16; 18] = wide_lit!(
        'c', 'o', 'r', 'e', 'm', 'e', 's', 's', 'a', 'g', 'i', 'n', 'g', '.', 'd', 'l', 'l'
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
        fn GetProcessHeap() -> HANDLE;
        fn HeapAlloc(hHeap: HANDLE, dwFlags: u32, dwBytes: usize) -> *mut c_void;
        fn HeapFree(hHeap: HANDLE, dwFlags: u32, lpMem: *mut c_void) -> BOOL;
        fn MultiByteToWideChar(
            CodePage: u32,
            dwFlags: u32,
            lpMultiByteStr: *const u8,
            cbMultiByte: i32,
            lpWideCharStr: *mut u16,
            cchWideChar: i32,
        ) -> i32;
        fn GetModuleFileNameW(hModule: HMODULE, lpFilename: *mut u16, nSize: u32) -> u32;
        fn GetSystemDirectoryW(lpBuffer: *mut u16, uSize: u32) -> u32;
        fn GetModuleHandleExW(
            dwFlags: u32,
            lpModuleName: *const u16,
            phModule: *mut HMODULE,
        ) -> BOOL;
        fn GetFileAttributesW(lpFileName: *const u16) -> u32;
        fn GetCurrentThreadId() -> u32;
        fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
        fn VirtualProtect(
            lpAddress: *mut c_void,
            dwSize: usize,
            flNewProtect: u32,
            lpflOldProtect: *mut u32,
        ) -> BOOL;
        fn SetErrorMode(uMode: u32) -> u32;
        fn SetUnhandledExceptionFilter(
            lpTopLevelExceptionFilter: Option<
                unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32,
            >,
        ) -> Option<unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32>;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hWnd: HWND) -> HANDLE;
        fn ReleaseDC(hWnd: HWND, hDC: HANDLE) -> i32;
        fn GetSystemMetrics(nIndex: i32) -> i32;
        fn SetProcessDPIAware() -> BOOL;
        fn AdjustWindowRectEx(
            lpRect: *mut RECT,
            dwStyle: u32,
            bMenu: BOOL,
            dwExStyle: u32,
        ) -> BOOL;
        fn MessageBoxW(
            hWnd: HWND,
            lpText: *const u16,
            lpCaption: *const u16,
            uType: u32,
        ) -> i32;
    }

    #[repr(C)]
    struct RTL_OSVERSIONINFOEXW {
        dwOSVersionInfoSize: u32,
        dwMajorVersion: u32,
        dwMinorVersion: u32,
        dwBuildNumber: u32,
        dwPlatformId: u32,
        szCSDVersion: [u16; 128],
        wServicePackMajor: u16,
        wServicePackMinor: u16,
        wSuiteMask: u16,
        wProductType: u8,
        wReserved: u8,
    }

    #[repr(C)]
    struct EXCEPTION_RECORD {
        ExceptionCode: u32,
        ExceptionFlags: u32,
        ExceptionRecord: *mut EXCEPTION_RECORD,
        ExceptionAddress: *mut c_void,
        NumberParameters: u32,
        ExceptionInformation: [usize; 15],
    }

    #[repr(C)]
    struct EXCEPTION_POINTERS {
        ExceptionRecord: *mut EXCEPTION_RECORD,
        ContextRecord: *mut c_void,
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

    fn os_version_info() -> (u32, u32, u32, u16) {
        static PACKED: AtomicUsize = AtomicUsize::new(0);
        let cached = PACKED.load(Ordering::Acquire);
        if cached != 0 {
            let maj = ((cached >> 48) & 0xFF) as u32;
            let min = ((cached >> 40) & 0xFF) as u32;
            let bld = ((cached >> 8) & 0xFFFF_FFFF) as u32;
            let sp = (cached & 0xFF) as u16;
            return (maj, min, bld, sp);
        }
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        let mut info: RTL_OSVERSIONINFOEXW = unsafe { core::mem::zeroed() };
        info.dwOSVersionInfoSize = size_of::<RTL_OSVERSIONINFOEXW>() as u32;
        if let Some(addr) = unsafe { resolve_cached(&SLOT, &NTDLL_W, false, b"RtlGetVersion\0") } {
            let func: unsafe extern "system" fn(*mut RTL_OSVERSIONINFOEXW) -> i32 =
                unsafe { core::mem::transmute(addr) };
            let _ = unsafe { func(&mut info) };
        }
        let maj = info.dwMajorVersion.min(0xFF);
        let min = info.dwMinorVersion.min(0xFF);
        let bld = info.dwBuildNumber;
        let sp = info.wServicePackMajor.min(0xFF);
        let packed = ((maj as usize) << 48)
            | ((min as usize) << 40)
            | ((bld as usize) << 8)
            | (sp as usize);
        if packed != 0 {
            PACKED.store(packed, Ordering::Release);
        }
        (maj, min, bld, sp)
    }

    #[inline]
    fn is_win10_or_later() -> bool {
        os_version_info().0 >= 10
    }

    #[inline]
    fn has_dcomp() -> bool {
        if IS_LEGACY_BUILD || !is_win10_or_later() {
            return false;
        }
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

    static FIRST_FRAME_PRESENTED: AtomicU32 = AtomicU32::new(0);
    static D3D11_STRUCTURED_OK: AtomicU32 = AtomicU32::new(0);

    fn d3d11_diag_log() -> &'static std::sync::Mutex<String> {
        static LOG: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();
        LOG.get_or_init(|| std::sync::Mutex::new(String::new()))
    }

    fn append_d3d11_diag(line: &str) {
        if let Ok(mut guard) = d3d11_diag_log().lock() {
            if guard.len() < 4096 {
                guard.push_str(line);
                guard.push('\n');
            }
        }
    }

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

    pub(super) unsafe fn raw_d3d11_create_device() -> Option<D3D11CreateDeviceFn> {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        let cached = SLOT.load(Ordering::Acquire);
        if cached == MISSING {
            return None;
        }
        if cached != UNRESOLVED {
            return Some(unsafe { core::mem::transmute(cached) });
        }
        let mut sys_buf = [0u16; 520];
        let sys_len =
            unsafe { GetSystemDirectoryW(sys_buf.as_mut_ptr(), 500) } as usize;
        let mut module: HMODULE = 0;
        if sys_len > 0 && sys_len < 500 {
            let mut pos = sys_len;
            if sys_buf[pos - 1] != b'\\' as u16 && sys_buf[pos - 1] != b'/' as u16 {
                sys_buf[pos] = b'\\' as u16;
                pos += 1;
            }
            for &b in b"d3d11.dll" {
                sys_buf[pos] = b as u16;
                pos += 1;
            }
            sys_buf[pos] = 0;
            module = unsafe { LoadLibraryW(sys_buf.as_ptr()) };
        }
        if module == 0 {
            module = unsafe { LoadLibraryW(D3D11_W.as_ptr()) };
        }
        let addr = if module != 0 {
            unsafe { GetProcAddress(module, b"D3D11CreateDevice\0".as_ptr()) as usize }
        } else {
            0
        };
        SLOT.store(if addr == 0 { MISSING } else { addr }, Ordering::Release);
        if addr == 0 {
            None
        } else {
            Some(unsafe { core::mem::transmute(addr) })
        }
    }

    unsafe fn exe_dir_wide() -> Option<Vec<u16>> {
        let mut buf = vec![0u16; 4096];
        let len = unsafe { GetModuleFileNameW(0, buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if len == 0 || len >= buf.len() {
            return None;
        }
        let slice = &buf[..len];
        let sep = slice
            .iter()
            .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)?;
        Some(slice[..=sep].to_vec())
    }

    unsafe fn load_app_relative_dll(rel_ascii: &[u8]) -> HMODULE {
        const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;
        let Some(mut path) = (unsafe { exe_dir_wide() }) else {
            return 0;
        };
        path.extend(rel_ascii.iter().map(|&b| b as u16));
        path.push(0);
        if unsafe { GetFileAttributesW(path.as_ptr()) } == INVALID_FILE_ATTRIBUTES {
            return 0;
        }
        unsafe { LoadLibraryW(path.as_ptr()) }
    }

    unsafe fn load_local_warp_module() -> HMODULE {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        let cached = SLOT.load(Ordering::Acquire);
        if cached == MISSING {
            return 0;
        }
        if cached != UNRESOLVED {
            return cached as HMODULE;
        }
        let mut m = unsafe { load_app_relative_dll(b"d3d10warp.dll") };
        if m == 0 {
            m = unsafe { load_app_relative_dll(b"x64\\d3d10warp.dll") };
        }
        SLOT.store(if m == 0 { MISSING } else { m as usize }, Ordering::Release);
        m
    }

    unsafe fn local_d3d11_create_device() -> Option<D3D11CreateDeviceFn> {
        static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
        let cached = SLOT.load(Ordering::Acquire);
        if cached == MISSING {
            return None;
        }
        if cached != UNRESOLVED {
            return Some(unsafe { core::mem::transmute(cached) });
        }
        let mut m = unsafe { load_app_relative_dll(b"d3d11.dll") };
        if m == 0 {
            m = unsafe { load_app_relative_dll(b"x64\\d3d11.dll") };
        }
        let addr = if m != 0 {
            unsafe { GetProcAddress(m, b"D3D11CreateDevice\0".as_ptr()) as usize }
        } else {
            0
        };
        SLOT.store(if addr == 0 { MISSING } else { addr }, Ordering::Release);
        if addr == 0 {
            None
        } else {
            Some(unsafe { core::mem::transmute(addr) })
        }
    }

    unsafe fn device_supports_structured_buffers(dev: *mut c_void) -> (u32, bool) {
        if dev.is_null() {
            return (0, false);
        }
        let vtbl = unsafe { *(dev as *const *const usize) };
        if vtbl.is_null() {
            return (0, false);
        }
        let get_fl: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { core::mem::transmute(*vtbl.add(37)) };
        let check_feat: unsafe extern "system" fn(*mut c_void, i32, *mut c_void, u32) -> HRESULT =
            unsafe { core::mem::transmute(*vtbl.add(33)) };
        let fl = unsafe { get_fl(dev) };
        let mut hw_opts: i32 = 0;
        let hr = unsafe {
            check_feat(
                dev,
                D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS,
                ptr::from_mut(&mut hw_opts).cast(),
                4,
            )
        };
        let ok = fl >= D3D_FEATURE_LEVEL_11_0 || (hr >= 0 && hw_opts != 0);
        if ok {
            D3D11_STRUCTURED_OK.store(1, Ordering::Release);
        }
        (fl, ok)
    }

    pub(super) const WIN7_QUAD_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_quad_vs.dxbc"));
    pub(super) const WIN7_QUAD_PS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_quad_ps.dxbc"));
    pub(super) const WIN7_SHADOW_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_shadow_vs.dxbc"));
    pub(super) const WIN7_SHADOW_PS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_shadow_ps.dxbc"));
    pub(super) const WIN7_PATH_RASTERIZATION_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_path_rasterization_vs.dxbc"));
    pub(super) const WIN7_PATH_RASTERIZATION_PS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_path_rasterization_ps.dxbc"));
    pub(super) const WIN7_PATH_SPRITE_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_path_sprite_vs.dxbc"));
    pub(super) const WIN7_UNDERLINE_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_underline_vs.dxbc"));
    pub(super) const WIN7_UNDERLINE_PS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_underline_ps.dxbc"));
    pub(super) const WIN7_MONO_SPRITE_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_mono_sprite_vs.dxbc"));
    pub(super) const WIN7_POLY_SPRITE_VS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_poly_sprite_vs.dxbc"));
    pub(super) const WIN7_POLY_SPRITE_PS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/win7_poly_sprite_ps.dxbc"));

    fn replacement_vs_dxbc(bytecode: &[u8]) -> Option<&'static [u8]> {
        match dxbc_structured_t1_name(bytecode)? {
            b"quads" => Some(WIN7_QUAD_VS),
            b"shadows" => Some(WIN7_SHADOW_VS),
            b"path_rasterization_sprites" => Some(WIN7_PATH_RASTERIZATION_VS),
            b"path_sprites" => Some(WIN7_PATH_SPRITE_VS),
            b"underlines" => Some(WIN7_UNDERLINE_VS),
            b"mono_sprites" => Some(WIN7_MONO_SPRITE_VS),
            b"poly_sprites" => Some(WIN7_POLY_SPRITE_VS),
            _ => None,
        }
    }

    fn replacement_ps_dxbc(bytecode: &[u8]) -> Option<&'static [u8]> {
        match dxbc_structured_t1_name(bytecode)? {
            b"quads" => Some(WIN7_QUAD_PS),
            b"shadows" => Some(WIN7_SHADOW_PS),
            b"path_rasterization_sprites" => Some(WIN7_PATH_RASTERIZATION_PS),
            b"underlines" => Some(WIN7_UNDERLINE_PS),
            b"poly_sprites" => Some(WIN7_POLY_SPRITE_PS),
            _ => None,
        }
    }

    #[repr(C)]
    struct Win7D3D11Device {
        vtbl: *const ID3D11Device_Vtbl,
        refs: AtomicU32,
        raw: *mut c_void,
    }

    #[inline]
    pub(super) fn wrap_d3d10_device(raw_device: *mut c_void) -> *mut c_void {
        D3D11_STRUCTURED_OK.store(1, Ordering::Release);
        Box::into_raw(Box::new(Win7D3D11Device {
            vtbl: &D3D11_DEVICE_VTBL,
            refs: AtomicU32::new(1),
            raw: raw_device,
        }))
        .cast()
    }

    #[inline]
    unsafe fn unwrap_d3d11_device(dev: *mut c_void) -> *mut c_void {
        if !dev.is_null()
            && unsafe { *(dev as *const usize) } == ptr::addr_of!(D3D11_DEVICE_VTBL) as usize
        {
            unsafe { (*(dev as *const Win7D3D11Device)).raw }
        } else {
            dev
        }
    }

    unsafe extern "system" fn d3d_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN || g == IID_ID3D11_DEVICE {
            unsafe { d3d_add_ref(this) };
            unsafe { *out = this };
            return S_OK;
        }
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
    unsafe extern "system" fn d3d_add_ref(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7D3D11Device) };
        me.refs.fetch_add(1, Ordering::Relaxed) + 1
    }
    unsafe extern "system" fn d3d_release(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7D3D11Device) };
        let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let boxed = unsafe { Box::from_raw(this as *mut Win7D3D11Device) };
            unsafe { com_release(boxed.raw) };
            0
        } else {
            prev - 1
        }
    }
    unsafe extern "system" fn d3d_create_buffer(
        this: *mut c_void,
        desc: *const D3D11_BUFFER_DESC,
        initial_data: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        if !desc.is_null() {
            let mut d = unsafe { *desc };
            if (d.MiscFlags & D3D11_RESOURCE_MISC_BUFFER_STRUCTURED) != 0 {
                d.ByteWidth = d.ByteWidth.max(8);
                d.MiscFlags &= !D3D11_RESOURCE_MISC_BUFFER_STRUCTURED;
                d.StructureByteStride = 0;
                return unsafe {
                    ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateBuffer)(
                        raw,
                        &d,
                        initial_data,
                        out,
                    )
                };
            }
        }
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateBuffer)(raw, desc, initial_data, out) }
    }
    unsafe extern "system" fn d3d_create_tex1d(
        this: *mut c_void,
        desc: *const c_void,
        initial_data: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture1D)(raw, desc, initial_data, out)
        }
    }
    unsafe extern "system" fn d3d_create_tex2d(
        this: *mut c_void,
        desc: *const D3D11_TEXTURE2D_DESC,
        initial_data: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        let hr = unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture2D)(raw, desc, initial_data, out)
        };
        if hr >= 0 || desc.is_null() {
            return hr;
        }
        let mut d = unsafe { *desc };
        if initial_data.is_null() && (d.Width > 8192 || d.Height > 8192) {
            d.Width = d.Width.min(8192);
            d.Height = d.Height.min(8192);
            let hr_dim = unsafe {
                ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture2D)(raw, &d, initial_data, out)
            };
            if hr_dim >= 0 {
                return hr_dim;
            }
        }
        if d.SampleDesc.Count > 1 && d.SampleDesc.Quality != 0 {
            d.SampleDesc.Quality = 0;
            let hr2 = unsafe {
                ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture2D)(raw, &d, initial_data, out)
            };
            if hr2 >= 0 {
                return hr2;
            }
        }
        if d.SampleDesc.Count > 2 {
            d.SampleDesc.Count = 2;
            d.SampleDesc.Quality = 0;
            let hr3 = unsafe {
                ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture2D)(raw, &d, initial_data, out)
            };
            if hr3 >= 0 {
                return hr3;
            }
        }
        if d.SampleDesc.Count > 1 {
            d.SampleDesc.Count = 1;
            d.SampleDesc.Quality = 0;
            return unsafe {
                ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture2D)(raw, &d, initial_data, out)
            };
        }
        hr
    }
    unsafe extern "system" fn d3d_create_tex3d(
        this: *mut c_void,
        desc: *const c_void,
        initial_data: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateTexture3D)(raw, desc, initial_data, out)
        }
    }
    unsafe extern "system" fn d3d_create_srv(
        this: *mut c_void,
        resource: *mut c_void,
        desc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        if desc.is_null() && !resource.is_null() {
            if let Some(buf_ptr) = unsafe { com_qi(resource, &IID_ID3D11_BUFFER) } {
                let mut bdesc: D3D11_BUFFER_DESC = unsafe { core::mem::zeroed() };
                unsafe {
                    ((*com_vtbl::<ID3D11Buffer_Vtbl>(buf_ptr)).GetDesc)(buf_ptr, &mut bdesc);
                    com_release(buf_ptr);
                }
                if (bdesc.MiscFlags & D3D11_RESOURCE_MISC_BUFFER_STRUCTURED) == 0
                    && (bdesc.BindFlags & D3D11_BIND_SHADER_RESOURCE) != 0
                {
                    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                        Format: DXGI_FORMAT_R32G32_UINT,
                        ViewDimension: D3D_SRV_DIMENSION_BUFFER,
                        u: [0, (bdesc.ByteWidth / 8).max(1), 0, 0],
                    };
                    return unsafe {
                        ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateShaderResourceView)(
                            raw, resource, &srv_desc, out,
                        )
                    };
                }
            }
        }
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateShaderResourceView)(
                raw, resource, desc, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_uav(
        this: *mut c_void,
        resource: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateUnorderedAccessView)(
                raw, resource, desc, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_rtv(
        this: *mut c_void,
        resource: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateRenderTargetView)(raw, resource, desc, out)
        }
    }
    unsafe extern "system" fn d3d_create_dsv(
        this: *mut c_void,
        resource: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateDepthStencilView)(raw, resource, desc, out)
        }
    }
    unsafe extern "system" fn d3d_create_il(
        this: *mut c_void,
        elems: *const c_void,
        num: u32,
        bytecode: *const c_void,
        len: usize,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateInputLayout)(
                raw, elems, num, bytecode, len, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_vs(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        if !bytecode.is_null() && len > 0 {
            let slice = unsafe { core::slice::from_raw_parts(bytecode.cast::<u8>(), len) };
            if let Some(repl) = replacement_vs_dxbc(slice) {
                return unsafe {
                    ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateVertexShader)(
                        raw,
                        repl.as_ptr().cast(),
                        repl.len(),
                        linkage,
                        out,
                    )
                };
            }
        }
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateVertexShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_gs(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateGeometryShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_gs_so(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        so_decl: *const c_void,
        num_entries: u32,
        strides: *const u32,
        num_strides: u32,
        rasterized_stream: u32,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateGeometryShaderWithStreamOutput)(
                raw,
                bytecode,
                len,
                so_decl,
                num_entries,
                strides,
                num_strides,
                rasterized_stream,
                linkage,
                out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_ps(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        if !bytecode.is_null() && len > 0 {
            let slice = unsafe { core::slice::from_raw_parts(bytecode.cast::<u8>(), len) };
            if let Some(repl) = replacement_ps_dxbc(slice) {
                return unsafe {
                    ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreatePixelShader)(
                        raw,
                        repl.as_ptr().cast(),
                        repl.len(),
                        linkage,
                        out,
                    )
                };
            }
        }
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreatePixelShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_hs(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateHullShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_ds(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateDomainShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_cs(
        this: *mut c_void,
        bytecode: *const c_void,
        len: usize,
        linkage: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateComputeShader)(
                raw, bytecode, len, linkage, out,
            )
        }
    }
    unsafe extern "system" fn d3d_create_cl(
        this: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateClassLinkage)(raw, out) }
    }
    unsafe extern "system" fn d3d_create_bs(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateBlendState)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_dss(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateDepthStencilState)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_rs(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateRasterizerState)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_ss(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateSamplerState)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_query(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateQuery)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_pred(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreatePredicate)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_counter(
        this: *mut c_void,
        desc: *const c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateCounter)(raw, desc, out) }
    }
    unsafe extern "system" fn d3d_create_def_ctx(
        this: *mut c_void,
        flags: u32,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CreateDeferredContext)(raw, flags, out) }
    }
    unsafe extern "system" fn d3d_open_shared(
        this: *mut c_void,
        handle: HANDLE,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).OpenSharedResource)(raw, handle, iid, out)
        }
    }
    unsafe extern "system" fn d3d_check_fmt(
        this: *mut c_void,
        fmt: u32,
        out: *mut u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CheckFormatSupport)(raw, fmt, out) }
    }
    unsafe extern "system" fn d3d_check_msaa(
        this: *mut c_void,
        fmt: u32,
        count: u32,
        out: *mut u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CheckMultisampleQualityLevels)(
                raw, fmt, count, out,
            )
        }
    }
    unsafe extern "system" fn d3d_check_counter_info(this: *mut c_void, info: *mut c_void) {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CheckCounterInfo)(raw, info) }
    }
    unsafe extern "system" fn d3d_check_counter(
        this: *mut c_void,
        desc: *const c_void,
        ctype: *mut i32,
        active: *mut u32,
        name: *mut u8,
        name_len: *mut u32,
        units: *mut u8,
        units_len: *mut u32,
        description: *mut u8,
        desc_len: *mut u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CheckCounter)(
                raw,
                desc,
                ctype,
                active,
                name,
                name_len,
                units,
                units_len,
                description,
                desc_len,
            )
        }
    }
    unsafe extern "system" fn d3d_check_feature(
        this: *mut c_void,
        feature: i32,
        data: *mut c_void,
        data_size: u32,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        let hr = unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).CheckFeatureSupport)(
                raw, feature, data, data_size,
            )
        };
        if feature == D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS && !data.is_null() && data_size >= 4 {
            unsafe { *(data as *mut i32) = 1 };
            return S_OK;
        }
        hr
    }
    unsafe extern "system" fn d3d_get_priv(
        this: *mut c_void,
        guid: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetPrivateData)(raw, guid, size, data) }
    }
    unsafe extern "system" fn d3d_set_priv(
        this: *mut c_void,
        guid: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).SetPrivateData)(raw, guid, size, data) }
    }
    unsafe extern "system" fn d3d_set_priv_if(
        this: *mut c_void,
        guid: *const GUID,
        unk: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe {
            ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).SetPrivateDataInterface)(raw, guid, unk)
        }
    }
    unsafe extern "system" fn d3d_get_fl(this: *mut c_void) -> u32 {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetFeatureLevel)(raw) }
    }
    unsafe extern "system" fn d3d_get_flags(this: *mut c_void) -> u32 {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetCreationFlags)(raw) }
    }
    unsafe extern "system" fn d3d_get_removed(this: *mut c_void) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetDeviceRemovedReason)(raw) }
    }
    unsafe extern "system" fn d3d_get_imm_ctx(this: *mut c_void, out: *mut *mut c_void) {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetImmediateContext)(raw, out) }
    }
    unsafe extern "system" fn d3d_set_exc(this: *mut c_void, flags: u32) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).SetExceptionMode)(raw, flags) }
    }
    unsafe extern "system" fn d3d_get_exc(this: *mut c_void) -> u32 {
        let raw = unsafe { (*(this as *const Win7D3D11Device)).raw };
        unsafe { ((*com_vtbl::<ID3D11Device_Vtbl>(raw)).GetExceptionMode)(raw) }
    }

    static D3D11_DEVICE_VTBL: ID3D11Device_Vtbl = ID3D11Device_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: d3d_qi,
            AddRef: d3d_add_ref,
            Release: d3d_release,
        },
        CreateBuffer: d3d_create_buffer,
        CreateTexture1D: d3d_create_tex1d,
        CreateTexture2D: d3d_create_tex2d,
        CreateTexture3D: d3d_create_tex3d,
        CreateShaderResourceView: d3d_create_srv,
        CreateUnorderedAccessView: d3d_create_uav,
        CreateRenderTargetView: d3d_create_rtv,
        CreateDepthStencilView: d3d_create_dsv,
        CreateInputLayout: d3d_create_il,
        CreateVertexShader: d3d_create_vs,
        CreateGeometryShader: d3d_create_gs,
        CreateGeometryShaderWithStreamOutput: d3d_create_gs_so,
        CreatePixelShader: d3d_create_ps,
        CreateHullShader: d3d_create_hs,
        CreateDomainShader: d3d_create_ds,
        CreateComputeShader: d3d_create_cs,
        CreateClassLinkage: d3d_create_cl,
        CreateBlendState: d3d_create_bs,
        CreateDepthStencilState: d3d_create_dss,
        CreateRasterizerState: d3d_create_rs,
        CreateSamplerState: d3d_create_ss,
        CreateQuery: d3d_create_query,
        CreatePredicate: d3d_create_pred,
        CreateCounter: d3d_create_counter,
        CreateDeferredContext: d3d_create_def_ctx,
        OpenSharedResource: d3d_open_shared,
        CheckFormatSupport: d3d_check_fmt,
        CheckMultisampleQualityLevels: d3d_check_msaa,
        CheckCounterInfo: d3d_check_counter_info,
        CheckCounter: d3d_check_counter,
        CheckFeatureSupport: d3d_check_feature,
        GetPrivateData: d3d_get_priv,
        SetPrivateData: d3d_set_priv,
        SetPrivateDataInterface: d3d_set_priv_if,
        GetFeatureLevel: d3d_get_fl,
        GetCreationFlags: d3d_get_flags,
        GetDeviceRemovedReason: d3d_get_removed,
        GetImmediateContext: d3d_get_imm_ctx,
        SetExceptionMode: d3d_set_exc,
        GetExceptionMode: d3d_get_exc,
    };

    unsafe fn maybe_wrap_d3d10_device(dev: *mut c_void, fl: u32, sb_ok: bool) -> *mut c_void {
        if dev.is_null() {
            return dev;
        }
        let force_d3d10 = std::env::var_os("HORIZON_FORCE_D3D10_SHADERS").is_some();
        if (!sb_ok || force_d3d10) && fl >= D3D_FEATURE_LEVEL_10_1 {
            append_d3d11_diag(&format!(
                "Wrapping D3D device (fl=0x{:x}, native_sb={}) in Win7D3D11Device (Buffer<uint2> emulation)",
                fl, sb_ok
            ));
            wrap_d3d10_device(dev)
        } else {
            dev
        }
    }

    unsafe fn call_d3d11_filtered(
        func: D3D11CreateDeviceFn,
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
        let hr = unsafe {
            func(
                adapter,
                driver_type,
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
                adapter,
                driver_type,
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

    unsafe fn create_warp_or_software_device(
        func: D3D11CreateDeviceFn,
        flags: u32,
        feature_levels: *const u32,
        num_feature_levels: u32,
        sdk_version: u32,
        device_out: *mut *mut c_void,
        fl_out: *mut u32,
        ctx_out: *mut *mut c_void,
    ) -> HRESULT {
        let mut dev = ptr::null_mut();
        let mut fl = 0u32;
        let mut ctx = ptr::null_mut();
        let want_ctx = !ctx_out.is_null();

        // 1. Try Windows WARP (`D3D_DRIVER_TYPE_WARP = 5`, `d3d10warp.dll`).
        let warp_hr = unsafe {
            call_d3d11_filtered(
                func,
                ptr::null_mut(),
                D3D_DRIVER_TYPE_WARP,
                0,
                flags,
                feature_levels,
                num_feature_levels,
                sdk_version,
                &mut dev,
                &mut fl,
                if want_ctx { &mut ctx } else { ptr::null_mut() },
            )
        };
        if warp_hr >= 0 && !dev.is_null() {
            let (actual_fl, sb_ok) = unsafe { device_supports_structured_buffers(dev) };
            append_d3d11_diag(&format!(
                "WARP(System32): hr=0x{:08x} fl=0x{:x} structured_buffer={}",
                warp_hr as u32, actual_fl, sb_ok
            ));
            if sb_ok {
                let out_dev = unsafe { maybe_wrap_d3d10_device(dev, actual_fl, sb_ok) };
                if !fl_out.is_null() {
                    unsafe { *fl_out = fl };
                }
                if !ctx_out.is_null() {
                    unsafe { *ctx_out = ctx };
                }
                if !device_out.is_null() {
                    unsafe { *device_out = out_dev };
                } else {
                    unsafe { com_release(out_dev) };
                }
                return warp_hr;
            }
        } else {
            append_d3d11_diag(&format!("WARP(System32): failed hr=0x{:08x}", warp_hr as u32));
        }

        // 2. If System32 WARP lacks StructuredBuffer (unpatched Win7 SP1 without KB2670838),
        //    try app-local `d3d10warp.dll` (e.g. Mesa3D x64) via `D3D_DRIVER_TYPE_SOFTWARE`.
        let local_warp = unsafe { load_local_warp_module() };
        if local_warp != 0 {
            let mut sw_dev = ptr::null_mut();
            let mut sw_fl = 0u32;
            let mut sw_ctx = ptr::null_mut();
            let sw_hr = unsafe {
                call_d3d11_filtered(
                    func,
                    ptr::null_mut(),
                    D3D_DRIVER_TYPE_SOFTWARE,
                    local_warp,
                    flags,
                    feature_levels,
                    num_feature_levels,
                    sdk_version,
                    &mut sw_dev,
                    &mut sw_fl,
                    if want_ctx { &mut sw_ctx } else { ptr::null_mut() },
                )
            };
            if sw_hr >= 0 && !sw_dev.is_null() {
                let (actual_fl, sb_ok) = unsafe { device_supports_structured_buffers(sw_dev) };
                append_d3d11_diag(&format!(
                    "WARP(app-local d3d10warp.dll): hr=0x{:08x} fl=0x{:x} structured_buffer={}",
                    sw_hr as u32, actual_fl, sb_ok
                ));
                if sb_ok {
                    unsafe {
                        com_release(ctx);
                        com_release(dev);
                    }
                    if !fl_out.is_null() {
                        unsafe { *fl_out = sw_fl };
                    }
                    if !ctx_out.is_null() {
                        unsafe { *ctx_out = sw_ctx };
                    }
                    if !device_out.is_null() {
                        unsafe { *device_out = sw_dev };
                    } else {
                        unsafe { com_release(sw_dev) };
                    }
                    return sw_hr;
                }
                unsafe {
                    com_release(sw_ctx);
                    com_release(sw_dev);
                }
            } else {
                append_d3d11_diag(&format!(
                    "WARP(app-local d3d10warp.dll): failed hr=0x{:08x}",
                    sw_hr as u32
                ));
            }
        }

        // 3. Also check app-local `d3d11.dll` if distinct from the already-loaded module.
        if let Some(local_fn) = unsafe { local_d3d11_create_device() } {
            if local_fn as usize != func as usize {
                for drv in [D3D_DRIVER_TYPE_WARP, D3D_DRIVER_TYPE_HARDWARE] {
                    let mut l_dev = ptr::null_mut();
                    let mut l_fl = 0u32;
                    let mut l_ctx = ptr::null_mut();
                    let l_hr = unsafe {
                        call_d3d11_filtered(
                            local_fn,
                            ptr::null_mut(),
                            drv,
                            0,
                            flags,
                            feature_levels,
                            num_feature_levels,
                            sdk_version,
                            &mut l_dev,
                            &mut l_fl,
                            if want_ctx { &mut l_ctx } else { ptr::null_mut() },
                        )
                    };
                    if l_hr >= 0 && !l_dev.is_null() {
                        let (actual_fl, sb_ok) =
                            unsafe { device_supports_structured_buffers(l_dev) };
                        append_d3d11_diag(&format!(
                            "Local d3d11.dll(drv={}): hr=0x{:08x} fl=0x{:x} structured_buffer={}",
                            drv, l_hr as u32, actual_fl, sb_ok
                        ));
                        if sb_ok {
                            unsafe {
                                com_release(ctx);
                                com_release(dev);
                            }
                            if !fl_out.is_null() {
                                unsafe { *fl_out = l_fl };
                            }
                            if !ctx_out.is_null() {
                                unsafe { *ctx_out = l_ctx };
                            }
                            if !device_out.is_null() {
                                unsafe { *device_out = l_dev };
                            } else {
                                unsafe { com_release(l_dev) };
                            }
                            return l_hr;
                        }
                        unsafe {
                            com_release(l_ctx);
                            com_release(l_dev);
                        }
                    }
                }
            }
        }

        if warp_hr >= 0 && !dev.is_null() {
            let (actual_fl, sb_ok) = unsafe { device_supports_structured_buffers(dev) };
            let out_dev = unsafe { maybe_wrap_d3d10_device(dev, actual_fl, sb_ok) };
            if !fl_out.is_null() {
                unsafe { *fl_out = fl };
            }
            if !ctx_out.is_null() {
                unsafe { *ctx_out = ctx };
            }
            if !device_out.is_null() {
                unsafe { *device_out = out_dev };
            } else {
                unsafe { com_release(out_dev) };
            }
        } else {
            if !ctx_out.is_null() {
                unsafe { *ctx_out = ptr::null_mut() };
            }
            if !device_out.is_null() {
                unsafe { *device_out = ptr::null_mut() };
            }
        }
        warp_hr
    }

    #[repr(C)]
    struct Win7WarpAdapter1 {
        vtbl: *const IDXGIAdapter1_Vtbl,
        refs: AtomicU32,
        raw_adapter: *mut c_void,
        parent_factory: *mut c_void,
    }

    #[inline]
    unsafe fn is_warp_adapter(adapter: *mut c_void) -> bool {
        !adapter.is_null()
            && unsafe { *(adapter as *const usize) } == ptr::addr_of!(WARP_ADAPTER1_VTBL) as usize
    }

    unsafe extern "system" fn wa1_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        if iid.is_null() {
            unsafe { *out = ptr::null_mut() };
            return E_INVALIDARG;
        }
        let g = unsafe { *iid };
        if g == IID_IUNKNOWN
            || g == IID_IDXGI_OBJECT
            || g == IID_IDXGI_ADAPTER
            || g == IID_IDXGI_ADAPTER1
        {
            unsafe { wa1_add_ref(this) };
            unsafe { *out = this };
            return S_OK;
        }
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }
    unsafe extern "system" fn wa1_add_ref(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7WarpAdapter1) };
        me.refs.fetch_add(1, Ordering::Relaxed) + 1
    }
    unsafe extern "system" fn wa1_release(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const Win7WarpAdapter1) };
        let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let boxed = unsafe { Box::from_raw(this as *mut Win7WarpAdapter1) };
            unsafe {
                com_release(boxed.raw_adapter);
                com_release(boxed.parent_factory);
            }
            0
        } else {
            prev - 1
        }
    }
    unsafe extern "system" fn wa1_set_priv(
        this: *mut c_void,
        name: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateData)(raw, name, size, data) }
        } else {
            S_OK
        }
    }
    unsafe extern "system" fn wa1_set_priv_if(
        this: *mut c_void,
        name: *const GUID,
        unk: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).SetPrivateDataInterface)(raw, name, unk) }
        } else {
            S_OK
        }
    }
    unsafe extern "system" fn wa1_get_priv(
        this: *mut c_void,
        name: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe { ((*com_vtbl::<IDXGIObject_Vtbl>(raw)).GetPrivateData)(raw, name, size, data) }
        } else {
            E_NOTIMPL
        }
    }
    unsafe extern "system" fn wa1_get_parent(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || iid.is_null() {
            return E_INVALIDARG;
        }
        let me = unsafe { &*(this as *const Win7WarpAdapter1) };
        if !me.parent_factory.is_null() {
            let hr = unsafe {
                ((*com_vtbl::<IUnknown_Vtbl>(me.parent_factory)).QueryInterface)(
                    me.parent_factory,
                    iid,
                    out,
                )
            };
            if hr >= 0 {
                return hr;
            }
        }
        if !me.raw_adapter.is_null() {
            return unsafe {
                ((*com_vtbl::<IDXGIObject_Vtbl>(me.raw_adapter)).GetParent)(me.raw_adapter, iid, out)
            };
        }
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
    unsafe extern "system" fn wa1_enum_outputs(
        this: *mut c_void,
        index: u32,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe { ((*com_vtbl::<IDXGIAdapter_Vtbl>(raw)).EnumOutputs)(raw, index, out) }
        } else {
            unsafe { *out = ptr::null_mut() };
            DXGI_ERROR_NOT_FOUND
        }
    }
    fn fill_default_warp_desc(desc: &mut DXGI_ADAPTER_DESC) {
        *desc = unsafe { core::mem::zeroed() };
        let label = b"Microsoft Basic Render Driver (WARP)";
        for (i, &b) in label.iter().enumerate() {
            desc.Description[i] = b as u16;
        }
        desc.VendorId = 0x1414;
        desc.DeviceId = 0x008c;
    }
    unsafe extern "system" fn wa1_get_desc(
        this: *mut c_void,
        out: *mut DXGI_ADAPTER_DESC,
    ) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            let hr = unsafe { ((*com_vtbl::<IDXGIAdapter_Vtbl>(raw)).GetDesc)(raw, out) };
            if hr >= 0 {
                return hr;
            }
        }
        fill_default_warp_desc(unsafe { &mut *out });
        S_OK
    }
    unsafe extern "system" fn wa1_check_if(
        this: *mut c_void,
        guid: *const GUID,
        umd_ver: *mut i64,
    ) -> HRESULT {
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            unsafe {
                ((*com_vtbl::<IDXGIAdapter_Vtbl>(raw)).CheckInterfaceSupport)(raw, guid, umd_ver)
            }
        } else {
            if !umd_ver.is_null() {
                unsafe { *umd_ver = 0 };
            }
            S_OK
        }
    }
    unsafe extern "system" fn wa1_get_desc1(
        this: *mut c_void,
        out: *mut DXGI_ADAPTER_DESC1,
    ) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        let raw = unsafe { (*(this as *const Win7WarpAdapter1)).raw_adapter };
        if !raw.is_null() {
            if let Some(a1) = unsafe { com_qi(raw, &IID_IDXGI_ADAPTER1) } {
                let hr = unsafe { ((*com_vtbl::<IDXGIAdapter1_Vtbl>(a1)).GetDesc1)(a1, out) };
                unsafe { com_release(a1) };
                if hr >= 0 {
                    unsafe { (*out).Flags |= DXGI_ADAPTER_FLAG_SOFTWARE };
                    return S_OK;
                }
            }
        }
        let mut base_desc: DXGI_ADAPTER_DESC = unsafe { core::mem::zeroed() };
        let _ = unsafe { wa1_get_desc(this, &mut base_desc) };
        unsafe {
            *out = DXGI_ADAPTER_DESC1 {
                Description: base_desc.Description,
                VendorId: base_desc.VendorId,
                DeviceId: base_desc.DeviceId,
                SubSysId: base_desc.SubSysId,
                Revision: base_desc.Revision,
                DedicatedVideoMemory: base_desc.DedicatedVideoMemory,
                DedicatedSystemMemory: base_desc.DedicatedSystemMemory,
                SharedSystemMemory: base_desc.SharedSystemMemory,
                AdapterLuid: base_desc.AdapterLuid,
                Flags: DXGI_ADAPTER_FLAG_SOFTWARE,
            };
        }
        S_OK
    }

    static WARP_ADAPTER1_VTBL: IDXGIAdapter1_Vtbl = IDXGIAdapter1_Vtbl {
        base: IDXGIAdapter_Vtbl {
            base: IDXGIObject_Vtbl {
                base: IUnknown_Vtbl {
                    QueryInterface: wa1_qi,
                    AddRef: wa1_add_ref,
                    Release: wa1_release,
                },
                SetPrivateData: wa1_set_priv,
                SetPrivateDataInterface: wa1_set_priv_if,
                GetPrivateData: wa1_get_priv,
                GetParent: wa1_get_parent,
            },
            EnumOutputs: wa1_enum_outputs,
            GetDesc: wa1_get_desc,
            CheckInterfaceSupport: wa1_check_if,
        },
        GetDesc1: wa1_get_desc1,
    };

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
            if unsafe { is_warp_adapter(adapter) } || driver_type == D3D_DRIVER_TYPE_WARP {
                return unsafe {
                    create_warp_or_software_device(
                        func,
                        flags,
                        feature_levels,
                        num_feature_levels,
                        sdk_version,
                        device,
                        feature_level,
                        immediate_context,
                    )
                };
            }
            let hr = unsafe {
                call_d3d11_filtered(
                    func,
                    adapter,
                    driver_type,
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
            if hr >= 0 && !device.is_null() {
                let created = unsafe { *device };
                let (fl, sb_ok) = unsafe { device_supports_structured_buffers(created) };
                append_d3d11_diag(&format!(
                    "HW D3D11CreateDevice: hr=0x{:08x} fl=0x{:x} structured_buffer={}",
                    hr as u32, fl, sb_ok
                ));
                unsafe {
                    *device = maybe_wrap_d3d10_device(created, fl, sb_ok);
                }
            } else {
                append_d3d11_diag(&format!(
                    "HW D3D11CreateDevice: failed hr=0x{:08x}",
                    hr as u32
                ));
            }
            hr
        }
    );

    unsafe fn synthesize_warp_adapter(parent_factory: *mut c_void) -> Option<*mut c_void> {
        let func = unsafe { raw_d3d11_create_device()? };
        let levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1];
        let mut device = ptr::null_mut();
        let mut chosen = 0u32;
        let hr = unsafe {
            create_warp_or_software_device(
                func,
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
        let raw_dev = unsafe { unwrap_d3d11_device(device) };
        let mut raw_adapter = ptr::null_mut();
        if let Some(dxgi_dev) = unsafe { com_qi(raw_dev, &IID_IDXGI_DEVICE) } {
            let _ = unsafe {
                ((*com_vtbl::<IDXGIDevice_Vtbl>(dxgi_dev)).GetAdapter)(dxgi_dev, &mut raw_adapter)
            };
            unsafe { com_release(dxgi_dev) };
        }
        unsafe { com_release(device) };
        if !parent_factory.is_null() {
            unsafe { com_add_ref(parent_factory) };
        }
        let wrapped = Box::into_raw(Box::new(Win7WarpAdapter1 {
            vtbl: &WARP_ADAPTER1_VTBL,
            refs: AtomicU32::new(1),
            raw_adapter,
            parent_factory,
        }));
        Some(wrapped.cast())
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
        let hr = unsafe { ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).Present)(raw, sync, flags) };
        if hr >= 0 {
            FIRST_FRAME_PRESENTED.store(1, Ordering::Release);
        }
        hr
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
        unsafe {
            ((*com_vtbl::<IDXGISwapChain_Vtbl>(raw)).ResizeBuffers)(
                raw,
                0,
                w.max(1),
                h.max(1),
                fmt,
                flags,
            )
        }
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
    unsafe fn resolve_factory_for_device(
        device: *mut c_void,
        fallback_raw_factory: *mut c_void,
    ) -> (*mut c_void, bool) {
        let raw_dev = unsafe { unwrap_d3d11_device(device) };
        if let Some(dxgi_dev) = unsafe { com_qi(raw_dev, &IID_IDXGI_DEVICE) } {
            let mut adapter = ptr::null_mut();
            let hr = unsafe {
                ((*com_vtbl::<IDXGIDevice_Vtbl>(dxgi_dev)).GetAdapter)(dxgi_dev, &mut adapter)
            };
            unsafe { com_release(dxgi_dev) };
            if hr >= 0 && !adapter.is_null() {
                let mut dev_factory = ptr::null_mut();
                let p_hr = unsafe {
                    ((*com_vtbl::<IDXGIObject_Vtbl>(adapter)).GetParent)(
                        adapter,
                        &IID_IDXGI_FACTORY,
                        &mut dev_factory,
                    )
                };
                unsafe { com_release(adapter) };
                if p_hr >= 0 && !dev_factory.is_null() {
                    if unsafe { *(dev_factory as *const usize) }
                        == ptr::addr_of!(FACTORY6_VTBL) as usize
                    {
                        let inner = unsafe { (*(dev_factory as *const Win7DxgiFactory6)).raw };
                        unsafe {
                            com_add_ref(inner);
                            com_release(dev_factory);
                        }
                        return (inner, true);
                    }
                    return (dev_factory, true);
                }
            }
        }
        (fallback_raw_factory, false)
    }

    unsafe extern "system" fn f6_enum_adapters(
        this: *mut c_void,
        index: u32,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        if std::env::var_os("HORIZON_FORCE_WARP").is_some() {
            if index == 0 {
                if let Some(warp_adapter) = unsafe { synthesize_warp_adapter(this) } {
                    unsafe { *out = warp_adapter };
                    return S_OK;
                }
            }
            unsafe { *out = ptr::null_mut() };
            return DXGI_ERROR_NOT_FOUND;
        }
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
            if let Some(warp_adapter) = unsafe { synthesize_warp_adapter(this) } {
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
        let hr =
            unsafe { ((*com_vtbl::<IDXGIFactory_Vtbl>(raw)).MakeWindowAssociation)(raw, hwnd, flags) };
        if hr < 0 { S_OK } else { hr }
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
        let raw_dev = unsafe { unwrap_d3d11_device(device) };
        let raw = unsafe { (*(this as *const Win7DxgiFactory6)).raw };
        let (target_factory, owns_ref) = unsafe { resolve_factory_for_device(raw_dev, raw) };
        let hr = unsafe {
            ((*com_vtbl::<IDXGIFactory_Vtbl>(target_factory)).CreateSwapChain)(
                target_factory,
                raw_dev,
                desc,
                out,
            )
        };
        if owns_ref {
            unsafe { com_release(target_factory) };
        }
        hr
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
        if out.is_null() {
            return E_INVALIDARG;
        }
        if std::env::var_os("HORIZON_FORCE_WARP").is_some() {
            if index == 0 {
                if let Some(warp_adapter) = unsafe { synthesize_warp_adapter(this) } {
                    unsafe { *out = warp_adapter };
                    return S_OK;
                }
            }
            unsafe { *out = ptr::null_mut() };
            return DXGI_ERROR_NOT_FOUND;
        }
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        let hr = unsafe {
            ((*com_vtbl::<IDXGIFactory1_Vtbl>(me.raw)).EnumAdapters1)(me.raw, index, out)
        };
        if hr >= 0 {
            return hr;
        }
        let slot = me.warp_slot.load(Ordering::Acquire);
        if hr == DXGI_ERROR_NOT_FOUND && (slot == u32::MAX || slot == index) {
            me.warp_slot.store(index, Ordering::Release);
            if let Some(warp_adapter) = unsafe { synthesize_warp_adapter(this) } {
                unsafe { *out = warp_adapter };
                return S_OK;
            }
        }
        hr
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
        let raw_dev = unsafe { unwrap_d3d11_device(device) };
        let me = unsafe { &*(this as *const Win7DxgiFactory6) };
        if has_dcomp() {
            if let Some(f2) = unsafe { com_qi(me.raw, &IID_IDXGI_FACTORY2) } {
                let hr = unsafe {
                    ((*com_vtbl::<IDXGIFactory2_Vtbl>(f2)).CreateSwapChainForHwnd)(
                        f2,
                        raw_dev,
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
                Width: d.Width.max(1),
                Height: d.Height.max(1),
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
        let (target_factory, owns_ref) = unsafe { resolve_factory_for_device(raw_dev, me.raw) };
        let mut raw_sc = ptr::null_mut();
        let hr = unsafe {
            ((*com_vtbl::<IDXGIFactory_Vtbl>(target_factory)).CreateSwapChain)(
                target_factory,
                raw_dev,
                &legacy,
                &mut raw_sc,
            )
        };
        if hr >= 0 && !raw_sc.is_null() {
            let _ = unsafe {
                ((*com_vtbl::<IDXGIFactory_Vtbl>(target_factory)).MakeWindowAssociation)(
                    target_factory,
                    hwnd,
                    DXGI_MWA_NO_ALT_ENTER,
                )
            };
        }
        if owns_ref {
            unsafe { com_release(target_factory) };
        }
        if hr < 0 || raw_sc.is_null() {
            append_d3d11_diag(&format!("CreateSwapChain failed: hr=0x{:08x}", hr as u32));
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
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || iid.is_null() {
            return E_INVALIDARG;
        }
        if let Some(warp) = unsafe { synthesize_warp_adapter(this) } {
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

    #[repr(C)]
    struct Win7HStringHeader {
        flags: u32,
        len: u32,
        _0: u32,
        _1: u32,
        data: *mut u16,
        count: AtomicU32,
        buffer_start: u16,
    }

    #[repr(C)]
    struct Win7HStringRefHeader {
        flags: u32,
        len: u32,
        padding1: u32,
        padding2: u32,
        data: *const u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DispatcherQueueOptionsAbi {
        pub dw_size: u32,
        pub thread_type: i32,
        pub apartment_type: i32,
    }

    unsafe fn hstring_alloc(source: *const u16, length: u32) -> Result<*mut c_void, HRESULT> {
        if length == 0 {
            return Ok(ptr::null_mut());
        }
        if source.is_null() {
            return Err(E_POINTER);
        }
        let Some(extra) = (length as usize).checked_mul(2) else {
            return Err(E_OUTOFMEMORY);
        };
        let Some(bytes) = size_of::<Win7HStringHeader>().checked_add(extra) else {
            return Err(E_OUTOFMEMORY);
        };
        let raw = HeapAlloc(GetProcessHeap(), 0, bytes).cast::<Win7HStringHeader>();
        if raw.is_null() {
            return Err(E_OUTOFMEMORY);
        }
        raw.write(Win7HStringHeader {
            flags: 0,
            len: length,
            _0: 0,
            _1: 0,
            data: ptr::null_mut(),
            count: AtomicU32::new(1),
            buffer_start: 0,
        });
        let buf = ptr::addr_of_mut!((*raw).buffer_start);
        (*raw).data = buf;
        ptr::copy_nonoverlapping(source, buf, length as usize);
        *buf.add(length as usize) = 0;
        Ok(raw.cast::<c_void>())
    }

    define_shim!(
        CoInitializeEx,
        "__imp_CoInitializeEx",
        IMP_CO_INITIALIZE_EX,
        (pv_reserved: *const c_void, dw_coinit: u32) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoInitializeEx\0") else {
                return E_NOTIMPL;
            };
            let func: unsafe extern "system" fn(*const c_void, u32) -> HRESULT =
                core::mem::transmute(addr);
            func(pv_reserved, dw_coinit)
        }
    );

    define_shim!(
        CoInitialize,
        "__imp_CoInitialize",
        IMP_CO_INITIALIZE,
        (pv_reserved: *const c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoInitialize\0") else {
                return E_NOTIMPL;
            };
            let func: unsafe extern "system" fn(*const c_void) -> HRESULT =
                core::mem::transmute(addr);
            func(pv_reserved)
        }
    );

    define_shim!(
        CoUninitialize,
        "__imp_CoUninitialize",
        IMP_CO_UNINITIALIZE,
        () {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoUninitialize\0") {
                let func: unsafe extern "system" fn() = core::mem::transmute(addr);
                func();
            }
        }
    );

    define_shim!(
        CoCreateFreeThreadedMarshaler,
        "__imp_CoCreateFreeThreadedMarshaler",
        IMP_CO_CREATE_FREE_THREADED_MARSHALER,
        (punk_outer: *mut c_void, ppunk_marshal: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) =
                resolve_cached(&SLOT, &OLE32_W, true, b"CoCreateFreeThreadedMarshaler\0")
            else {
                if !ppunk_marshal.is_null() {
                    *ppunk_marshal = ptr::null_mut();
                }
                return E_NOINTERFACE;
            };
            let func: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                core::mem::transmute(addr);
            func(punk_outer, ppunk_marshal)
        }
    );

    define_shim!(
        CoTaskMemAlloc,
        "__imp_CoTaskMemAlloc",
        IMP_CO_TASK_MEM_ALLOC,
        (cb: usize) -> *mut c_void {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoTaskMemAlloc\0") else {
                return ptr::null_mut();
            };
            let func: unsafe extern "system" fn(usize) -> *mut c_void = core::mem::transmute(addr);
            func(cb)
        }
    );

    define_shim!(
        CoTaskMemRealloc,
        "__imp_CoTaskMemRealloc",
        IMP_CO_TASK_MEM_REALLOC,
        (pv: *const c_void, cb: usize) -> *mut c_void {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoTaskMemRealloc\0") else {
                return ptr::null_mut();
            };
            let func: unsafe extern "system" fn(*const c_void, usize) -> *mut c_void =
                core::mem::transmute(addr);
            func(pv, cb)
        }
    );

    define_shim!(
        CoTaskMemFree,
        "__imp_CoTaskMemFree",
        IMP_CO_TASK_MEM_FREE,
        (pv: *const c_void) {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CoTaskMemFree\0") {
                let func: unsafe extern "system" fn(*const c_void) = core::mem::transmute(addr);
                func(pv);
            }
        }
    );

    define_shim!(
        StringFromIID,
        "__imp_StringFromIID",
        IMP_STRING_FROM_IID,
        (rclsid: *const GUID, lplpsz: *mut *mut u16) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"StringFromIID\0") else {
                if !lplpsz.is_null() {
                    *lplpsz = ptr::null_mut();
                }
                return E_OUTOFMEMORY;
            };
            let func: unsafe extern "system" fn(*const GUID, *mut *mut u16) -> HRESULT =
                core::mem::transmute(addr);
            func(rclsid, lplpsz)
        }
    );

    define_shim!(
        CLSIDFromProgID,
        "__imp_CLSIDFromProgID",
        IMP_CLSID_FROM_PROG_ID,
        (lpsz_prog_id: *const u16, lpclsid: *mut GUID) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"CLSIDFromProgID\0") else {
                return REGDB_E_CLASSNOTREG;
            };
            let func: unsafe extern "system" fn(*const u16, *mut GUID) -> HRESULT =
                core::mem::transmute(addr);
            func(lpsz_prog_id, lpclsid)
        }
    );

    define_shim!(
        PropVariantClear,
        "__imp_PropVariantClear",
        IMP_PROP_VARIANT_CLEAR,
        (pvar: *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"PropVariantClear\0") else {
                return S_OK;
            };
            let func: unsafe extern "system" fn(*mut c_void) -> HRESULT = core::mem::transmute(addr);
            func(pvar)
        }
    );

    define_shim!(
        PropVariantCopy,
        "__imp_PropVariantCopy",
        IMP_PROP_VARIANT_COPY,
        (pvar_dest: *mut c_void, pvar_src: *const c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            let Some(addr) = resolve_cached(&SLOT, &OLE32_W, true, b"PropVariantCopy\0") else {
                return E_NOTIMPL;
            };
            let func: unsafe extern "system" fn(*mut c_void, *const c_void) -> HRESULT =
                core::mem::transmute(addr);
            func(pvar_dest, pvar_src)
        }
    );

    define_shim!(
        RoInitialize,
        "__imp_RoInitialize",
        IMP_RO_INITIALIZE,
        (init_type: i32) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"RoInitialize\0") {
                let func: unsafe extern "system" fn(i32) -> HRESULT = core::mem::transmute(addr);
                return func(init_type);
            }
            let coinit = if init_type == 0 { 2u32 } else { 0u32 };
            CoInitializeEx(ptr::null(), coinit)
        }
    );

    define_shim!(
        RoUninitialize,
        "__imp_RoUninitialize",
        IMP_RO_UNINITIALIZE,
        () {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"RoUninitialize\0") {
                let func: unsafe extern "system" fn() = core::mem::transmute(addr);
                func();
                return;
            }
            CoUninitialize();
        }
    );

    define_shim!(
        RoActivateInstance,
        "__imp_RoActivateInstance",
        IMP_RO_ACTIVATE_INSTANCE,
        (activatable_class_id: *mut c_void, instance: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"RoActivateInstance\0") {
                let func: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(activatable_class_id, instance);
            }
            if !instance.is_null() {
                *instance = ptr::null_mut();
            }
            REGDB_E_CLASSNOTREG
        }
    );

    define_shim!(
        SetRestrictedErrorInfo,
        "__imp_SetRestrictedErrorInfo",
        IMP_SET_RESTRICTED_ERROR_INFO,
        (restricted_error_info: *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &COMBASE_W, true, b"SetRestrictedErrorInfo\0")
            {
                let func: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(restricted_error_info);
            }
            S_OK
        }
    );

    define_shim!(
        GetRestrictedErrorInfo,
        "__imp_GetRestrictedErrorInfo",
        IMP_GET_RESTRICTED_ERROR_INFO,
        (restricted_error_info: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &COMBASE_W, true, b"GetRestrictedErrorInfo\0")
            {
                let func: unsafe extern "system" fn(*mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(restricted_error_info);
            }
            if !restricted_error_info.is_null() {
                *restricted_error_info = ptr::null_mut();
            }
            1
        }
    );

    define_shim!(
        RoCaptureErrorContext,
        "__imp_RoCaptureErrorContext",
        IMP_RO_CAPTURE_ERROR_CONTEXT,
        (hr: HRESULT) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"RoCaptureErrorContext\0")
            {
                let func: unsafe extern "system" fn(HRESULT) -> HRESULT =
                    core::mem::transmute(addr);
                return func(hr);
            }
            S_OK
        }
    );

    define_shim!(
        WindowsCreateString,
        "__imp_WindowsCreateString",
        IMP_WINDOWS_CREATE_STRING,
        (source_string: *const u16, length: u32, string: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsCreateString\0") {
                let func: unsafe extern "system" fn(*const u16, u32, *mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(source_string, length, string);
            }
            if string.is_null() {
                return E_INVALIDARG;
            }
            match hstring_alloc(source_string, length) {
                Ok(ptr_val) => {
                    *string = ptr_val;
                    S_OK
                }
                Err(hr) => {
                    *string = ptr::null_mut();
                    hr
                }
            }
        }
    );

    define_shim!(
        WindowsCreateStringReference,
        "__imp_WindowsCreateStringReference",
        IMP_WINDOWS_CREATE_STRING_REFERENCE,
        (
            source_string: *const u16,
            length: u32,
            hstring_header: *mut c_void,
            string: *mut *mut c_void,
        ) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsCreateStringReference\0")
            {
                let func: unsafe extern "system" fn(
                    *const u16,
                    u32,
                    *mut c_void,
                    *mut *mut c_void,
                ) -> HRESULT = core::mem::transmute(addr);
                return func(source_string, length, hstring_header, string);
            }
            if string.is_null() || hstring_header.is_null() {
                return E_INVALIDARG;
            }
            if source_string.is_null() {
                *string = ptr::null_mut();
                return E_POINTER;
            }
            if *source_string.add(length as usize) != 0 {
                *string = ptr::null_mut();
                return E_INVALIDARG;
            }
            if length == 0 {
                *string = ptr::null_mut();
                return S_OK;
            }
            let hdr = hstring_header.cast::<Win7HStringRefHeader>();
            hdr.write(Win7HStringRefHeader {
                flags: HSTRING_REFERENCE_FLAG,
                len: length,
                padding1: 0,
                padding2: 0,
                data: source_string,
            });
            *string = hstring_header;
            S_OK
        }
    );

    define_shim!(
        WindowsDeleteString,
        "__imp_WindowsDeleteString",
        IMP_WINDOWS_DELETE_STRING,
        (string: *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsDeleteString\0") {
                let func: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(string);
            }
            if string.is_null() {
                return S_OK;
            }
            let hdr = string.cast::<Win7HStringHeader>();
            if ((*hdr).flags & HSTRING_REFERENCE_FLAG) == 0
                && (*hdr).count.fetch_sub(1, Ordering::AcqRel) == 1
            {
                let _ = HeapFree(GetProcessHeap(), 0, string);
            }
            S_OK
        }
    );

    define_shim!(
        WindowsDuplicateString,
        "__imp_WindowsDuplicateString",
        IMP_WINDOWS_DUPLICATE_STRING,
        (string: *mut c_void, new_string: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsDuplicateString\0")
            {
                let func: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(string, new_string);
            }
            if new_string.is_null() {
                return E_INVALIDARG;
            }
            if string.is_null() {
                *new_string = ptr::null_mut();
                return S_OK;
            }
            let hdr = string.cast::<Win7HStringHeader>();
            if ((*hdr).flags & HSTRING_REFERENCE_FLAG) == 0 {
                (*hdr).count.fetch_add(1, Ordering::Relaxed);
                *new_string = string;
                S_OK
            } else {
                match hstring_alloc((*hdr).data, (*hdr).len) {
                    Ok(copy) => {
                        *new_string = copy;
                        S_OK
                    }
                    Err(hr) => {
                        *new_string = ptr::null_mut();
                        hr
                    }
                }
            }
        }
    );

    define_shim!(
        WindowsGetStringRawBuffer,
        "__imp_WindowsGetStringRawBuffer",
        IMP_WINDOWS_GET_STRING_RAW_BUFFER,
        (string: *mut c_void, length: *mut u32) -> *const u16 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsGetStringRawBuffer\0")
            {
                let func: unsafe extern "system" fn(*mut c_void, *mut u32) -> *const u16 =
                    core::mem::transmute(addr);
                return func(string, length);
            }
            static EMPTY_WIDE: u16 = 0;
            if string.is_null() {
                if !length.is_null() {
                    *length = 0;
                }
                return ptr::addr_of!(EMPTY_WIDE);
            }
            let hdr = string.cast::<Win7HStringRefHeader>();
            if !length.is_null() {
                *length = (*hdr).len;
            }
            (*hdr).data
        }
    );

    define_shim!(
        WindowsGetStringLen,
        "__imp_WindowsGetStringLen",
        IMP_WINDOWS_GET_STRING_LEN,
        (string: *mut c_void) -> u32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsGetStringLen\0") {
                let func: unsafe extern "system" fn(*mut c_void) -> u32 =
                    core::mem::transmute(addr);
                return func(string);
            }
            if string.is_null() {
                0
            } else {
                (*string.cast::<Win7HStringRefHeader>()).len
            }
        }
    );

    define_shim!(
        WindowsIsStringEmpty,
        "__imp_WindowsIsStringEmpty",
        IMP_WINDOWS_IS_STRING_EMPTY,
        (string: *mut c_void) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &COMBASE_W, true, b"WindowsIsStringEmpty\0") {
                let func: unsafe extern "system" fn(*mut c_void) -> BOOL =
                    core::mem::transmute(addr);
                return func(string);
            }
            if string.is_null() {
                1
            } else {
                i32::from((*string.cast::<Win7HStringRefHeader>()).len == 0)
            }
        }
    );

    define_shim!(
        ActivateAudioInterfaceAsync,
        "__imp_ActivateAudioInterfaceAsync",
        IMP_ACTIVATE_AUDIO_INTERFACE_ASYNC,
        (
            device_interface_path: *const u16,
            riid: *const GUID,
            activation_params: *const c_void,
            completion_handler: *mut c_void,
            activation_operation: *mut *mut c_void,
        ) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &MMDEVAPI_W, true, b"ActivateAudioInterfaceAsync\0")
            {
                let func: unsafe extern "system" fn(
                    *const u16,
                    *const GUID,
                    *const c_void,
                    *mut c_void,
                    *mut *mut c_void,
                ) -> HRESULT = core::mem::transmute(addr);
                return func(
                    device_interface_path,
                    riid,
                    activation_params,
                    completion_handler,
                    activation_operation,
                );
            }
            if !activation_operation.is_null() {
                *activation_operation = ptr::null_mut();
            }
            E_NOTIMPL
        }
    );

    define_shim!(
        SetProcessDpiAwarenessContext,
        "__imp_SetProcessDpiAwarenessContext",
        IMP_SET_PROCESS_DPI_AWARENESS_CONTEXT,
        (value: isize) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &USER32_W, false, b"SetProcessDpiAwarenessContext\0")
            {
                let func: unsafe extern "system" fn(isize) -> BOOL = core::mem::transmute(addr);
                return func(value);
            }
            SetProcessDPIAware()
        }
    );

    define_shim!(
        SetProcessDpiAwareness,
        "__imp_SetProcessDpiAwareness",
        IMP_SET_PROCESS_DPI_AWARENESS,
        (value: i32) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &SHCORE_W, true, b"SetProcessDpiAwareness\0")
            {
                let func: unsafe extern "system" fn(i32) -> HRESULT = core::mem::transmute(addr);
                return func(value);
            }
            if value != 0 {
                let _ = SetProcessDPIAware();
            }
            S_OK
        }
    );

    define_shim!(
        AdjustWindowRectExForDpi,
        "__imp_AdjustWindowRectExForDpi",
        IMP_ADJUST_WINDOW_RECT_EX_FOR_DPI,
        (lp_rect: *mut RECT, dw_style: u32, b_menu: BOOL, dw_ex_style: u32, dpi: u32) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &USER32_W, false, b"AdjustWindowRectExForDpi\0")
            {
                let func: unsafe extern "system" fn(*mut RECT, u32, BOOL, u32, u32) -> BOOL =
                    core::mem::transmute(addr);
                return func(lp_rect, dw_style, b_menu, dw_ex_style, dpi);
            }
            AdjustWindowRectEx(lp_rect, dw_style, b_menu, dw_ex_style)
        }
    );

    define_shim!(
        EnableNonClientDpiScaling,
        "__imp_EnableNonClientDpiScaling",
        IMP_ENABLE_NON_CLIENT_DPI_SCALING,
        (hwnd: HWND) -> BOOL {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) =
                resolve_cached(&SLOT, &USER32_W, false, b"EnableNonClientDpiScaling\0")
            {
                let func: unsafe extern "system" fn(HWND) -> BOOL = core::mem::transmute(addr);
                return func(hwnd);
            }
            1
        }
    );

    define_shim!(
        GetDpiForSystem,
        "__imp_GetDpiForSystem",
        IMP_GET_DPI_FOR_SYSTEM,
        () -> u32 {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(&SLOT, &USER32_W, false, b"GetDpiForSystem\0") {
                let func: unsafe extern "system" fn() -> u32 = core::mem::transmute(addr);
                return func();
            }
            system_screen_dpi()
        }
    );

    define_shim!(
        CreateDispatcherQueueController,
        "__imp_CreateDispatcherQueueController",
        IMP_CREATE_DISPATCHER_QUEUE_CONTROLLER,
        (options: DispatcherQueueOptionsAbi, controller: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(
                &SLOT,
                &COREMESSAGING_W,
                true,
                b"CreateDispatcherQueueController\0",
            ) {
                let func: unsafe extern "system" fn(
                    DispatcherQueueOptionsAbi,
                    *mut *mut c_void,
                ) -> HRESULT = core::mem::transmute(addr);
                return func(options, controller);
            }
            if !controller.is_null() {
                *controller = ptr::null_mut();
            }
            E_NOTIMPL
        }
    );

    define_shim!(
        CreateDirect3D11DeviceFromDXGIDevice,
        "__imp_CreateDirect3D11DeviceFromDXGIDevice",
        IMP_CREATE_DIRECT3D11_DEVICE_FROM_DXGI_DEVICE,
        (dxgi_device: *mut c_void, graphics_device: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(
                &SLOT,
                &D3D11_W,
                true,
                b"CreateDirect3D11DeviceFromDXGIDevice\0",
            ) {
                let func: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(dxgi_device, graphics_device);
            }
            if !graphics_device.is_null() {
                *graphics_device = ptr::null_mut();
            }
            E_NOINTERFACE
        }
    );

    define_shim!(
        CreateDirect3D11SurfaceFromDXGISurface,
        "__imp_CreateDirect3D11SurfaceFromDXGISurface",
        IMP_CREATE_DIRECT3D11_SURFACE_FROM_DXGI_SURFACE,
        (dxgi_surface: *mut c_void, graphics_surface: *mut *mut c_void) -> HRESULT {
            static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
            if let Some(addr) = resolve_cached(
                &SLOT,
                &D3D11_W,
                true,
                b"CreateDirect3D11SurfaceFromDXGISurface\0",
            ) {
                let func: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                    core::mem::transmute(addr);
                return func(dxgi_surface, graphics_surface);
            }
            if !graphics_surface.is_null() {
                *graphics_surface = ptr::null_mut();
            }
            E_NOINTERFACE
        }
    );

    fn shim_addr_for_import_name(name: &[u8]) -> Option<usize> {
        match name {
            b"DWriteCreateFactory" => Some(dwrite::IMP_DWRITE_CREATE_FACTORY as usize),
            b"D3D11CreateDevice" => Some(IMP_D3D11_CREATE_DEVICE as usize),
            b"CreateDXGIFactory2" => Some(IMP_CREATE_DXGI_FACTORY_2 as usize),
            b"DCompositionCreateDevice" => Some(IMP_DCOMPOSITION_CREATE_DEVICE as usize),
            b"DXGIGetDebugInterface1" => Some(IMP_DXGI_GET_DEBUG_INTERFACE_1 as usize),
            b"CoCreateInstance" => Some(IMP_CO_CREATE_INSTANCE as usize),
            b"CoInitializeEx" => Some(IMP_CO_INITIALIZE_EX as usize),
            b"CoInitialize" => Some(IMP_CO_INITIALIZE as usize),
            b"CoUninitialize" => Some(IMP_CO_UNINITIALIZE as usize),
            b"CoCreateFreeThreadedMarshaler" => Some(IMP_CO_CREATE_FREE_THREADED_MARSHALER as usize),
            b"CoIncrementMTAUsage" => Some(IMP_CO_INCREMENT_MTA_USAGE as usize),
            b"GetCurrentPackageFullName" => Some(IMP_GET_CURRENT_PACKAGE_FULL_NAME as usize),
            b"GetDpiForMonitor" => Some(IMP_GET_DPI_FOR_MONITOR as usize),
            b"GetDpiForWindow" => Some(IMP_GET_DPI_FOR_WINDOW as usize),
            b"GetDpiForSystem" => Some(IMP_GET_DPI_FOR_SYSTEM as usize),
            b"GetSystemMetricsForDpi" => Some(IMP_GET_SYSTEM_METRICS_FOR_DPI as usize),
            b"AdjustWindowRectExForDpi" => Some(IMP_ADJUST_WINDOW_RECT_EX_FOR_DPI as usize),
            b"EnableNonClientDpiScaling" => Some(IMP_ENABLE_NON_CLIENT_DPI_SCALING as usize),
            b"SetProcessDpiAwareness" => Some(IMP_SET_PROCESS_DPI_AWARENESS as usize),
            b"SetProcessDpiAwarenessContext" => {
                Some(IMP_SET_PROCESS_DPI_AWARENESS_CONTEXT as usize)
            }
            b"GetPointerType" => Some(IMP_GET_POINTER_TYPE as usize),
            b"RegisterSuspendResumeNotification" => {
                Some(IMP_REGISTER_SUSPEND_RESUME_NOTIFICATION as usize)
            }
            b"UnregisterSuspendResumeNotification" => {
                Some(IMP_UNREGISTER_SUSPEND_RESUME_NOTIFICATION as usize)
            }
            b"UiaGetReservedNotSupportedValue" => {
                Some(IMP_UIA_GET_RESERVED_NOT_SUPPORTED_VALUE as usize)
            }
            b"UiaGetReservedMixedAttributeValue" => {
                Some(IMP_UIA_GET_RESERVED_MIXED_ATTRIBUTE_VALUE as usize)
            }
            b"WaitOnAddress" => Some(IMP_WAIT_ON_ADDRESS as usize),
            b"WakeByAddressSingle" => Some(IMP_WAKE_BY_ADDRESS_SINGLE as usize),
            b"WakeByAddressAll" => Some(IMP_WAKE_BY_ADDRESS_ALL as usize),
            b"ProcessPrng" => Some(IMP_PROCESS_PRNG as usize),
            b"GetTempPath2W" => Some(IMP_GET_TEMP_PATH_2W as usize),
            b"GetSystemTimePreciseAsFileTime" => {
                Some(IMP_GET_SYSTEM_TIME_PRECISE_AS_FILE_TIME as usize)
            }
            b"GetHostNameW" => Some(IMP_GET_HOST_NAME_W as usize),
            b"u_strlen" => Some(IMP_U_STRLEN as usize),
            _ => None,
        }
    }

    unsafe fn patch_main_module_iat() {
        const PAGE_READWRITE: u32 = 0x04;
        let base = unsafe { GetModuleHandleW(ptr::null()) } as *mut u8;
        if base.is_null() {
            return;
        }
        let dos_magic = unsafe { ptr::read_unaligned(base as *const u16) };
        if dos_magic != 0x5A4D {
            return;
        }
        let lfanew = unsafe { ptr::read_unaligned(base.add(0x3C) as *const i32) };
        if lfanew <= 0 || (lfanew as usize) > 0x10_0000 {
            return;
        }
        let nt = unsafe { base.add(lfanew as usize) };
        if unsafe { ptr::read_unaligned(nt as *const u32) } != 0x0000_4550 {
            return;
        }
        let opt = unsafe { nt.add(24) };
        if unsafe { ptr::read_unaligned(opt as *const u16) } != 0x020B {
            return;
        }
        let size_of_image = unsafe { ptr::read_unaligned(opt.add(56) as *const u32) } as usize;
        let num_dirs = unsafe { ptr::read_unaligned(opt.add(108) as *const u32) };
        if num_dirs <= 1 || size_of_image < 0x1000 {
            return;
        }
        let import_rva = unsafe { ptr::read_unaligned(opt.add(112 + 8) as *const u32) } as usize;
        if import_rva == 0 || import_rva + 20 > size_of_image {
            return;
        }
        let mut desc_rva = import_rva;
        while desc_rva + 20 <= size_of_image {
            let desc = unsafe { base.add(desc_rva) };
            let orig_thunk = unsafe { ptr::read_unaligned(desc as *const u32) } as usize;
            let name_rva = unsafe { ptr::read_unaligned(desc.add(12) as *const u32) } as usize;
            let first_thunk = unsafe { ptr::read_unaligned(desc.add(16) as *const u32) } as usize;
            if orig_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }
            if orig_thunk != 0 && first_thunk != 0 {
                let mut idx = 0usize;
                loop {
                    let int_pos = orig_thunk.saturating_add(idx.saturating_mul(8));
                    let iat_pos = first_thunk.saturating_add(idx.saturating_mul(8));
                    if int_pos + 8 > size_of_image || iat_pos + 8 > size_of_image {
                        break;
                    }
                    let entry = unsafe { ptr::read_unaligned(base.add(int_pos) as *const u64) };
                    if entry == 0 {
                        break;
                    }
                    if (entry & (1u64 << 63)) == 0 {
                        let hint_name = (entry & 0x7FFF_FFFF) as usize;
                        if hint_name + 3 < size_of_image {
                            let str_start = hint_name + 2;
                            let max_len = (size_of_image - str_start).min(256);
                            let raw_bytes = unsafe {
                                core::slice::from_raw_parts(base.add(str_start), max_len)
                            };
                            if let Some(nul) = raw_bytes.iter().position(|&b| b == 0) {
                                if let Some(shim_fn) = shim_addr_for_import_name(&raw_bytes[..nul])
                                {
                                    let slot = unsafe { base.add(iat_pos) as *mut usize };
                                    if unsafe { *slot } != shim_fn {
                                        let mut old_prot = 0u32;
                                        if unsafe {
                                            VirtualProtect(
                                                slot.cast(),
                                                size_of::<usize>(),
                                                PAGE_READWRITE,
                                                &mut old_prot,
                                            )
                                        } != 0
                                        {
                                            unsafe { *slot = shim_fn };
                                            let mut dummy = 0u32;
                                            let _ = unsafe {
                                                VirtualProtect(
                                                    slot.cast(),
                                                    size_of::<usize>(),
                                                    old_prot,
                                                    &mut dummy,
                                                )
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }
                    idx += 1;
                }
            }
            desc_rva += 20;
        }
    }

    static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);

    fn write_crash_log(report: &str) -> String {
        let candidates = [
            std::env::current_exe().ok().and_then(|p| {
                p.parent()
                    .map(|dir| dir.join("horizon-launcher-crash.log"))
            }),
            Some(std::env::temp_dir().join("horizon-launcher-crash.log")),
        ];
        for path in candidates.into_iter().flatten() {
            if std::fs::write(&path, report).is_ok() {
                return path.display().to_string();
            }
        }
        String::from("horizon-launcher-crash.log")
    }

    fn is_interactive_gui() -> bool {
        std::env::args_os().len() <= 1 && std::env::var_os("CI").is_none()
    }

    fn show_error_dialog(title: &str, body: &str) {
        if !is_interactive_gui() {
            return;
        }
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut body_w: Vec<u16> = body.encode_utf16().collect();
        body_w.push(0);
        unsafe {
            let _ = MessageBoxW(0, body_w.as_ptr(), title_w.as_ptr(), 0x0001_0010);
        }
    }

    fn format_d3d11_diag_snapshot() -> String {
        d3d11_diag_log()
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    unsafe fn describe_address_module(addr: *const c_void) -> String {
        const FLAGS: u32 = 0x0000_0002 | 0x0000_0004; // UNCHANGED_REFCOUNT | FROM_ADDRESS
        let mut hmod: HMODULE = 0;
        if addr.is_null()
            || unsafe { GetModuleHandleExW(FLAGS, addr as *const u16, &mut hmod) } == 0
            || hmod == 0
        {
            return format!("0x{:016x}", addr as usize);
        }
        let mut buf = [0u16; 512];
        let len = unsafe { GetModuleFileNameW(hmod, buf.as_mut_ptr(), buf.len() as u32) } as usize;
        let name = if len > 0 && len < buf.len() {
            let full = String::from_utf16_lossy(&buf[..len]);
            full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string()
        } else {
            String::from("<module>")
        };
        let rva = (addr as usize).wrapping_sub(hmod as usize);
        format!("{name}+0x{rva:x} (0x{:016x})", addr as usize)
    }

    unsafe extern "system" fn unhandled_seh_filter(info: *const EXCEPTION_POINTERS) -> i32 {
        if info.is_null() {
            return 0;
        }
        let rec_ptr = unsafe { (*info).ExceptionRecord };
        if rec_ptr.is_null() {
            return 0;
        }
        let code = unsafe { (*rec_ptr).ExceptionCode };
        // Let C++/Rust unwind exceptions and debug/thread-name notifications pass through.
        if code < 0x8000_0000 || code == 0xE06D_7363 {
            return 0;
        }
        let addr = unsafe { (*rec_ptr).ExceptionAddress };
        let where_str = unsafe { describe_address_module(addr) };
        let (maj, min, bld, sp) = os_version_info();
        let extra = if code == 0xC000_0005 && unsafe { (*rec_ptr).NumberParameters } >= 2 {
            let op = unsafe { (*rec_ptr).ExceptionInformation[0] };
            let target = unsafe { (*rec_ptr).ExceptionInformation[1] };
            format!("access_violation(op={op}, target=0x{target:016x})")
        } else {
            String::new()
        };
        let diag = format_d3d11_diag_snapshot();
        let report = format!(
            "Horizon Launcher Legacy Crash Report (SEH)\n\
             OS: Windows {maj}.{min}.{bld} SP{sp} (legacy_build={IS_LEGACY_BUILD})\n\
             Exception: 0x{code:08x} at {where_str} {extra}\n\
             FirstFramePresented: {}\n\
             D3D11Diagnostics:\n{diag}\n",
            FIRST_FRAME_PRESENTED.load(Ordering::Acquire) != 0,
        );
        let log_path = write_crash_log(&report);
        show_error_dialog(
            "Horizon Launcher — Startup Error",
            &format!(
                "Horizon Launcher encountered an unexpected native error on startup.\n\n\
                 Exception: 0x{code:08x} at {where_str}\n\
                 {extra}\n\n\
                 Details were written to:\n{log_path}"
            ),
        );
        1 // EXCEPTION_EXECUTE_HANDLER (prevents WerFault.exe hang)
    }

    fn install_crash_diagnostics() {
        static INSTALLED: AtomicU32 = AtomicU32::new(0);
        if INSTALLED.swap(1, Ordering::AcqRel) != 0 {
            return;
        }
        MAIN_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        unsafe {
            SetErrorMode(0x0001 | 0x0002);
            let _ = SetUnhandledExceptionFilter(Some(unhandled_seh_filter));
        }
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            if unsafe { GetCurrentThreadId() } != MAIN_THREAD_ID.load(Ordering::Acquire) {
                prev_hook(panic_info);
                return;
            }
            let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                String::from("unknown panic payload")
            };
            let location = panic_info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| String::from("<unknown>"));
            let (maj, min, bld, sp) = os_version_info();
            let diag = format_d3d11_diag_snapshot();
            let bt = std::backtrace::Backtrace::force_capture();
            let report = format!(
                "Horizon Launcher Legacy Crash Report (Panic)\n\
                 OS: Windows {maj}.{min}.{bld} SP{sp} (legacy_build={IS_LEGACY_BUILD})\n\
                 Location: {location}\n\
                 Message: {payload}\n\
                 FirstFramePresented: {}\n\
                 D3D11Diagnostics:\n{diag}\n\
                 Backtrace:\n{bt}\n",
                FIRST_FRAME_PRESENTED.load(Ordering::Acquire) != 0,
            );
            let log_path = write_crash_log(&report);
            let needs_d3d11_hint = D3D11_STRUCTURED_OK.load(Ordering::Acquire) == 0
                && (payload.contains("DirectX")
                    || payload.contains("graphics adapter")
                    || payload.contains("StructuredBuffer")
                    || payload.contains("window"));
            if needs_d3d11_hint {
                show_error_dialog(
                    "Horizon Launcher — DirectX 11 Update Needed (Windows 7)",
                    &format!(
                        "Your graphics driver does not support Direct3D 11 Shader Model 4.1 StructuredBuffer, \
                         and Windows 7's built-in software renderer (d3d10warp.dll) is not updated.\n\n\
                         To fix this on 64-bit Windows 7 SP1, do EITHER of the following:\n\
                         1. Install Microsoft's Platform Update for Windows 7 (KB2670838) and restart, OR\n\
                         2. Copy 64-bit d3d10warp.dll (for example from mesa3d x64) into the same folder as horizon-launcher-legacy.exe.\n\n\
                         Technical details saved to:\n{log_path}"
                    ),
                );
            } else {
                show_error_dialog(
                    "Horizon Launcher — Startup Error",
                    &format!(
                        "Horizon Launcher stopped with an error:\n\n\
                         {payload}\n({location})\n\n\
                         Details were written to:\n{log_path}"
                    ),
                );
            }
            prev_hook(panic_info);
            if FIRST_FRAME_PRESENTED.load(Ordering::Acquire) == 0 && is_interactive_gui() {
                unsafe {
                    let _ = TerminateProcess(GetCurrentProcess(), 101);
                }
            }
        }));
    }

    pub fn init() {
        static INIT_ONCE: AtomicU32 = AtomicU32::new(0);
        let first_init = INIT_ONCE.swap(1, Ordering::AcqRel) == 0;
        let refs: &[usize] = &[
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
            ptr::addr_of!(IMP_CREATE_DXGI_FACTORY_2) as usize,
            dwrite::shim_anchor(),
            ptr::addr_of!(IMP_CO_INITIALIZE_EX) as usize,
            ptr::addr_of!(IMP_CO_INITIALIZE) as usize,
            ptr::addr_of!(IMP_CO_UNINITIALIZE) as usize,
            ptr::addr_of!(IMP_CO_CREATE_FREE_THREADED_MARSHALER) as usize,
            ptr::addr_of!(IMP_CO_TASK_MEM_ALLOC) as usize,
            ptr::addr_of!(IMP_CO_TASK_MEM_REALLOC) as usize,
            ptr::addr_of!(IMP_CO_TASK_MEM_FREE) as usize,
            ptr::addr_of!(IMP_STRING_FROM_IID) as usize,
            ptr::addr_of!(IMP_CLSID_FROM_PROG_ID) as usize,
            ptr::addr_of!(IMP_PROP_VARIANT_CLEAR) as usize,
            ptr::addr_of!(IMP_PROP_VARIANT_COPY) as usize,
            ptr::addr_of!(IMP_RO_INITIALIZE) as usize,
            ptr::addr_of!(IMP_RO_UNINITIALIZE) as usize,
            ptr::addr_of!(IMP_RO_ACTIVATE_INSTANCE) as usize,
            ptr::addr_of!(IMP_SET_RESTRICTED_ERROR_INFO) as usize,
            ptr::addr_of!(IMP_GET_RESTRICTED_ERROR_INFO) as usize,
            ptr::addr_of!(IMP_RO_CAPTURE_ERROR_CONTEXT) as usize,
            ptr::addr_of!(IMP_WINDOWS_CREATE_STRING) as usize,
            ptr::addr_of!(IMP_WINDOWS_CREATE_STRING_REFERENCE) as usize,
            ptr::addr_of!(IMP_WINDOWS_DELETE_STRING) as usize,
            ptr::addr_of!(IMP_WINDOWS_DUPLICATE_STRING) as usize,
            ptr::addr_of!(IMP_WINDOWS_GET_STRING_RAW_BUFFER) as usize,
            ptr::addr_of!(IMP_WINDOWS_GET_STRING_LEN) as usize,
            ptr::addr_of!(IMP_WINDOWS_IS_STRING_EMPTY) as usize,
            ptr::addr_of!(IMP_ACTIVATE_AUDIO_INTERFACE_ASYNC) as usize,
            ptr::addr_of!(IMP_SET_PROCESS_DPI_AWARENESS_CONTEXT) as usize,
            ptr::addr_of!(IMP_SET_PROCESS_DPI_AWARENESS) as usize,
            ptr::addr_of!(IMP_ADJUST_WINDOW_RECT_EX_FOR_DPI) as usize,
            ptr::addr_of!(IMP_ENABLE_NON_CLIENT_DPI_SCALING) as usize,
            ptr::addr_of!(IMP_GET_DPI_FOR_SYSTEM) as usize,
            ptr::addr_of!(IMP_CREATE_DISPATCHER_QUEUE_CONTROLLER) as usize,
            ptr::addr_of!(IMP_CREATE_DIRECT3D11_DEVICE_FROM_DXGI_DEVICE) as usize,
            ptr::addr_of!(IMP_CREATE_DIRECT3D11_SURFACE_FROM_DXGI_SURFACE) as usize,
        ];
        core::hint::black_box(refs);
        if first_init {
            unsafe { patch_main_module_iat() };
        }
        if (IS_LEGACY_BUILD || !has_dcomp())
            && std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none()
        {
            std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");
        }
        #[cfg(not(test))]
        if first_init && (IS_LEGACY_BUILD || !is_win10_or_later()) {
            install_crash_diagnostics();
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
        assert_eq!(size_of::<IDXGIAdapter_Vtbl>(), 10 * word);
        assert_eq!(size_of::<IDXGIAdapter1_Vtbl>(), 11 * word);
        assert_eq!(size_of::<IDXGIDevice_Vtbl>(), 12 * word);
        if word == 8 {
            assert_eq!(size_of::<DXGI_ADAPTER_DESC>(), 304);
            assert_eq!(size_of::<DXGI_ADAPTER_DESC1>(), 312);
        }
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
        assert_eq!(size_of::<ID3D11DeviceChild_Vtbl>(), 7 * word);
        assert_eq!(size_of::<ID3D11Resource_Vtbl>(), 10 * word);
        assert_eq!(size_of::<ID3D11Buffer_Vtbl>(), 11 * word);
        assert_eq!(size_of::<ID3D11Device_Vtbl>(), 43 * word);
        assert_eq!(size_of::<D3D11_BUFFER_DESC>(), 24);
        assert_eq!(size_of::<D3D11_TEXTURE2D_DESC>(), 44);
        assert_eq!(size_of::<D3D11_SHADER_RESOURCE_VIEW_DESC>(), 24);
    }

    fn build_synthetic_rdef_dxbc(target_ver: u16, entries: &[(&[u8], u32, u32)]) -> Vec<u8> {
        let stride = if target_ver >= 0x0501 { 40usize } else { 32usize };
        let res_offset = 28usize;
        let strings_start = res_offset + entries.len() * stride;
        let mut body = vec![0u8; strings_start];
        body[8..12].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        body[12..16].copy_from_slice(&(res_offset as u32).to_le_bytes());
        body[16..18].copy_from_slice(&target_ver.to_le_bytes());

        for (i, &(name, res_type, bind_point)) in entries.iter().enumerate() {
            let name_off = body.len() as u32;
            body.extend_from_slice(name);
            body.push(0);
            let rec = res_offset + i * stride;
            body[rec..rec + 4].copy_from_slice(&name_off.to_le_bytes());
            body[rec + 4..rec + 8].copy_from_slice(&res_type.to_le_bytes());
            body[rec + 20..rec + 24].copy_from_slice(&bind_point.to_le_bytes());
            body[rec + 24..rec + 28].copy_from_slice(&1u32.to_le_bytes());
        }

        let total_size = 44 + body.len();
        let mut dxbc = Vec::with_capacity(total_size);
        dxbc.extend_from_slice(b"DXBC");
        dxbc.extend_from_slice(&[0u8; 16]);
        dxbc.extend_from_slice(&1u32.to_le_bytes());
        dxbc.extend_from_slice(&(total_size as u32).to_le_bytes());
        dxbc.extend_from_slice(&1u32.to_le_bytes());
        dxbc.extend_from_slice(&36u32.to_le_bytes());
        dxbc.extend_from_slice(b"RDEF");
        dxbc.extend_from_slice(&(body.len() as u32).to_le_bytes());
        dxbc.extend_from_slice(&body);
        dxbc
    }

    #[test]
    fn dxbc_structured_t1_name_parses_rdef_chunks() {
        assert_eq!(dxbc_structured_t1_name(b""), None);
        assert_eq!(dxbc_structured_t1_name(b"DXBCshort"), None);

        for name in [
            &b"quads"[..],
            &b"shadows"[..],
            &b"path_rasterization_sprites"[..],
            &b"path_sprites"[..],
            &b"underlines"[..],
            &b"mono_sprites"[..],
            &b"poly_sprites"[..],
        ] {
            let sm41 = build_synthetic_rdef_dxbc(
                0x0401,
                &[(b"fast_clip_shapes", 2, 15), (name, 5, 1), (b"t_sprite", 2, 0)],
            );
            assert_eq!(dxbc_structured_t1_name(&sm41), Some(name));

            let sm51 = build_synthetic_rdef_dxbc(0x0501, &[(name, 5, 1)]);
            assert_eq!(dxbc_structured_t1_name(&sm51), Some(name));
        }

        // Typed Buffer<uint2> (`D3D_SIT_TBUFFER`/`D3D_SIT_TEXTURE`, not `D3D_SIT_STRUCTURED = 5`)
        // or structured buffers at registers other than t1 must NOT match.
        let typed_buf = build_synthetic_rdef_dxbc(0x0401, &[(b"d3d10_instance_buf", 2, 1)]);
        assert_eq!(dxbc_structured_t1_name(&typed_buf), None);
        let other_slot = build_synthetic_rdef_dxbc(0x0401, &[(b"quads", 5, 2)]);
        assert_eq!(dxbc_structured_t1_name(&other_slot), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn win7_d3d10_shaders_and_device_wrapper_work_on_warp_fl10_1() {
        use super::imp::{
            WIN7_MONO_SPRITE_VS, WIN7_PATH_RASTERIZATION_PS, WIN7_PATH_RASTERIZATION_VS,
            WIN7_PATH_SPRITE_VS, WIN7_POLY_SPRITE_PS, WIN7_POLY_SPRITE_VS, WIN7_QUAD_PS,
            WIN7_QUAD_VS, WIN7_SHADOW_PS, WIN7_SHADOW_VS, WIN7_UNDERLINE_PS, WIN7_UNDERLINE_VS,
            raw_d3d11_create_device, wrap_d3d10_device,
        };

        let vs_blobs: &[(&[u8], &[u8])] = &[
            (b"quads", WIN7_QUAD_VS),
            (b"shadows", WIN7_SHADOW_VS),
            (b"path_rasterization_sprites", WIN7_PATH_RASTERIZATION_VS),
            (b"path_sprites", WIN7_PATH_SPRITE_VS),
            (b"underlines", WIN7_UNDERLINE_VS),
            (b"mono_sprites", WIN7_MONO_SPRITE_VS),
            (b"poly_sprites", WIN7_POLY_SPRITE_VS),
        ];
        let ps_blobs: &[(&[u8], &[u8])] = &[
            (b"quads", WIN7_QUAD_PS),
            (b"shadows", WIN7_SHADOW_PS),
            (b"path_rasterization_sprites", WIN7_PATH_RASTERIZATION_PS),
            (b"underlines", WIN7_UNDERLINE_PS),
            (b"poly_sprites", WIN7_POLY_SPRITE_PS),
        ];

        for &(_, blob) in vs_blobs.iter().chain(ps_blobs.iter()) {
            assert!(blob.len() > 64 && &blob[..4] == b"DXBC");
            assert_eq!(dxbc_structured_t1_name(blob), None);
        }

        unsafe {
            let create = raw_d3d11_create_device().expect("System32 d3d11.dll!D3D11CreateDevice");
            let fl_10_1: u32 = 0xa100;
            let mut raw_dev: *mut c_void = ptr::null_mut();
            let mut chosen_fl: u32 = 0;
            let mut imm_ctx: *mut c_void = ptr::null_mut();
            let hr = create(
                ptr::null_mut(),
                5,    // D3D_DRIVER_TYPE_WARP
                0,
                0x20, // D3D11_CREATE_DEVICE_BGRA_SUPPORT
                &fl_10_1,
                1,
                7,    // D3D11_SDK_VERSION
                &mut raw_dev,
                &mut chosen_fl,
                &mut imm_ctx,
            );
            assert!(hr >= 0 && !raw_dev.is_null(), "WARP FL 10_1 failed: 0x{:08x}", hr as u32);
            assert_eq!(chosen_fl, fl_10_1);

            let dev = wrap_d3d10_device(raw_dev);
            let vtbl = &*(*(dev as *const *const ID3D11Device_Vtbl));

            let mut hw_opts: i32 = 0;
            let hr_feat = (vtbl.CheckFeatureSupport)(dev, 4, ptr::from_mut(&mut hw_opts).cast(), 4);
            assert!(hr_feat >= 0 && hw_opts == 1);

            for &(t1_name, _) in vs_blobs {
                let fake_sb_vs = build_synthetic_rdef_dxbc(0x0401, &[(t1_name, 5, 1)]);
                let mut vs: *mut c_void = ptr::null_mut();
                let hr_vs = (vtbl.CreateVertexShader)(
                    dev,
                    fake_sb_vs.as_ptr().cast(),
                    fake_sb_vs.len(),
                    ptr::null_mut(),
                    &mut vs,
                );
                assert!(
                    hr_vs >= 0 && !vs.is_null(),
                    "VS replacement for {:?} failed on FL 10_1: 0x{:08x}",
                    core::str::from_utf8(t1_name),
                    hr_vs as u32
                );
                ((*(*(vs as *const *const IUnknown_Vtbl))).Release)(vs);
            }

            for &(t1_name, _) in ps_blobs {
                let fake_sb_ps = build_synthetic_rdef_dxbc(
                    0x0401,
                    &[(b"fast_clip_shapes", 2, 15), (t1_name, 5, 1)],
                );
                let mut ps: *mut c_void = ptr::null_mut();
                let hr_ps = (vtbl.CreatePixelShader)(
                    dev,
                    fake_sb_ps.as_ptr().cast(),
                    fake_sb_ps.len(),
                    ptr::null_mut(),
                    &mut ps,
                );
                assert!(
                    hr_ps >= 0 && !ps.is_null(),
                    "PS replacement for {:?} failed on FL 10_1: 0x{:08x}",
                    core::str::from_utf8(t1_name),
                    hr_ps as u32
                );
                ((*(*(ps as *const *const IUnknown_Vtbl))).Release)(ps);
            }

            // Validate gpui-fast's exact structured instance buffer + NULL-desc SRV sequence on FL 10_1.
            let bdesc = D3D11_BUFFER_DESC {
                ByteWidth: 128 * 160,
                Usage: 2,              // D3D11_USAGE_DYNAMIC
                BindFlags: 0x8,        // D3D11_BIND_SHADER_RESOURCE
                CPUAccessFlags: 0x10000, // D3D11_CPU_ACCESS_WRITE
                MiscFlags: 0x40,       // D3D11_RESOURCE_MISC_BUFFER_STRUCTURED
                StructureByteStride: 160,
            };
            let mut buf: *mut c_void = ptr::null_mut();
            let hr_buf = (vtbl.CreateBuffer)(dev, &bdesc, ptr::null(), &mut buf);
            assert!(hr_buf >= 0 && !buf.is_null(), "CreateBuffer failed: 0x{:08x}", hr_buf as u32);

            let mut srv: *mut c_void = ptr::null_mut();
            let hr_srv = (vtbl.CreateShaderResourceView)(dev, buf, ptr::null(), &mut srv);
            assert!(hr_srv >= 0 && !srv.is_null(), "CreateSRV failed: 0x{:08x}", hr_srv as u32);

            // Validate MSAA 4x path intermediate texture creation on FL 10_1.
            let tdesc = D3D11_TEXTURE2D_DESC {
                Width: 256,
                Height: 256,
                MipLevels: 1,
                ArraySize: 1,
                Format: 87, // DXGI_FORMAT_B8G8R8A8_UNORM
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 4,
                    Quality: 0xffff_ffff,
                },
                Usage: 0,
                BindFlags: 0x20, // D3D11_BIND_RENDER_TARGET
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut tex: *mut c_void = ptr::null_mut();
            let hr_tex = (vtbl.CreateTexture2D)(dev, &tdesc, ptr::null(), &mut tex);
            assert!(hr_tex >= 0 && !tex.is_null(), "CreateTexture2D MSAA failed: 0x{:08x}", hr_tex as u32);

            ((*(*(tex as *const *const IUnknown_Vtbl))).Release)(tex);
            ((*(*(srv as *const *const IUnknown_Vtbl))).Release)(srv);
            ((*(*(buf as *const *const IUnknown_Vtbl))).Release)(buf);
            ((*(*(imm_ctx as *const *const IUnknown_Vtbl))).Release)(imm_ctx);
            (vtbl.base.Release)(dev);
        }
    }
}
