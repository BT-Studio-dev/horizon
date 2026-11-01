//! DirectWrite 1.0 -> DirectWrite 1.5 COM compatibility adapter for Windows 7.
//!
//! When `dwrite.dll` natively implements `IDWriteFactory5` (Windows 10 1703+
//! and Windows 11), `DWriteCreateFactory` returns the OS factory directly.
//! On Windows 7 (and Windows 8 / early Windows 10), `DWriteCreateFactory`
//! opens the native `IDWriteFactory` and wraps it in `Win7DWriteFactory5`,
//! bridging in-memory font loading, custom font sets, `IDWriteFontCollection1`,
//! `IDWriteFontFace3`, `IDWriteTextFormat1`, `IDWriteTextLayout`,
//! `IDWriteRenderingParams1`, and grayscale `IDWriteGlyphRunAnalysis`.

use super::com_abi::*;
use super::imp::{
    DWRITE_W, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_POINTER, IID_IUNKNOWN, S_OK, StaticCom,
    UNRESOLVED, com_add_ref, com_qi, com_release, com_vtbl, resolve_cached, static_unk_add_ref,
    static_unk_release,
};
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const DWRITE_E_NOCOLOR: HRESULT = 0x8898_500Cu32 as i32;
const DWRITE_RENDERING_MODE_ALIASED: i32 = 1;
const DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC: i32 = 5;
const DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE: i32 = 1;
const DWRITE_TEXTURE_ALIASED_1X1: i32 = 0;
const DWRITE_TEXTURE_CLEARTYPE_3X1: i32 = 1;

const IID_IDWRITE_FACTORY: GUID = GUID::from_u128(0xb859ee5a_d838_4b5b_a2e8_1adc7d93db48);
const IID_IDWRITE_FACTORY1: GUID = GUID::from_u128(0x30572f99_dac6_41db_a16e_0486307e606a);
const IID_IDWRITE_FACTORY2: GUID = GUID::from_u128(0x0439fc60_ca44_4994_8dee_3a9af7b732ec);
const IID_IDWRITE_FACTORY3: GUID = GUID::from_u128(0x9a1b41c3_d3bb_466a_87fc_fe67556a3b65);
const IID_IDWRITE_FACTORY4: GUID = GUID::from_u128(0x4b0b5bd3_0797_4549_8ac5_fe915cc53856);
const IID_IDWRITE_FACTORY5: GUID = GUID::from_u128(0x958db99a_be2a_4f09_af7d_65189803d1d3);

const IID_IDWRITE_FONT_COLLECTION: GUID = GUID::from_u128(0xa84cee02_3eea_4eee_a827_87c1a02a0fcc);
const IID_IDWRITE_FONT_COLLECTION1: GUID = GUID::from_u128(0x53585141_d9f8_4095_8321_d73cf6bd116c);
const IID_IDWRITE_FONT_COLLECTION_LOADER: GUID =
    GUID::from_u128(0xcca920e4_52f0_492b_bfa8_29c72ee0a468);
const IID_IDWRITE_FONT_FILE_ENUMERATOR: GUID =
    GUID::from_u128(0x72755049_5ff7_435d_8348_4be97cfa6c7c);
const IID_IDWRITE_FONT_FILE_LOADER: GUID = GUID::from_u128(0x727cad4e_d6af_4c9e_8a08_d695b11caa49);
const IID_IDWRITE_IN_MEMORY_FONT_FILE_LOADER: GUID =
    GUID::from_u128(0xdc102f47_a12d_4b1c_822d_9e117e33043f);
const IID_IDWRITE_FONT_FILE_STREAM: GUID = GUID::from_u128(0x6d4865fe_0ab8_4d91_8f62_5dd6be34a3e0);
const IID_IDWRITE_FONT_SET: GUID = GUID::from_u128(0x53585141_d9f8_4095_8321_d73cf6bd116b);
const IID_IDWRITE_FONT_SET_BUILDER: GUID = GUID::from_u128(0x2f642afe_9c68_4f40_b8be_457401afcb3d);
const IID_IDWRITE_FONT_SET_BUILDER1: GUID =
    GUID::from_u128(0x3ff7715f_3cdc_4dc6_9b72_ec5621dccafd);
const IID_IDWRITE_FONT_LIST: GUID = GUID::from_u128(0x1a0d8438_1d97_4ec1_aef9_a2fb86ed6acb);
const IID_IDWRITE_FONT_FAMILY: GUID = GUID::from_u128(0xda20d8ef_812a_4c43_9802_62ec4abd7add);
const IID_IDWRITE_FONT_FAMILY1: GUID = GUID::from_u128(0xda20d8ef_812a_4c43_9802_62ec4abd7adf);
const IID_IDWRITE_FONT: GUID = GUID::from_u128(0xacd16696_8c14_4f5d_877e_fe3fc1d32737);
const IID_IDWRITE_FONT_FACE: GUID = GUID::from_u128(0x5f49804d_7024_4d43_bfa9_d25984f53849);
const IID_IDWRITE_FONT_FACE1: GUID = GUID::from_u128(0xa71efdb4_9fdb_4838_ad90_cfc3be8c3daf);
const IID_IDWRITE_FONT_FACE2: GUID = GUID::from_u128(0xd8b768ff_64bc_4e66_982b_ec8e87f693f7);
const IID_IDWRITE_FONT_FACE3: GUID = GUID::from_u128(0xd37d7598_09be_4222_a236_2081341cc1f2);
const IID_IDWRITE_FONT_FACE_REFERENCE: GUID =
    GUID::from_u128(0x5e7fa7ca_dde3_424c_89f0_9fcd6fed58cd);
const IID_IDWRITE_TEXT_FORMAT: GUID = GUID::from_u128(0x9c906818_31d7_4fd3_a151_7c5e225db55a);
const IID_IDWRITE_TEXT_FORMAT1: GUID = GUID::from_u128(0x5f174b49_0d8b_4cfb_8bca_f1cce9d06c67);
const IID_IDWRITE_TEXT_LAYOUT: GUID = GUID::from_u128(0x53737037_6d14_410b_9bfe_0b182bb70961);
const IID_IDWRITE_PIXEL_SNAPPING: GUID = GUID::from_u128(0xeaf3a2da_ecf4_4d24_b644_b34f6842024b);
const IID_IDWRITE_TEXT_RENDERER: GUID = GUID::from_u128(0xef8a8135_5cc6_45fe_8825_c5a0724eb819);
const IID_IDWRITE_RENDERING_PARAMS: GUID = GUID::from_u128(0x2f0da53a_2add_47cd_82ee_d9ec34688e75);
const IID_IDWRITE_RENDERING_PARAMS1: GUID =
    GUID::from_u128(0x94413cf4_a6fc_4248_8b50_6674348fcad3);
const IID_IDWRITE_GLYPH_RUN_ANALYSIS: GUID =
    GUID::from_u128(0x7d97dbf7_e085_42d4_81e3_6a883bded118);
const IID_IDWRITE_FONT_FALLBACK: GUID = GUID::from_u128(0xefa008f9_f7a1_48bf_b05c_f224713cc0ff);
const IID_IDWRITE_FONT_FALLBACK_BUILDER: GUID =
    GUID::from_u128(0xfd882d06_8aba_4fb8_b849_8be8b73e14de);

#[derive(Clone, Copy)]
struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

// -----------------------------------------------------------------------------
// In-memory font file loader & stream
// -----------------------------------------------------------------------------

#[repr(C)]
struct Win7FontFileStream {
    vtbl: *const IDWriteFontFileStream_Vtbl,
    refs: AtomicU32,
    blob: Arc<[u8]>,
}

unsafe extern "system" fn ffs_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_FILE_STREAM {
        unsafe { ffs_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn ffs_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7FontFileStream)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn ffs_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7FontFileStream) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe { drop(Box::from_raw(this as *mut Win7FontFileStream)) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn ffs_read(
    this: *mut c_void,
    frag_start: *mut *const c_void,
    offset: u64,
    size: u64,
    frag_ctx: *mut *mut c_void,
) -> HRESULT {
    if frag_start.is_null() || frag_ctx.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontFileStream) };
    let len = me.blob.len() as u64;
    let Some(end) = offset.checked_add(size) else {
        return E_INVALIDARG;
    };
    if end > len {
        unsafe {
            *frag_start = ptr::null();
            *frag_ctx = ptr::null_mut();
        }
        return E_INVALIDARG;
    }
    unsafe {
        *frag_start = me.blob.as_ptr().add(offset as usize).cast();
        *frag_ctx = ptr::null_mut();
    }
    S_OK
}
unsafe extern "system" fn ffs_release_frag(_: *mut c_void, _: *mut c_void) {}
unsafe extern "system" fn ffs_get_size(this: *mut c_void, out: *mut u64) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontFileStream) };
    unsafe { *out = me.blob.len() as u64 };
    S_OK
}
unsafe extern "system" fn ffs_get_time(_: *mut c_void, out: *mut u64) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = 0 };
    S_OK
}

static FONT_FILE_STREAM_VTBL: IDWriteFontFileStream_Vtbl = IDWriteFontFileStream_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: ffs_qi,
        AddRef: ffs_add_ref,
        Release: ffs_release,
    },
    ReadFileFragment: ffs_read,
    ReleaseFileFragment: ffs_release_frag,
    GetFileSize: ffs_get_size,
    GetLastWriteTime: ffs_get_time,
};

#[repr(C)]
struct Win7InMemoryFontFileLoader {
    vtbl: *const IDWriteInMemoryFontFileLoader_Vtbl,
    refs: AtomicU32,
    blobs: Mutex<Vec<Arc<[u8]>>>,
}

unsafe extern "system" fn iml_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_FONT_FILE_LOADER
        || g == IID_IDWRITE_IN_MEMORY_FONT_FILE_LOADER
    {
        unsafe { iml_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn iml_add_ref(this: *mut c_void) -> u32 {
    unsafe {
        (*(this as *const Win7InMemoryFontFileLoader))
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}
unsafe extern "system" fn iml_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7InMemoryFontFileLoader) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe { drop(Box::from_raw(this as *mut Win7InMemoryFontFileLoader)) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn iml_create_stream(
    this: *mut c_void,
    key: *const c_void,
    key_size: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || key.is_null() || key_size != 4 {
        return E_INVALIDARG;
    }
    let idx = unsafe { ptr::read_unaligned(key as *const u32) } as usize;
    let me = unsafe { &*(this as *const Win7InMemoryFontFileLoader) };
    let guard = me.blobs.lock().unwrap_or_else(|e| e.into_inner());
    let Some(blob) = guard.get(idx).cloned() else {
        unsafe { *out = ptr::null_mut() };
        return E_INVALIDARG;
    };
    drop(guard);
    let stream = Box::into_raw(Box::new(Win7FontFileStream {
        vtbl: &FONT_FILE_STREAM_VTBL,
        refs: AtomicU32::new(1),
        blob,
    }));
    unsafe { *out = stream.cast() };
    S_OK
}
unsafe extern "system" fn iml_create_ref(
    this: *mut c_void,
    factory: *mut c_void,
    font_data: *const c_void,
    font_data_size: u32,
    _owner: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || factory.is_null() || font_data.is_null() || font_data_size == 0 {
        return E_INVALIDARG;
    }
    let raw_factory = unwrap_factory(factory);
    let bytes =
        unsafe { core::slice::from_raw_parts(font_data as *const u8, font_data_size as usize) };
    let arc: Arc<[u8]> = Arc::from(bytes);
    let me = unsafe { &*(this as *const Win7InMemoryFontFileLoader) };
    let mut guard = me.blobs.lock().unwrap_or_else(|e| e.into_inner());
    let key = guard.len() as u32;
    guard.push(arc);
    drop(guard);
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw_factory)).CreateCustomFontFileReference)(
            raw_factory,
            ptr::from_ref(&key).cast(),
            4,
            this,
            out,
        )
    }
}
unsafe extern "system" fn iml_get_count(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7InMemoryFontFileLoader) };
    me.blobs.lock().unwrap_or_else(|e| e.into_inner()).len() as u32
}

static IN_MEMORY_LOADER_VTBL: IDWriteInMemoryFontFileLoader_Vtbl =
    IDWriteInMemoryFontFileLoader_Vtbl {
        base: IDWriteFontFileLoader_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: iml_qi,
                AddRef: iml_add_ref,
                Release: iml_release,
            },
            CreateStreamFromKey: iml_create_stream,
        },
        CreateInMemoryFontFileReference: iml_create_ref,
        GetFileCount: iml_get_count,
    };

// -----------------------------------------------------------------------------
// Custom font collection loader & font set builder
// -----------------------------------------------------------------------------

fn collection_store() -> &'static Mutex<Vec<Vec<SendPtr>>> {
    static STORE: OnceLock<Mutex<Vec<Vec<SendPtr>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

#[repr(C)]
struct Win7FontFileEnumerator {
    vtbl: *const IDWriteFontFileEnumerator_Vtbl,
    refs: AtomicU32,
    files: Vec<SendPtr>,
    cursor: AtomicUsize,
}

impl Drop for Win7FontFileEnumerator {
    fn drop(&mut self) {
        for f in &self.files {
            unsafe { com_release(f.0) };
        }
    }
}

unsafe extern "system" fn ffe_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_FILE_ENUMERATOR {
        unsafe { ffe_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn ffe_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7FontFileEnumerator)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn ffe_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7FontFileEnumerator) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe { drop(Box::from_raw(this as *mut Win7FontFileEnumerator)) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn ffe_move_next(this: *mut c_void, has_current: *mut BOOL) -> HRESULT {
    if has_current.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontFileEnumerator) };
    let next = me.cursor.fetch_add(1, Ordering::AcqRel);
    unsafe { *has_current = if next < me.files.len() { 1 } else { 0 } };
    S_OK
}
unsafe extern "system" fn ffe_get_current(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontFileEnumerator) };
    let cur = me.cursor.load(Ordering::Acquire);
    if cur == 0 || cur > me.files.len() {
        unsafe { *out = ptr::null_mut() };
        return E_INVALIDARG;
    }
    let f = me.files[cur - 1].0;
    unsafe {
        com_add_ref(f);
        *out = f;
    }
    S_OK
}

static FONT_FILE_ENUMERATOR_VTBL: IDWriteFontFileEnumerator_Vtbl = IDWriteFontFileEnumerator_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: ffe_qi,
        AddRef: ffe_add_ref,
        Release: ffe_release,
    },
    MoveNext: ffe_move_next,
    GetCurrentFontFile: ffe_get_current,
};

unsafe extern "system" fn cl_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_COLLECTION_LOADER {
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn cl_create_enum(
    _: *mut c_void,
    _factory: *mut c_void,
    key: *const c_void,
    key_size: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || key.is_null() || key_size != 4 {
        return E_INVALIDARG;
    }
    let id = unsafe { ptr::read_unaligned(key as *const u32) } as usize;
    let guard = collection_store().lock().unwrap_or_else(|e| e.into_inner());
    let files: Vec<SendPtr> = guard
        .get(id)
        .map(|list| {
            for f in list {
                unsafe { com_add_ref(f.0) };
            }
            list.clone()
        })
        .unwrap_or_default();
    drop(guard);
    let enumerator = Box::into_raw(Box::new(Win7FontFileEnumerator {
        vtbl: &FONT_FILE_ENUMERATOR_VTBL,
        refs: AtomicU32::new(1),
        files,
        cursor: AtomicUsize::new(0),
    }));
    unsafe { *out = enumerator.cast() };
    S_OK
}

static COLLECTION_LOADER_VTBL: IDWriteFontCollectionLoader_Vtbl =
    IDWriteFontCollectionLoader_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: cl_qi,
            AddRef: static_unk_add_ref,
            Release: static_unk_release,
        },
        CreateEnumeratorFromKey: cl_create_enum,
    };

static GLOBAL_COLLECTION_LOADER: StaticCom<IDWriteFontCollectionLoader_Vtbl> = StaticCom {
    vtbl: &COLLECTION_LOADER_VTBL,
};

#[repr(C)]
struct Win7CustomFontSet {
    vtbl: *const IDWriteFontSet_Vtbl,
    refs: AtomicU32,
    files: Vec<SendPtr>,
}

impl Drop for Win7CustomFontSet {
    fn drop(&mut self) {
        for f in &self.files {
            unsafe { com_release(f.0) };
        }
    }
}

unsafe extern "system" fn cfs_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_SET {
        unsafe { cfs_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn cfs_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7CustomFontSet)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn cfs_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7CustomFontSet) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe { drop(Box::from_raw(this as *mut Win7CustomFontSet)) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn cfs_count(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7CustomFontSet)).files.len() as u32 }
}
unsafe extern "system" fn cfs_notimpl_ref(
    _: *mut c_void,
    _: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn fs_find_notimpl(
    _: *mut c_void,
    _: *mut c_void,
    idx: *mut u32,
    exists: *mut BOOL,
) -> HRESULT {
    if !idx.is_null() {
        unsafe { *idx = 0 };
    }
    if !exists.is_null() {
        unsafe { *exists = 0 };
    }
    S_OK
}
unsafe extern "system" fn fs_prop1(_: *mut c_void, _: i32, out: *mut *mut c_void) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn fs_prop2(
    _: *mut c_void,
    _: i32,
    _: *const u16,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn fs_prop3(
    _: *mut c_void,
    _: u32,
    _: i32,
    exists: *mut BOOL,
    out: *mut *mut c_void,
) -> HRESULT {
    if !exists.is_null() {
        unsafe { *exists = 0 };
    }
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn fs_occ(_: *mut c_void, _: *const c_void, cnt: *mut u32) -> HRESULT {
    if !cnt.is_null() {
        unsafe { *cnt = 0 };
    }
    S_OK
}
unsafe extern "system" fn cfs_match1(
    _: *mut c_void,
    _: *const u16,
    _: i32,
    _: i32,
    _: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn fs_match2(
    _: *mut c_void,
    _: *const c_void,
    _: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}

static CUSTOM_FONT_SET_VTBL: IDWriteFontSet_Vtbl = IDWriteFontSet_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: cfs_qi,
        AddRef: cfs_add_ref,
        Release: cfs_release,
    },
    GetFontCount: cfs_count,
    GetFontFaceReference: cfs_notimpl_ref,
    FindFontFaceReference: fs_find_notimpl,
    FindFontFace: fs_find_notimpl,
    GetPropertyValues: fs_prop1,
    GetPropertyValues2: fs_prop2,
    GetPropertyValues3: fs_prop3,
    GetPropertyOccurrenceCount: fs_occ,
    GetMatchingFonts: cfs_match1,
    GetMatchingFonts2: fs_match2,
};

#[repr(C)]
struct Win7FontSetBuilder1 {
    vtbl: *const IDWriteFontSetBuilder1_Vtbl,
    refs: AtomicU32,
    files: Mutex<Vec<SendPtr>>,
}

impl Drop for Win7FontSetBuilder1 {
    fn drop(&mut self) {
        let guard = self.files.lock().unwrap_or_else(|e| e.into_inner());
        for f in guard.iter() {
            unsafe { com_release(f.0) };
        }
    }
}

unsafe extern "system" fn fsb_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_FONT_SET_BUILDER
        || g == IID_IDWRITE_FONT_SET_BUILDER1
    {
        unsafe { fsb_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn fsb_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7FontSetBuilder1)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn fsb_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7FontSetBuilder1) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe { drop(Box::from_raw(this as *mut Win7FontSetBuilder1)) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn fsb_add_ref1(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: u32,
) -> HRESULT {
    S_OK
}
unsafe extern "system" fn fsb_add_ref2(_: *mut c_void, _: *mut c_void) -> HRESULT {
    S_OK
}
unsafe extern "system" fn fsb_create_set(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontSetBuilder1) };
    let guard = me.files.lock().unwrap_or_else(|e| e.into_inner());
    let cloned: Vec<SendPtr> = guard
        .iter()
        .map(|f| {
            unsafe { com_add_ref(f.0) };
            *f
        })
        .collect();
    drop(guard);
    let set = Box::into_raw(Box::new(Win7CustomFontSet {
        vtbl: &CUSTOM_FONT_SET_VTBL,
        refs: AtomicU32::new(1),
        files: cloned,
    }));
    unsafe { *out = set.cast() };
    S_OK
}
unsafe extern "system" fn fsb_add_file(this: *mut c_void, font_file: *mut c_void) -> HRESULT {
    if font_file.is_null() {
        return E_INVALIDARG;
    }
    unsafe { com_add_ref(font_file) };
    let me = unsafe { &*(this as *const Win7FontSetBuilder1) };
    me.files
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(SendPtr(font_file));
    S_OK
}

static FONT_SET_BUILDER1_VTBL: IDWriteFontSetBuilder1_Vtbl = IDWriteFontSetBuilder1_Vtbl {
    base: IDWriteFontSetBuilder_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: fsb_qi,
            AddRef: fsb_add_ref,
            Release: fsb_release,
        },
        AddFontFaceReference: fsb_add_ref1,
        AddFontFaceReference2: fsb_add_ref2,
        AddFontSet: fsb_add_ref2,
        CreateFontSet: fsb_create_set,
    },
    AddFontFile: fsb_add_file,
};

// -----------------------------------------------------------------------------
// Interned IDWriteFontFace3 wrapper
// -----------------------------------------------------------------------------

#[repr(C)]
struct Win7DWriteFontFace3 {
    vtbl: *const IDWriteFontFace3_Vtbl,
    refs: AtomicU32,
    raw_face: *mut c_void,
    raw_face1: *mut c_void,
    weight: AtomicU32,
    stretch: AtomicU32,
    style: AtomicU32,
    family_names: AtomicUsize,
    face_names: AtomicUsize,
}

fn face_intern_map() -> &'static Mutex<HashMap<usize, usize>> {
    static MAP: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn populate_face_meta_from_font(wrapper: &Win7DWriteFontFace3, raw_font: *mut c_void) {
    if raw_font.is_null() {
        return;
    }
    let fv = unsafe { &*com_vtbl::<IDWriteFont_Vtbl>(raw_font) };
    wrapper
        .weight
        .store(unsafe { (fv.GetWeight)(raw_font) } as u32, Ordering::Release);
    wrapper.stretch.store(
        unsafe { (fv.GetStretch)(raw_font) } as u32,
        Ordering::Release,
    );
    wrapper
        .style
        .store(unsafe { (fv.GetStyle)(raw_font) } as u32, Ordering::Release);
    if wrapper.face_names.load(Ordering::Acquire) == 0 {
        let mut names = ptr::null_mut();
        if unsafe { (fv.GetFaceNames)(raw_font, &mut names) } >= 0 && !names.is_null() {
            if wrapper
                .face_names
                .compare_exchange(0, names as usize, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                unsafe { com_release(names) };
            }
        }
    }
    if wrapper.family_names.load(Ordering::Acquire) == 0 {
        let mut fam = ptr::null_mut();
        if unsafe { (fv.GetFontFamily)(raw_font, &mut fam) } >= 0 && !fam.is_null() {
            let mut fnames = ptr::null_mut();
            if unsafe { ((*com_vtbl::<IDWriteFontFamily_Vtbl>(fam)).GetFamilyNames)(fam, &mut fnames) }
                >= 0
                && !fnames.is_null()
            {
                if wrapper
                    .family_names
                    .compare_exchange(0, fnames as usize, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    unsafe { com_release(fnames) };
                }
            }
            unsafe { com_release(fam) };
        }
    }
}

unsafe fn intern_font_face3(
    raw_face: *mut c_void,
    opt_raw_font: *mut c_void,
    raw_factory: *mut c_void,
) -> *mut c_void {
    if raw_face.is_null() {
        return ptr::null_mut();
    }
    if unsafe { *(raw_face as *const usize) } == ptr::addr_of!(FONT_FACE3_VTBL) as usize {
        unsafe { ff3_add_ref(raw_face) };
        return raw_face;
    }
    let key = raw_face as usize;
    let mut guard = face_intern_map().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&existing) = guard.get(&key) {
        let ptr = existing as *mut Win7DWriteFontFace3;
        unsafe {
            populate_face_meta_from_font(&*ptr, opt_raw_font);
            ff3_add_ref(ptr.cast());
        }
        return ptr.cast();
    }
    unsafe { com_add_ref(raw_face) };
    let raw_face1 =
        unsafe { com_qi(raw_face, &IID_IDWRITE_FONT_FACE1).unwrap_or(ptr::null_mut()) };
    let boxed = Box::into_raw(Box::new(Win7DWriteFontFace3 {
        vtbl: &FONT_FACE3_VTBL,
        refs: AtomicU32::new(2), // 1 kept in intern map + 1 returned to caller
        raw_face,
        raw_face1,
        weight: AtomicU32::new(400),
        stretch: AtomicU32::new(5),
        style: AtomicU32::new(0),
        family_names: AtomicUsize::new(0),
        face_names: AtomicUsize::new(0),
    }));
    guard.insert(key, boxed as usize);
    drop(guard);

    if !opt_raw_font.is_null() {
        unsafe { populate_face_meta_from_font(&*boxed, opt_raw_font) };
    } else if !raw_factory.is_null() {
        let mut sys_coll = ptr::null_mut();
        if unsafe {
            ((*com_vtbl::<IDWriteFactory_Vtbl>(raw_factory)).GetSystemFontCollection)(
                raw_factory,
                &mut sys_coll,
                0,
            )
        } >= 0
            && !sys_coll.is_null()
        {
            let mut found_font = ptr::null_mut();
            if unsafe {
                ((*com_vtbl::<IDWriteFontCollection_Vtbl>(sys_coll)).GetFontFromFontFace)(
                    sys_coll,
                    raw_face,
                    &mut found_font,
                )
            } >= 0
                && !found_font.is_null()
            {
                unsafe {
                    populate_face_meta_from_font(&*boxed, found_font);
                    com_release(found_font);
                }
            }
            unsafe { com_release(sys_coll) };
        }
    }
    boxed.cast()
}

#[inline]
unsafe fn unwrap_font_face(face: *mut c_void) -> *mut c_void {
    if !face.is_null()
        && unsafe { *(face as *const usize) } == ptr::addr_of!(FONT_FACE3_VTBL) as usize
    {
        unsafe { (*(face as *const Win7DWriteFontFace3)).raw_face }
    } else {
        face
    }
}

unsafe extern "system" fn ff3_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_FONT_FACE
        || g == IID_IDWRITE_FONT_FACE1
        || g == IID_IDWRITE_FONT_FACE2
        || g == IID_IDWRITE_FONT_FACE3
    {
        unsafe { ff3_add_ref(this) };
        unsafe { *out = this };
        return S_OK;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
}
unsafe extern "system" fn ff3_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteFontFace3)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn ff3_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteFontFace3) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteFontFace3) };
        let fn_ptr = boxed.family_names.load(Ordering::Acquire) as *mut c_void;
        let fcn_ptr = boxed.face_names.load(Ordering::Acquire) as *mut c_void;
        unsafe {
            com_release(fn_ptr);
            com_release(fcn_ptr);
            com_release(boxed.raw_face1);
            com_release(boxed.raw_face);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn ff3_get_type(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetType)(raw) }
}
unsafe extern "system" fn ff3_get_files(
    this: *mut c_void,
    num: *mut u32,
    files: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetFiles)(raw, num, files) }
}
unsafe extern "system" fn ff3_get_index(this: *mut c_void) -> u32 {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetIndex)(raw) }
}
unsafe extern "system" fn ff3_get_sims(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetSimulations)(raw) }
}
unsafe extern "system" fn ff3_is_sym(this: *mut c_void) -> BOOL {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).IsSymbolFont)(raw) }
}
unsafe extern "system" fn ff3_get_metrics(this: *mut c_void, out: *mut DWRITE_FONT_METRICS) {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetMetrics)(raw, out) };
}
unsafe extern "system" fn ff3_get_glyph_count(this: *mut c_void) -> u16 {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetGlyphCount)(raw) }
}
unsafe extern "system" fn ff3_get_design_gm(
    this: *mut c_void,
    indices: *const u16,
    count: u32,
    metrics: *mut c_void,
    sideways: BOOL,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetDesignGlyphMetrics)(
            raw, indices, count, metrics, sideways,
        )
    }
}
unsafe extern "system" fn ff3_get_glyph_indices(
    this: *mut c_void,
    codepoints: *const u32,
    count: u32,
    indices: *mut u16,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetGlyphIndices)(raw, codepoints, count, indices)
    }
}
unsafe extern "system" fn ff3_try_table(
    this: *mut c_void,
    tag: u32,
    data: *mut *const c_void,
    size: *mut u32,
    ctx: *mut *mut c_void,
    exists: *mut BOOL,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).TryGetFontTable)(raw, tag, data, size, ctx, exists)
    }
}
unsafe extern "system" fn ff3_rel_table(this: *mut c_void, ctx: *const c_void) {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).ReleaseFontTable)(raw, ctx) };
}
unsafe extern "system" fn ff3_outline(
    this: *mut c_void,
    em: f32,
    indices: *const u16,
    advances: *const f32,
    offsets: *const c_void,
    count: u32,
    sideways: BOOL,
    rtl: BOOL,
    sink: *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetGlyphRunOutline)(
            raw, em, indices, advances, offsets, count, sideways, rtl, sink,
        )
    }
}
unsafe extern "system" fn ff3_rec_mode0(
    this: *mut c_void,
    em: f32,
    ppd: f32,
    measure: i32,
    params: *mut c_void,
    mode: *mut i32,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    let raw_params = unwrap_rendering_params(params);
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetRecommendedRenderingMode)(
            raw, em, ppd, measure, raw_params, mode,
        )
    }
}
unsafe extern "system" fn ff3_gdi_metrics(
    this: *mut c_void,
    em: f32,
    ppd: f32,
    xform: *const DWRITE_MATRIX,
    out: *mut DWRITE_FONT_METRICS,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetGdiCompatibleMetrics)(raw, em, ppd, xform, out)
    }
}
unsafe extern "system" fn ff3_gdi_gm(
    this: *mut c_void,
    em: f32,
    ppd: f32,
    xform: *const DWRITE_MATRIX,
    gdi_nat: BOOL,
    indices: *const u16,
    count: u32,
    metrics: *mut c_void,
    sideways: BOOL,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFace3)).raw_face };
    unsafe {
        ((*com_vtbl::<IDWriteFontFace_Vtbl>(raw)).GetGdiCompatibleGlyphMetrics)(
            raw, em, ppd, xform, gdi_nat, indices, count, metrics, sideways,
        )
    }
}
unsafe extern "system" fn ff3_get_metrics1(this: *mut c_void, out: *mut DWRITE_FONT_METRICS1) {
    if out.is_null() {
        return;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontFace3) };
    if !me.raw_face1.is_null() {
        unsafe { ((*com_vtbl::<IDWriteFontFace1_Vtbl>(me.raw_face1)).GetMetrics1)(me.raw_face1, out) };
        return;
    }
    let mut base = DWRITE_FONT_METRICS::default();
    unsafe { ((*com_vtbl::<IDWriteFontFace_Vtbl>(me.raw_face)).GetMetrics)(me.raw_face, &mut base) };
    unsafe {
        *out = DWRITE_FONT_METRICS1 {
            base,
            glyphBoxLeft: 0,
            glyphBoxTop: base.ascent as i16,
            glyphBoxRight: base.designUnitsPerEm as i16,
            glyphBoxBottom: -(base.descent as i16),
            ..DWRITE_FONT_METRICS1::default()
        };
    }
}
unsafe extern "system" fn ff3_gdi_metrics1(
    this: *mut c_void,
    em: f32,
    ppd: f32,
    xform: *const DWRITE_MATRIX,
    out: *mut DWRITE_FONT_METRICS1,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = DWRITE_FONT_METRICS1::default() };
    unsafe { ff3_gdi_metrics(this, em, ppd, xform, &mut (*out).base) }
}
unsafe extern "system" fn ff3_caret(_: *mut c_void, caret: *mut c_void) {
    if !caret.is_null() {
        unsafe { ptr::write_bytes(caret as *mut u8, 0, 8) };
    }
}
unsafe extern "system" fn ff3_unicode_ranges(
    this: *mut c_void,
    max: u32,
    ranges: *mut c_void,
    actual: *mut u32,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7DWriteFontFace3) };
    if !me.raw_face1.is_null() {
        return unsafe {
            ((*com_vtbl::<IDWriteFontFace1_Vtbl>(me.raw_face1)).GetUnicodeRanges)(
                me.raw_face1,
                max,
                ranges,
                actual,
            )
        };
    }
    if !actual.is_null() {
        unsafe { *actual = 0 };
    }
    S_OK
}
unsafe extern "system" fn ff3_false(_: *mut c_void) -> BOOL {
    0
}
unsafe extern "system" fn ff3_adv_notimpl(
    _: *mut c_void,
    _: u32,
    _: *const u16,
    _: *mut i32,
    _: BOOL,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn ff3_gdi_adv_notimpl(
    _: *mut c_void,
    _: f32,
    _: f32,
    _: *const DWRITE_MATRIX,
    _: BOOL,
    _: BOOL,
    _: u32,
    _: *const u16,
    _: *mut i32,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn ff3_kern_notimpl(
    _: *mut c_void,
    _: u32,
    _: *const u16,
    _: *mut i32,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn ff3_rec_mode1(
    this: *mut c_void,
    em: f32,
    dpi_x: f32,
    _dpi_y: f32,
    _xform: *const DWRITE_MATRIX,
    _sideways: BOOL,
    _outline: i32,
    measure: i32,
    mode: *mut i32,
) -> HRESULT {
    let ppd = (dpi_x / 96.0).max(1.0);
    unsafe { ff3_rec_mode0(this, em, ppd, measure, ptr::null_mut(), mode) }
}
unsafe extern "system" fn ff3_vert_notimpl(
    _: *mut c_void,
    _: u32,
    _: *const u16,
    _: *mut u16,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn ff3_zero_u32(_: *mut c_void) -> u32 {
    0
}
unsafe extern "system" fn ff3_palette_notimpl(
    _: *mut c_void,
    _: u32,
    _: u32,
    _: u32,
    _: *mut c_void,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn ff3_rec_mode3(
    this: *mut c_void,
    em: f32,
    dpi_x: f32,
    _dpi_y: f32,
    xform: *const DWRITE_MATRIX,
    _sideways: BOOL,
    _outline: i32,
    measure: i32,
    params: *mut c_void,
    mode: *mut i32,
    grid_fit: *mut i32,
) -> HRESULT {
    if !grid_fit.is_null() {
        unsafe { *grid_fit = 0 };
    }
    let scale = if !xform.is_null() {
        unsafe { (*xform).m11.abs().max(1.0) }
    } else {
        1.0
    };
    let ppd = ((dpi_x / 96.0) * scale).max(1.0);
    unsafe { ff3_rec_mode0(this, em, ppd, measure, params, mode) }
}
unsafe extern "system" fn ff3_get_ref(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn ff3_panose(_: *mut c_void, out: *mut c_void) {
    if !out.is_null() {
        unsafe { ptr::write_bytes(out as *mut u8, 0, 10) };
    }
}
unsafe extern "system" fn ff3_weight(this: *mut c_void) -> i32 {
    unsafe { (*(this as *const Win7DWriteFontFace3)).weight.load(Ordering::Acquire) as i32 }
}
unsafe extern "system" fn ff3_stretch(this: *mut c_void) -> i32 {
    unsafe { (*(this as *const Win7DWriteFontFace3)).stretch.load(Ordering::Acquire) as i32 }
}
unsafe extern "system" fn ff3_style(this: *mut c_void) -> i32 {
    unsafe { (*(this as *const Win7DWriteFontFace3)).style.load(Ordering::Acquire) as i32 }
}
unsafe extern "system" fn ff3_family_names(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let ptr = unsafe {
        (*(this as *const Win7DWriteFontFace3))
            .family_names
            .load(Ordering::Acquire) as *mut c_void
    };
    if ptr.is_null() {
        unsafe { *out = ptr::null_mut() };
        return E_NOTIMPL;
    }
    unsafe {
        com_add_ref(ptr);
        *out = ptr;
    }
    S_OK
}
unsafe extern "system" fn ff3_face_names(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let ptr = unsafe {
        (*(this as *const Win7DWriteFontFace3))
            .face_names
            .load(Ordering::Acquire) as *mut c_void
    };
    if ptr.is_null() {
        unsafe { *out = ptr::null_mut() };
        return E_NOTIMPL;
    }
    unsafe {
        com_add_ref(ptr);
        *out = ptr;
    }
    S_OK
}
unsafe extern "system" fn ff3_info_str(
    _: *mut c_void,
    _: i32,
    out: *mut *mut c_void,
    exists: *mut BOOL,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    if !exists.is_null() {
        unsafe { *exists = 0 };
    }
    S_OK
}
unsafe extern "system" fn ff3_has_char(this: *mut c_void, ch: u32) -> BOOL {
    let mut glyph = 0u16;
    if unsafe { ff3_get_glyph_indices(this, &ch, 1, &mut glyph) } >= 0 && glyph != 0 {
        1
    } else {
        0
    }
}
unsafe extern "system" fn ff3_local_char(_: *mut c_void, _: u32) -> BOOL {
    1
}
unsafe extern "system" fn ff3_local_glyph(_: *mut c_void, _: u16) -> BOOL {
    1
}
unsafe extern "system" fn ff3_are_local(
    _: *mut c_void,
    _: *const u16,
    _: u32,
    _: BOOL,
    is_local: *mut BOOL,
) -> HRESULT {
    if !is_local.is_null() {
        unsafe { *is_local = 1 };
    }
    S_OK
}

static FONT_FACE3_VTBL: IDWriteFontFace3_Vtbl = IDWriteFontFace3_Vtbl {
    base: IDWriteFontFace2_Vtbl {
        base: IDWriteFontFace1_Vtbl {
            base: IDWriteFontFace_Vtbl {
                base: IUnknown_Vtbl {
                    QueryInterface: ff3_qi,
                    AddRef: ff3_add_ref,
                    Release: ff3_release,
                },
                GetType: ff3_get_type,
                GetFiles: ff3_get_files,
                GetIndex: ff3_get_index,
                GetSimulations: ff3_get_sims,
                IsSymbolFont: ff3_is_sym,
                GetMetrics: ff3_get_metrics,
                GetGlyphCount: ff3_get_glyph_count,
                GetDesignGlyphMetrics: ff3_get_design_gm,
                GetGlyphIndices: ff3_get_glyph_indices,
                TryGetFontTable: ff3_try_table,
                ReleaseFontTable: ff3_rel_table,
                GetGlyphRunOutline: ff3_outline,
                GetRecommendedRenderingMode: ff3_rec_mode0,
                GetGdiCompatibleMetrics: ff3_gdi_metrics,
                GetGdiCompatibleGlyphMetrics: ff3_gdi_gm,
            },
            GetMetrics1: ff3_get_metrics1,
            GetGdiCompatibleMetrics1: ff3_gdi_metrics1,
            GetCaretMetrics: ff3_caret,
            GetUnicodeRanges: ff3_unicode_ranges,
            IsMonospacedFont: ff3_false,
            GetDesignGlyphAdvances: ff3_adv_notimpl,
            GetGdiCompatibleGlyphAdvances: ff3_gdi_adv_notimpl,
            GetKerningPairAdjustments: ff3_kern_notimpl,
            HasKerningPairs: ff3_false,
            GetRecommendedRenderingMode1: ff3_rec_mode1,
            GetVerticalGlyphVariants: ff3_vert_notimpl,
            HasVerticalGlyphVariants: ff3_false,
        },
        IsColorFont: ff3_false,
        GetColorPaletteCount: ff3_zero_u32,
        GetPaletteEntryCount: ff3_zero_u32,
        GetPaletteEntries: ff3_palette_notimpl,
        GetRecommendedRenderingMode2: ff3_rec_mode3,
    },
    GetFontFaceReference: ff3_get_ref,
    GetPanose: ff3_panose,
    GetWeight: ff3_weight,
    GetStretch: ff3_stretch,
    GetStyle: ff3_style,
    GetFamilyNames: ff3_family_names,
    GetFaceNames: ff3_face_names,
    GetInformationalStrings: ff3_info_str,
    HasCharacter: ff3_has_char,
    GetRecommendedRenderingMode3: ff3_rec_mode3,
    IsCharacterLocal: ff3_local_char,
    IsGlyphLocal: ff3_local_glyph,
    AreCharactersLocal: ff3_are_local,
    AreGlyphsLocal: ff3_are_local,
};

// -----------------------------------------------------------------------------
// IDWriteFontFaceReference, IDWriteFont, IDWriteFontFamily1, IDWriteFontSet, IDWriteFontCollection1
// -----------------------------------------------------------------------------

#[repr(C)]
struct Win7FontFaceReference {
    vtbl: *const IDWriteFontFaceReference_Vtbl,
    refs: AtomicU32,
    raw_font: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe extern "system" fn ffr_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_FACE_REFERENCE {
        unsafe { ffr_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn ffr_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7FontFaceReference)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn ffr_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7FontFaceReference) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7FontFaceReference) };
        unsafe {
            com_release(boxed.raw_font);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn ffr_create_face(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7FontFaceReference) };
    let mut raw_face = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFont_Vtbl>(me.raw_font)).CreateFontFace)(me.raw_font, &mut raw_face)
    };
    if hr < 0 || raw_face.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let wrapped = unsafe { intern_font_face3(raw_face, me.raw_font, me.raw_factory) };
    unsafe {
        com_release(raw_face);
        *out = wrapped;
    }
    S_OK
}
unsafe extern "system" fn ffr_create_face_sim(
    this: *mut c_void,
    _sims: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe { ffr_create_face(this, out) }
}
unsafe extern "system" fn ffr_equals(this: *mut c_void, other: *mut c_void) -> BOOL {
    if this == other { 1 } else { 0 }
}
unsafe extern "system" fn ffr_u32_zero(_: *mut c_void) -> u32 {
    0
}
unsafe extern "system" fn ffr_i32_zero(_: *mut c_void) -> i32 {
    0
}
unsafe extern "system" fn ffr_font_file(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn ffr_u64_zero(_: *mut c_void) -> u64 {
    0
}
unsafe extern "system" fn ffr_ftime(_: *mut c_void, out: *mut FILETIME) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = FILETIME::default() };
    }
    S_OK
}
unsafe extern "system" fn ffr_locality(_: *mut c_void) -> i32 {
    2 // DWRITE_LOCALITY_LOCAL
}
unsafe extern "system" fn ffr_enq0(_: *mut c_void) -> HRESULT {
    S_OK
}
unsafe extern "system" fn ffr_enq_chars(_: *mut c_void, _: *const u16, _: u32) -> HRESULT {
    S_OK
}
unsafe extern "system" fn ffr_enq_frag(_: *mut c_void, _: u64, _: u64) -> HRESULT {
    S_OK
}

static FONT_FACE_REF_VTBL: IDWriteFontFaceReference_Vtbl = IDWriteFontFaceReference_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: ffr_qi,
        AddRef: ffr_add_ref,
        Release: ffr_release,
    },
    CreateFontFace: ffr_create_face,
    CreateFontFaceWithSimulations: ffr_create_face_sim,
    Equals: ffr_equals,
    GetFontFaceIndex: ffr_u32_zero,
    GetSimulations: ffr_i32_zero,
    GetFontFile: ffr_font_file,
    GetLocalFileSize: ffr_u64_zero,
    GetFileSize: ffr_u64_zero,
    GetFileTime: ffr_ftime,
    GetLocality: ffr_locality,
    EnqueueFontDownloadRequest: ffr_enq0,
    EnqueueCharacterDownloadRequest: ffr_enq_chars,
    EnqueueGlyphDownloadRequest: ffr_enq_chars,
    EnqueueFileFragmentDownloadRequest: ffr_enq_frag,
};

#[repr(C)]
struct Win7DWriteFont {
    vtbl: *const IDWriteFont_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe extern "system" fn wf_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT {
        unsafe { wf_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn wf_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteFont)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn wf_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteFont) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteFont) };
        unsafe {
            com_release(boxed.raw);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn wf_family(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let me = unsafe { &*(this as *const Win7DWriteFont) };
    let mut raw_fam = ptr::null_mut();
    let hr = unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(me.raw)).GetFontFamily)(me.raw, &mut raw_fam) };
    if hr < 0 || raw_fam.is_null() {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        return hr;
    }
    unsafe { *out = wrap_font_family1(raw_fam, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn wf_weight(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetWeight)(raw) }
}
unsafe extern "system" fn wf_stretch(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetStretch)(raw) }
}
unsafe extern "system" fn wf_style(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetStyle)(raw) }
}
unsafe extern "system" fn wf_is_sym(this: *mut c_void) -> BOOL {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).IsSymbolFont)(raw) }
}
unsafe extern "system" fn wf_face_names(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetFaceNames)(raw, out) }
}
unsafe extern "system" fn wf_info_str(
    this: *mut c_void,
    id: i32,
    out: *mut *mut c_void,
    exists: *mut BOOL,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetInformationalStrings)(raw, id, out, exists) }
}
unsafe extern "system" fn wf_sims(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetSimulations)(raw) }
}
unsafe extern "system" fn wf_metrics(this: *mut c_void, out: *mut DWRITE_FONT_METRICS) {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).GetMetrics)(raw, out) };
}
unsafe extern "system" fn wf_has_char(this: *mut c_void, ch: u32, exists: *mut BOOL) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFont)).raw };
    unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(raw)).HasCharacter)(raw, ch, exists) }
}
unsafe extern "system" fn wf_create_face(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFont) };
    let mut raw_face = ptr::null_mut();
    let hr = unsafe { ((*com_vtbl::<IDWriteFont_Vtbl>(me.raw)).CreateFontFace)(me.raw, &mut raw_face) };
    if hr < 0 || raw_face.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let wrapped = unsafe { intern_font_face3(raw_face, me.raw, me.raw_factory) };
    unsafe {
        com_release(raw_face);
        *out = wrapped;
    }
    S_OK
}

static FONT_VTBL: IDWriteFont_Vtbl = IDWriteFont_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: wf_qi,
        AddRef: wf_add_ref,
        Release: wf_release,
    },
    GetFontFamily: wf_family,
    GetWeight: wf_weight,
    GetStretch: wf_stretch,
    GetStyle: wf_style,
    IsSymbolFont: wf_is_sym,
    GetFaceNames: wf_face_names,
    GetInformationalStrings: wf_info_str,
    GetSimulations: wf_sims,
    GetMetrics: wf_metrics,
    HasCharacter: wf_has_char,
    CreateFontFace: wf_create_face,
};

unsafe fn wrap_font(raw_font: *mut c_void, raw_factory: *mut c_void) -> *mut c_void {
    unsafe { com_add_ref(raw_factory) };
    Box::into_raw(Box::new(Win7DWriteFont {
        vtbl: &FONT_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_font,
        raw_factory,
    }))
    .cast()
}

#[repr(C)]
struct Win7DWriteFontFamily1 {
    vtbl: *const IDWriteFontFamily1_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe fn wrap_font_family1(raw_fam: *mut c_void, raw_factory: *mut c_void) -> *mut c_void {
    unsafe { com_add_ref(raw_factory) };
    Box::into_raw(Box::new(Win7DWriteFontFamily1 {
        vtbl: &FONT_FAMILY1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_fam,
        raw_factory,
    }))
    .cast()
}

unsafe extern "system" fn fam_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_FONT_LIST
        || g == IID_IDWRITE_FONT_FAMILY
        || g == IID_IDWRITE_FONT_FAMILY1
    {
        unsafe { fam_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteFontFamily1)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn fam_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteFontFamily1)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn fam_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteFontFamily1) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteFontFamily1) };
        unsafe {
            com_release(boxed.raw);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn fam_get_coll(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontFamily1) };
    let mut raw_coll = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontList_Vtbl>(me.raw)).GetFontCollection)(me.raw, &mut raw_coll)
    };
    if hr < 0 || raw_coll.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_collection1(raw_coll, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn fam_count(this: *mut c_void) -> u32 {
    let raw = unsafe { (*(this as *const Win7DWriteFontFamily1)).raw };
    unsafe { ((*com_vtbl::<IDWriteFontList_Vtbl>(raw)).GetFontCount)(raw) }
}
unsafe extern "system" fn fam_get_font(
    this: *mut c_void,
    idx: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontFamily1) };
    let mut raw_font = ptr::null_mut();
    let hr =
        unsafe { ((*com_vtbl::<IDWriteFontList_Vtbl>(me.raw)).GetFont)(me.raw, idx, &mut raw_font) };
    if hr < 0 || raw_font.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_font(raw_font, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn fam_names(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFamily1)).raw };
    unsafe { ((*com_vtbl::<IDWriteFontFamily_Vtbl>(raw)).GetFamilyNames)(raw, out) }
}
unsafe extern "system" fn fam_first_match(
    this: *mut c_void,
    w: i32,
    s: i32,
    st: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontFamily1) };
    let mut raw_font = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontFamily_Vtbl>(me.raw)).GetFirstMatchingFont)(
            me.raw,
            w,
            s,
            st,
            &mut raw_font,
        )
    };
    if hr < 0 || raw_font.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_font(raw_font, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn fam_matching(
    this: *mut c_void,
    w: i32,
    s: i32,
    st: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontFamily1)).raw };
    unsafe { ((*com_vtbl::<IDWriteFontFamily_Vtbl>(raw)).GetMatchingFonts)(raw, w, s, st, out) }
}
unsafe extern "system" fn fam_locality(_: *mut c_void, _: u32) -> i32 {
    2
}
unsafe extern "system" fn fam_get_ref(
    this: *mut c_void,
    idx: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontFamily1) };
    let mut raw_font = ptr::null_mut();
    let hr =
        unsafe { ((*com_vtbl::<IDWriteFontList_Vtbl>(me.raw)).GetFont)(me.raw, idx, &mut raw_font) };
    if hr < 0 || raw_font.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(me.raw_factory) };
    let r = Box::into_raw(Box::new(Win7FontFaceReference {
        vtbl: &FONT_FACE_REF_VTBL,
        refs: AtomicU32::new(1),
        raw_font,
        raw_factory: me.raw_factory,
    }));
    unsafe { *out = r.cast() };
    S_OK
}

static FONT_FAMILY1_VTBL: IDWriteFontFamily1_Vtbl = IDWriteFontFamily1_Vtbl {
    base: IDWriteFontFamily_Vtbl {
        base: IDWriteFontList_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: fam_qi,
                AddRef: fam_add_ref,
                Release: fam_release,
            },
            GetFontCollection: fam_get_coll,
            GetFontCount: fam_count,
            GetFont: fam_get_font,
        },
        GetFamilyNames: fam_names,
        GetFirstMatchingFont: fam_first_match,
        GetMatchingFonts: fam_matching,
    },
    GetFontLocality: fam_locality,
    GetFont1: fam_get_font,
    GetFontFaceReference: fam_get_ref,
};

#[repr(C)]
struct Win7CollectionFontSet {
    vtbl: *const IDWriteFontSet_Vtbl,
    refs: AtomicU32,
    raw_collection: *mut c_void,
    raw_list: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe extern "system" fn colfs_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_SET {
        unsafe { colfs_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn colfs_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7CollectionFontSet)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn colfs_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7CollectionFontSet) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7CollectionFontSet) };
        unsafe {
            com_release(boxed.raw_collection);
            com_release(boxed.raw_list);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn colfs_count(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7CollectionFontSet) };
    if !me.raw_list.is_null() {
        return unsafe { ((*com_vtbl::<IDWriteFontList_Vtbl>(me.raw_list)).GetFontCount)(me.raw_list) };
    }
    0
}
unsafe extern "system" fn colfs_get_ref(
    this: *mut c_void,
    idx: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7CollectionFontSet) };
    if me.raw_list.is_null() {
        unsafe { *out = ptr::null_mut() };
        return E_INVALIDARG;
    }
    let mut raw_font = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontList_Vtbl>(me.raw_list)).GetFont)(me.raw_list, idx, &mut raw_font)
    };
    if hr < 0 || raw_font.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(me.raw_factory) };
    let r = Box::into_raw(Box::new(Win7FontFaceReference {
        vtbl: &FONT_FACE_REF_VTBL,
        refs: AtomicU32::new(1),
        raw_font,
        raw_factory: me.raw_factory,
    }));
    unsafe { *out = r.cast() };
    S_OK
}
unsafe extern "system" fn colfs_matching(
    this: *mut c_void,
    family_name: *const u16,
    weight: i32,
    stretch: i32,
    style: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || family_name.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7CollectionFontSet) };
    if me.raw_collection.is_null() {
        let empty = Box::into_raw(Box::new(Win7CollectionFontSet {
            vtbl: &COLLECTION_FONT_SET_VTBL,
            refs: AtomicU32::new(1),
            raw_collection: ptr::null_mut(),
            raw_list: ptr::null_mut(),
            raw_factory: ptr::null_mut(),
        }));
        unsafe { *out = empty.cast() };
        return S_OK;
    }
    let cv = unsafe { &*com_vtbl::<IDWriteFontCollection_Vtbl>(me.raw_collection) };
    let mut fam_idx = 0u32;
    let mut exists = 0i32;
    let hr = unsafe { (cv.FindFamilyName)(me.raw_collection, family_name, &mut fam_idx, &mut exists) };
    if hr < 0 || exists == 0 {
        let empty = Box::into_raw(Box::new(Win7CollectionFontSet {
            vtbl: &COLLECTION_FONT_SET_VTBL,
            refs: AtomicU32::new(1),
            raw_collection: ptr::null_mut(),
            raw_list: ptr::null_mut(),
            raw_factory: ptr::null_mut(),
        }));
        unsafe { *out = empty.cast() };
        return S_OK;
    }
    let mut raw_fam = ptr::null_mut();
    let hr = unsafe { (cv.GetFontFamily)(me.raw_collection, fam_idx, &mut raw_fam) };
    if hr < 0 || raw_fam.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let mut raw_list = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontFamily_Vtbl>(raw_fam)).GetMatchingFonts)(
            raw_fam,
            weight,
            stretch,
            style,
            &mut raw_list,
        )
    };
    unsafe { com_release(raw_fam) };
    if hr < 0 || raw_list.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(me.raw_factory) };
    let matched = Box::into_raw(Box::new(Win7CollectionFontSet {
        vtbl: &COLLECTION_FONT_SET_VTBL,
        refs: AtomicU32::new(1),
        raw_collection: ptr::null_mut(),
        raw_list,
        raw_factory: me.raw_factory,
    }));
    unsafe { *out = matched.cast() };
    S_OK
}

static COLLECTION_FONT_SET_VTBL: IDWriteFontSet_Vtbl = IDWriteFontSet_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: colfs_qi,
        AddRef: colfs_add_ref,
        Release: colfs_release,
    },
    GetFontCount: colfs_count,
    GetFontFaceReference: colfs_get_ref,
    FindFontFaceReference: fs_find_notimpl,
    FindFontFace: fs_find_notimpl,
    GetPropertyValues: fs_prop1,
    GetPropertyValues2: fs_prop2,
    GetPropertyValues3: fs_prop3,
    GetPropertyOccurrenceCount: fs_occ,
    GetMatchingFonts: colfs_matching,
    GetMatchingFonts2: fs_match2,
};

#[repr(C)]
struct Win7DWriteFontCollection1 {
    vtbl: *const IDWriteFontCollection1_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe fn wrap_collection1(raw_coll: *mut c_void, raw_factory: *mut c_void) -> *mut c_void {
    unsafe { com_add_ref(raw_factory) };
    Box::into_raw(Box::new(Win7DWriteFontCollection1 {
        vtbl: &COLLECTION1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_coll,
        raw_factory,
    }))
    .cast()
}

#[inline]
unsafe fn unwrap_collection(coll: *mut c_void) -> *mut c_void {
    if !coll.is_null()
        && unsafe { *(coll as *const usize) } == ptr::addr_of!(COLLECTION1_VTBL) as usize
    {
        unsafe { (*(coll as *const Win7DWriteFontCollection1)).raw }
    } else {
        coll
    }
}

unsafe extern "system" fn col1_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_COLLECTION || g == IID_IDWRITE_FONT_COLLECTION1 {
        unsafe { col1_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteFontCollection1)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn col1_add_ref(this: *mut c_void) -> u32 {
    unsafe {
        (*(this as *const Win7DWriteFontCollection1))
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}
unsafe extern "system" fn col1_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteFontCollection1) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteFontCollection1) };
        unsafe {
            com_release(boxed.raw);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn col1_family_count(this: *mut c_void) -> u32 {
    let raw = unsafe { (*(this as *const Win7DWriteFontCollection1)).raw };
    unsafe { ((*com_vtbl::<IDWriteFontCollection_Vtbl>(raw)).GetFontFamilyCount)(raw) }
}
unsafe extern "system" fn col1_get_family(
    this: *mut c_void,
    idx: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontCollection1) };
    let mut raw_fam = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontCollection_Vtbl>(me.raw)).GetFontFamily)(me.raw, idx, &mut raw_fam)
    };
    if hr < 0 || raw_fam.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_font_family1(raw_fam, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn col1_find_family(
    this: *mut c_void,
    name: *const u16,
    idx: *mut u32,
    exists: *mut BOOL,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFontCollection1)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteFontCollection_Vtbl>(raw)).FindFamilyName)(raw, name, idx, exists)
    }
}
unsafe extern "system" fn col1_font_from_face(
    this: *mut c_void,
    face: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontCollection1) };
    let raw_face = unsafe { unwrap_font_face(face) };
    let mut raw_font = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFontCollection_Vtbl>(me.raw)).GetFontFromFontFace)(
            me.raw,
            raw_face,
            &mut raw_font,
        )
    };
    if hr < 0 || raw_font.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_font(raw_font, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn col1_get_font_set(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let me = unsafe { &*(this as *const Win7DWriteFontCollection1) };
    unsafe {
        com_add_ref(me.raw);
        com_add_ref(me.raw_factory);
    }
    let set = Box::into_raw(Box::new(Win7CollectionFontSet {
        vtbl: &COLLECTION_FONT_SET_VTBL,
        refs: AtomicU32::new(1),
        raw_collection: me.raw,
        raw_list: ptr::null_mut(),
        raw_factory: me.raw_factory,
    }));
    unsafe { *out = set.cast() };
    S_OK
}

static COLLECTION1_VTBL: IDWriteFontCollection1_Vtbl = IDWriteFontCollection1_Vtbl {
    base: IDWriteFontCollection_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: col1_qi,
            AddRef: col1_add_ref,
            Release: col1_release,
        },
        GetFontFamilyCount: col1_family_count,
        GetFontFamily: col1_get_family,
        FindFamilyName: col1_find_family,
        GetFontFromFontFace: col1_font_from_face,
    },
    GetFontSet: col1_get_font_set,
    GetFontFamily1: col1_get_family,
};

// -----------------------------------------------------------------------------
// IDWriteRenderingParams1, IDWriteGlyphRunAnalysis, IDWriteFontFallback
// -----------------------------------------------------------------------------

#[repr(C)]
struct Win7DWriteRenderingParams1 {
    vtbl: *const IDWriteRenderingParams1_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
}

#[inline]
fn unwrap_rendering_params(p: *mut c_void) -> *mut c_void {
    if !p.is_null()
        && unsafe { *(p as *const usize) } == ptr::addr_of!(RENDERING_PARAMS1_VTBL) as usize
    {
        unsafe { (*(p as *const Win7DWriteRenderingParams1)).raw }
    } else {
        p
    }
}

unsafe extern "system" fn rp1_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_RENDERING_PARAMS
        || g == IID_IDWRITE_RENDERING_PARAMS1
    {
        unsafe { rp1_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn rp1_add_ref(this: *mut c_void) -> u32 {
    unsafe {
        (*(this as *const Win7DWriteRenderingParams1))
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}
unsafe extern "system" fn rp1_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteRenderingParams1) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteRenderingParams1) };
        unsafe { com_release(boxed.raw) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn rp1_gamma(this: *mut c_void) -> f32 {
    let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
    unsafe { ((*com_vtbl::<IDWriteRenderingParams_Vtbl>(raw)).GetGamma)(raw) }
}
unsafe extern "system" fn rp1_contrast(this: *mut c_void) -> f32 {
    let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
    unsafe { ((*com_vtbl::<IDWriteRenderingParams_Vtbl>(raw)).GetEnhancedContrast)(raw) }
}
unsafe extern "system" fn rp1_cleartype(this: *mut c_void) -> f32 {
    let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
    unsafe { ((*com_vtbl::<IDWriteRenderingParams_Vtbl>(raw)).GetClearTypeLevel)(raw) }
}
unsafe extern "system" fn rp1_geom(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
    unsafe { ((*com_vtbl::<IDWriteRenderingParams_Vtbl>(raw)).GetPixelGeometry)(raw) }
}
unsafe extern "system" fn rp1_mode(this: *mut c_void) -> i32 {
    let raw = unsafe { (*(this as *const Win7DWriteRenderingParams1)).raw };
    unsafe { ((*com_vtbl::<IDWriteRenderingParams_Vtbl>(raw)).GetRenderingMode)(raw) }
}

static RENDERING_PARAMS1_VTBL: IDWriteRenderingParams1_Vtbl = IDWriteRenderingParams1_Vtbl {
    base: IDWriteRenderingParams_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: rp1_qi,
            AddRef: rp1_add_ref,
            Release: rp1_release,
        },
        GetGamma: rp1_gamma,
        GetEnhancedContrast: rp1_contrast,
        GetClearTypeLevel: rp1_cleartype,
        GetPixelGeometry: rp1_geom,
        GetRenderingMode: rp1_mode,
    },
    GetGrayscaleEnhancedContrast: rp1_contrast,
};

#[repr(C)]
struct Win7GlyphRunAnalysis {
    vtbl: *const IDWriteGlyphRunAnalysis_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    grayscale_from_cleartype: bool,
}

unsafe extern "system" fn gra_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_GLYPH_RUN_ANALYSIS {
        unsafe { gra_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7GlyphRunAnalysis)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn gra_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7GlyphRunAnalysis)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn gra_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7GlyphRunAnalysis) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7GlyphRunAnalysis) };
        unsafe { com_release(boxed.raw) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn gra_bounds(
    this: *mut c_void,
    tex_type: i32,
    bounds: *mut RECT,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7GlyphRunAnalysis) };
    let actual_type = if me.grayscale_from_cleartype && tex_type == DWRITE_TEXTURE_ALIASED_1X1 {
        DWRITE_TEXTURE_CLEARTYPE_3X1
    } else {
        tex_type
    };
    unsafe {
        ((*com_vtbl::<IDWriteGlyphRunAnalysis_Vtbl>(me.raw)).GetAlphaTextureBounds)(
            me.raw,
            actual_type,
            bounds,
        )
    }
}
unsafe extern "system" fn gra_alpha(
    this: *mut c_void,
    tex_type: i32,
    bounds: *const RECT,
    alpha: *mut u8,
    buf_size: u32,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7GlyphRunAnalysis) };
    if !me.grayscale_from_cleartype || tex_type != DWRITE_TEXTURE_ALIASED_1X1 {
        return unsafe {
            ((*com_vtbl::<IDWriteGlyphRunAnalysis_Vtbl>(me.raw)).CreateAlphaTexture)(
                me.raw, tex_type, bounds, alpha, buf_size,
            )
        };
    }
    if bounds.is_null() || alpha.is_null() {
        return E_INVALIDARG;
    }
    let r = unsafe { *bounds };
    let w = (r.right - r.left).max(0) as usize;
    let h = (r.bottom - r.top).max(0) as usize;
    let pixels = w.saturating_mul(h);
    if (buf_size as usize) < pixels {
        return E_INVALIDARG;
    }
    if pixels == 0 {
        return S_OK;
    }
    let mut rgb = vec![0u8; pixels * 3];
    let hr = unsafe {
        ((*com_vtbl::<IDWriteGlyphRunAnalysis_Vtbl>(me.raw)).CreateAlphaTexture)(
            me.raw,
            DWRITE_TEXTURE_CLEARTYPE_3X1,
            bounds,
            rgb.as_mut_ptr(),
            rgb.len() as u32,
        )
    };
    if hr < 0 {
        return hr;
    }
    for i in 0..pixels {
        let sum = rgb[i * 3] as u16 + rgb[i * 3 + 1] as u16 + rgb[i * 3 + 2] as u16;
        unsafe { *alpha.add(i) = ((sum + 1) / 3) as u8 };
    }
    S_OK
}
unsafe extern "system" fn gra_blend(
    this: *mut c_void,
    params: *mut c_void,
    gamma: *mut f32,
    contrast: *mut f32,
    ct: *mut f32,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7GlyphRunAnalysis)).raw };
    let raw_params = unwrap_rendering_params(params);
    unsafe {
        ((*com_vtbl::<IDWriteGlyphRunAnalysis_Vtbl>(raw)).GetAlphaBlendParams)(
            raw, raw_params, gamma, contrast, ct,
        )
    }
}

static GLYPH_RUN_ANALYSIS_VTBL: IDWriteGlyphRunAnalysis_Vtbl = IDWriteGlyphRunAnalysis_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: gra_qi,
        AddRef: gra_add_ref,
        Release: gra_release,
    },
    GetAlphaTextureBounds: gra_bounds,
    CreateAlphaTexture: gra_alpha,
    GetAlphaBlendParams: gra_blend,
};

unsafe extern "system" fn fb_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_FALLBACK {
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn fb_map(
    _: *mut c_void,
    _: *mut c_void,
    _: u32,
    len: u32,
    _: *mut c_void,
    _: *const u16,
    _: i32,
    _: i32,
    _: i32,
    _: f32,
    mapped_len: *mut u32,
    mapped_font: *mut *mut c_void,
    scale: *mut f32,
) -> HRESULT {
    if !mapped_len.is_null() {
        unsafe { *mapped_len = len };
    }
    if !mapped_font.is_null() {
        unsafe { *mapped_font = ptr::null_mut() };
    }
    if !scale.is_null() {
        unsafe { *scale = 1.0 };
    }
    S_OK
}

static NOOP_FONT_FALLBACK_VTBL: IDWriteFontFallback_Vtbl = IDWriteFontFallback_Vtbl {
    base: IUnknown_Vtbl {
        QueryInterface: fb_qi,
        AddRef: static_unk_add_ref,
        Release: static_unk_release,
    },
    MapCharacters: fb_map,
};
static NOOP_FONT_FALLBACK: StaticCom<IDWriteFontFallback_Vtbl> = StaticCom {
    vtbl: &NOOP_FONT_FALLBACK_VTBL,
};

unsafe extern "system" fn fbb_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_FONT_FALLBACK_BUILDER {
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn fbb_add_map(
    _: *mut c_void,
    _: *const c_void,
    _: u32,
    _: *const *const u16,
    _: u32,
    _: *mut c_void,
    _: *const u16,
    _: *const u16,
    _: f32,
) -> HRESULT {
    S_OK
}
unsafe extern "system" fn fbb_add_maps(_: *mut c_void, _: *mut c_void) -> HRESULT {
    S_OK
}
unsafe extern "system" fn fbb_create(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = ptr::addr_of!(NOOP_FONT_FALLBACK).cast_mut().cast() };
    S_OK
}

static NOOP_FALLBACK_BUILDER_VTBL: IDWriteFontFallbackBuilder_Vtbl =
    IDWriteFontFallbackBuilder_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: fbb_qi,
            AddRef: static_unk_add_ref,
            Release: static_unk_release,
        },
        AddMapping: fbb_add_map,
        AddMappings: fbb_add_maps,
        CreateFontFallback: fbb_create,
    };
static NOOP_FALLBACK_BUILDER: StaticCom<IDWriteFontFallbackBuilder_Vtbl> = StaticCom {
    vtbl: &NOOP_FALLBACK_BUILDER_VTBL,
};

// -----------------------------------------------------------------------------
// IDWriteTextFormat1, IDWriteTextLayout, and IDWriteTextRenderer proxy
// -----------------------------------------------------------------------------

#[repr(C)]
struct Win7DWriteTextFormat1 {
    vtbl: *const IDWriteTextFormat1_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    raw_factory: *mut c_void,
    fallback: AtomicUsize,
}

#[inline]
fn unwrap_text_format(fmt: *mut c_void) -> *mut c_void {
    if !fmt.is_null()
        && unsafe { *(fmt as *const usize) } == ptr::addr_of!(TEXT_FORMAT1_VTBL) as usize
    {
        unsafe { (*(fmt as *const Win7DWriteTextFormat1)).raw }
    } else if !fmt.is_null()
        && unsafe { *(fmt as *const usize) } == ptr::addr_of!(TEXT_LAYOUT_VTBL) as usize
    {
        unsafe { (*(fmt as *const Win7DWriteTextLayout)).raw }
    } else {
        fmt
    }
}

unsafe extern "system" fn tf1_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_TEXT_FORMAT || g == IID_IDWRITE_TEXT_FORMAT1 {
        unsafe { tf1_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteTextFormat1)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn tf1_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteTextFormat1)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn tf1_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteTextFormat1) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteTextFormat1) };
        let fb = boxed.fallback.load(Ordering::Acquire) as *mut c_void;
        unsafe {
            com_release(fb);
            com_release(boxed.raw);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}

macro_rules! forward_text_format_methods {
    ($struct_ty:ty, $prefix:ident) => {
        mod $prefix {
            use super::*;
            #[inline]
            unsafe fn raw(this: *mut c_void) -> *mut c_void {
                unsafe { (*(this as *const $struct_ty)).raw }
            }
            #[inline]
            unsafe fn raw_fac(this: *mut c_void) -> *mut c_void {
                unsafe { (*(this as *const $struct_ty)).raw_factory }
            }
            pub(super) unsafe extern "system" fn set_ta(this: *mut c_void, v: i32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetTextAlignment)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_pa(this: *mut c_void, v: i32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetParagraphAlignment)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_ww(this: *mut c_void, v: i32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetWordWrapping)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_rd(this: *mut c_void, v: i32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetReadingDirection)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_fd(this: *mut c_void, v: i32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetFlowDirection)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_tab(this: *mut c_void, v: f32) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetIncrementalTabStop)(r, v) }
            }
            pub(super) unsafe extern "system" fn set_trim(
                this: *mut c_void,
                a: *const c_void,
                b: *mut c_void,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetTrimming)(r, a, b) }
            }
            pub(super) unsafe extern "system" fn set_ls(
                this: *mut c_void,
                m: i32,
                s: f32,
                b: f32,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).SetLineSpacing)(r, m, s, b) }
            }
            pub(super) unsafe extern "system" fn get_ta(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetTextAlignment)(r) }
            }
            pub(super) unsafe extern "system" fn get_pa(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetParagraphAlignment)(r) }
            }
            pub(super) unsafe extern "system" fn get_ww(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetWordWrapping)(r) }
            }
            pub(super) unsafe extern "system" fn get_rd(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetReadingDirection)(r) }
            }
            pub(super) unsafe extern "system" fn get_fd(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFlowDirection)(r) }
            }
            pub(super) unsafe extern "system" fn get_tab(this: *mut c_void) -> f32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetIncrementalTabStop)(r) }
            }
            pub(super) unsafe extern "system" fn get_trim(
                this: *mut c_void,
                a: *mut c_void,
                b: *mut *mut c_void,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetTrimming)(r, a, b) }
            }
            pub(super) unsafe extern "system" fn get_ls(
                this: *mut c_void,
                m: *mut i32,
                s: *mut f32,
                b: *mut f32,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetLineSpacing)(r, m, s, b) }
            }
            pub(super) unsafe extern "system" fn get_fc(
                this: *mut c_void,
                out: *mut *mut c_void,
            ) -> HRESULT {
                if out.is_null() {
                    return E_INVALIDARG;
                }
                let r = unsafe { raw(this) };
                let mut raw_c = ptr::null_mut();
                let hr = unsafe {
                    ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontCollection)(r, &mut raw_c)
                };
                if hr < 0 || raw_c.is_null() {
                    unsafe { *out = ptr::null_mut() };
                    return hr;
                }
                unsafe { *out = wrap_collection1(raw_c, raw_fac(this)) };
                S_OK
            }
            pub(super) unsafe extern "system" fn get_ff_len(this: *mut c_void) -> u32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontFamilyNameLength)(r) }
            }
            pub(super) unsafe extern "system" fn get_ff(
                this: *mut c_void,
                buf: *mut u16,
                len: u32,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontFamilyName)(r, buf, len) }
            }
            pub(super) unsafe extern "system" fn get_fw(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontWeight)(r) }
            }
            pub(super) unsafe extern "system" fn get_fs(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontStyle)(r) }
            }
            pub(super) unsafe extern "system" fn get_fst(this: *mut c_void) -> i32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontStretch)(r) }
            }
            pub(super) unsafe extern "system" fn get_fsz(this: *mut c_void) -> f32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetFontSize)(r) }
            }
            pub(super) unsafe extern "system" fn get_loc_len(this: *mut c_void) -> u32 {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetLocaleNameLength)(r) }
            }
            pub(super) unsafe extern "system" fn get_loc(
                this: *mut c_void,
                buf: *mut u16,
                len: u32,
            ) -> HRESULT {
                let r = unsafe { raw(this) };
                unsafe { ((*com_vtbl::<IDWriteTextFormat_Vtbl>(r)).GetLocaleName)(r, buf, len) }
            }
        }
    };
}

forward_text_format_methods!(Win7DWriteTextFormat1, tf_fwd);

unsafe extern "system" fn tf1_set_i32_ok(_: *mut c_void, _: i32) -> HRESULT {
    S_OK
}
unsafe extern "system" fn tf1_get_i32_zero(_: *mut c_void) -> i32 {
    0
}
unsafe extern "system" fn tf1_set_fb(this: *mut c_void, fb: *mut c_void) -> HRESULT {
    let me = unsafe { &*(this as *const Win7DWriteTextFormat1) };
    unsafe { com_add_ref(fb) };
    let old = me.fallback.swap(fb as usize, Ordering::AcqRel) as *mut c_void;
    unsafe { com_release(old) };
    S_OK
}
unsafe extern "system" fn tf1_get_fb(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let fb = unsafe {
        (*(this as *const Win7DWriteTextFormat1))
            .fallback
            .load(Ordering::Acquire) as *mut c_void
    };
    unsafe {
        com_add_ref(fb);
        *out = fb;
    }
    S_OK
}

static TEXT_FORMAT1_VTBL: IDWriteTextFormat1_Vtbl = IDWriteTextFormat1_Vtbl {
    base: IDWriteTextFormat_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: tf1_qi,
            AddRef: tf1_add_ref,
            Release: tf1_release,
        },
        SetTextAlignment: tf_fwd::set_ta,
        SetParagraphAlignment: tf_fwd::set_pa,
        SetWordWrapping: tf_fwd::set_ww,
        SetReadingDirection: tf_fwd::set_rd,
        SetFlowDirection: tf_fwd::set_fd,
        SetIncrementalTabStop: tf_fwd::set_tab,
        SetTrimming: tf_fwd::set_trim,
        SetLineSpacing: tf_fwd::set_ls,
        GetTextAlignment: tf_fwd::get_ta,
        GetParagraphAlignment: tf_fwd::get_pa,
        GetWordWrapping: tf_fwd::get_ww,
        GetReadingDirection: tf_fwd::get_rd,
        GetFlowDirection: tf_fwd::get_fd,
        GetIncrementalTabStop: tf_fwd::get_tab,
        GetTrimming: tf_fwd::get_trim,
        GetLineSpacing: tf_fwd::get_ls,
        GetFontCollection: tf_fwd::get_fc,
        GetFontFamilyNameLength: tf_fwd::get_ff_len,
        GetFontFamilyName: tf_fwd::get_ff,
        GetFontWeight: tf_fwd::get_fw,
        GetFontStyle: tf_fwd::get_fs,
        GetFontStretch: tf_fwd::get_fst,
        GetFontSize: tf_fwd::get_fsz,
        GetLocaleNameLength: tf_fwd::get_loc_len,
        GetLocaleName: tf_fwd::get_loc,
    },
    SetVerticalGlyphOrientation: tf1_set_i32_ok,
    GetVerticalGlyphOrientation: tf1_get_i32_zero,
    SetLastLineWrapping: tf1_set_i32_ok,
    GetLastLineWrapping: tf1_get_i32_zero,
    SetOpticalAlignment: tf1_set_i32_ok,
    GetOpticalAlignment: tf1_get_i32_zero,
    SetFontFallback: tf1_set_fb,
    GetFontFallback: tf1_get_fb,
};

#[repr(C)]
struct Win7TextRendererProxy {
    vtbl: *const IDWriteTextRenderer_Vtbl,
    target: *mut c_void,
    raw_factory: *mut c_void,
}

unsafe extern "system" fn trp_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_PIXEL_SNAPPING || g == IID_IDWRITE_TEXT_RENDERER {
        unsafe { *out = this };
        S_OK
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}
unsafe extern "system" fn trp_snap_dis(
    this: *mut c_void,
    ctx: *const c_void,
    dis: *mut BOOL,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe { ((*com_vtbl::<IDWritePixelSnapping_Vtbl>(t)).IsPixelSnappingDisabled)(t, ctx, dis) }
}
unsafe extern "system" fn trp_xform(
    this: *mut c_void,
    ctx: *const c_void,
    xform: *mut DWRITE_MATRIX,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe { ((*com_vtbl::<IDWritePixelSnapping_Vtbl>(t)).GetCurrentTransform)(t, ctx, xform) }
}
unsafe extern "system" fn trp_ppd(
    this: *mut c_void,
    ctx: *const c_void,
    ppd: *mut f32,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe { ((*com_vtbl::<IDWritePixelSnapping_Vtbl>(t)).GetPixelsPerDip)(t, ctx, ppd) }
}
unsafe extern "system" fn trp_draw_run(
    this: *mut c_void,
    ctx: *const c_void,
    ox: f32,
    oy: f32,
    mode: i32,
    run: *const DWRITE_GLYPH_RUN,
    desc: *const c_void,
    effect: *mut c_void,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7TextRendererProxy) };
    if run.is_null() {
        return unsafe {
            ((*com_vtbl::<IDWriteTextRenderer_Vtbl>(me.target)).DrawGlyphRun)(
                me.target, ctx, ox, oy, mode, run, desc, effect,
            )
        };
    }
    let mut wrapped_run = unsafe { *run };
    let mut to_release = ptr::null_mut();
    if !wrapped_run.fontFace.is_null() {
        let face3 = unsafe {
            intern_font_face3(wrapped_run.fontFace, ptr::null_mut(), me.raw_factory)
        };
        wrapped_run.fontFace = face3;
        to_release = face3;
    }
    let hr = unsafe {
        ((*com_vtbl::<IDWriteTextRenderer_Vtbl>(me.target)).DrawGlyphRun)(
            me.target,
            ctx,
            ox,
            oy,
            mode,
            &wrapped_run,
            desc,
            effect,
        )
    };
    unsafe { com_release(to_release) };
    hr
}
unsafe extern "system" fn trp_underline(
    this: *mut c_void,
    ctx: *const c_void,
    ox: f32,
    oy: f32,
    ul: *const c_void,
    eff: *mut c_void,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe { ((*com_vtbl::<IDWriteTextRenderer_Vtbl>(t)).DrawUnderline)(t, ctx, ox, oy, ul, eff) }
}
unsafe extern "system" fn trp_strike(
    this: *mut c_void,
    ctx: *const c_void,
    ox: f32,
    oy: f32,
    st: *const c_void,
    eff: *mut c_void,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe {
        ((*com_vtbl::<IDWriteTextRenderer_Vtbl>(t)).DrawStrikethrough)(t, ctx, ox, oy, st, eff)
    }
}
unsafe extern "system" fn trp_inline(
    this: *mut c_void,
    ctx: *const c_void,
    ox: f32,
    oy: f32,
    obj: *mut c_void,
    sideways: BOOL,
    rtl: BOOL,
    eff: *mut c_void,
) -> HRESULT {
    let t = unsafe { (*(this as *const Win7TextRendererProxy)).target };
    unsafe {
        ((*com_vtbl::<IDWriteTextRenderer_Vtbl>(t)).DrawInlineObject)(
            t, ctx, ox, oy, obj, sideways, rtl, eff,
        )
    }
}

static TEXT_RENDERER_PROXY_VTBL: IDWriteTextRenderer_Vtbl = IDWriteTextRenderer_Vtbl {
    base: IDWritePixelSnapping_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: trp_qi,
            AddRef: static_unk_add_ref,
            Release: static_unk_release,
        },
        IsPixelSnappingDisabled: trp_snap_dis,
        GetCurrentTransform: trp_xform,
        GetPixelsPerDip: trp_ppd,
    },
    DrawGlyphRun: trp_draw_run,
    DrawUnderline: trp_underline,
    DrawStrikethrough: trp_strike,
    DrawInlineObject: trp_inline,
};

#[repr(C)]
struct Win7DWriteTextLayout {
    vtbl: *const IDWriteTextLayout_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
    raw_factory: *mut c_void,
}

forward_text_format_methods!(Win7DWriteTextLayout, tl_fwd);

unsafe extern "system" fn tl_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN || g == IID_IDWRITE_TEXT_FORMAT || g == IID_IDWRITE_TEXT_LAYOUT {
        unsafe { tl_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn tl_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteTextLayout)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn tl_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteTextLayout) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteTextLayout) };
        unsafe {
            com_release(boxed.raw);
            com_release(boxed.raw_factory);
        }
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn tl_set_mw(this: *mut c_void, v: f32) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetMaxWidth)(r, v) }
}
unsafe extern "system" fn tl_set_mh(this: *mut c_void, v: f32) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetMaxHeight)(r, v) }
}
unsafe extern "system" fn tl_set_fc(
    this: *mut c_void,
    coll: *mut c_void,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    let raw_c = unsafe { unwrap_collection(coll) };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontCollection)(r, raw_c, range) }
}
unsafe extern "system" fn tl_set_ff(
    this: *mut c_void,
    name: *const u16,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontFamilyName)(r, name, range) }
}
unsafe extern "system" fn tl_set_fw(
    this: *mut c_void,
    w: i32,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontWeight)(r, w, range) }
}
unsafe extern "system" fn tl_set_fs(
    this: *mut c_void,
    s: i32,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontStyle)(r, s, range) }
}
unsafe extern "system" fn tl_set_fst(
    this: *mut c_void,
    st: i32,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontStretch)(r, st, range) }
}
unsafe extern "system" fn tl_set_fsz(
    this: *mut c_void,
    sz: f32,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetFontSize)(r, sz, range) }
}
unsafe extern "system" fn tl_set_ul(
    this: *mut c_void,
    u: BOOL,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetUnderline)(r, u, range) }
}
unsafe extern "system" fn tl_set_stk(
    this: *mut c_void,
    s: BOOL,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetStrikethrough)(r, s, range) }
}
unsafe extern "system" fn tl_set_de(
    this: *mut c_void,
    e: *mut c_void,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetDrawingEffect)(r, e, range) }
}
unsafe extern "system" fn tl_set_io(
    this: *mut c_void,
    o: *mut c_void,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetInlineObject)(r, o, range) }
}
unsafe extern "system" fn tl_set_typo(
    this: *mut c_void,
    t: *mut c_void,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetTypography)(r, t, range) }
}
unsafe extern "system" fn tl_set_loc(
    this: *mut c_void,
    l: *const u16,
    range: DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).SetLocaleName)(r, l, range) }
}
unsafe extern "system" fn tl_get_mw(this: *mut c_void) -> f32 {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetMaxWidth)(r) }
}
unsafe extern "system" fn tl_get_mh(this: *mut c_void) -> f32 {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetMaxHeight)(r) }
}
unsafe extern "system" fn tl_get_fc(
    this: *mut c_void,
    pos: u32,
    out: *mut *mut c_void,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7DWriteTextLayout) };
    let mut raw_c = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(me.raw)).GetFontCollectionLayout)(
            me.raw,
            pos,
            &mut raw_c,
            range,
        )
    };
    if hr < 0 || raw_c.is_null() {
        if !out.is_null() {
            unsafe { *out = ptr::null_mut() };
        }
        return hr;
    }
    unsafe { *out = wrap_collection1(raw_c, me.raw_factory) };
    S_OK
}
unsafe extern "system" fn tl_get_u32_r(
    this: *mut c_void,
    pos: u32,
    out: *mut u32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontFamilyNameLengthLayout)(r, pos, out, range)
    }
}
unsafe extern "system" fn tl_get_ff(
    this: *mut c_void,
    pos: u32,
    buf: *mut u16,
    len: u32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontFamilyNameLayout)(r, pos, buf, len, range)
    }
}
unsafe extern "system" fn tl_get_fw(
    this: *mut c_void,
    pos: u32,
    out: *mut i32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontWeightLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_fs(
    this: *mut c_void,
    pos: u32,
    out: *mut i32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontStyleLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_fst(
    this: *mut c_void,
    pos: u32,
    out: *mut i32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontStretchLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_fsz(
    this: *mut c_void,
    pos: u32,
    out: *mut f32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetFontSizeLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_ul(
    this: *mut c_void,
    pos: u32,
    out: *mut BOOL,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetUnderlineLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_stk(
    this: *mut c_void,
    pos: u32,
    out: *mut BOOL,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetStrikethroughLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_de(
    this: *mut c_void,
    pos: u32,
    out: *mut *mut c_void,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetDrawingEffectLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_io(
    this: *mut c_void,
    pos: u32,
    out: *mut *mut c_void,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetInlineObjectLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_typo(
    this: *mut c_void,
    pos: u32,
    out: *mut *mut c_void,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetTypographyLayout)(r, pos, out, range) }
}
unsafe extern "system" fn tl_get_loc_len(
    this: *mut c_void,
    pos: u32,
    out: *mut u32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetLocaleNameLengthLayout)(r, pos, out, range)
    }
}
unsafe extern "system" fn tl_get_loc(
    this: *mut c_void,
    pos: u32,
    buf: *mut u16,
    len: u32,
    range: *mut DWRITE_TEXT_RANGE,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetLocaleNameLayout)(r, pos, buf, len, range)
    }
}
unsafe extern "system" fn tl_draw(
    this: *mut c_void,
    ctx: *const c_void,
    renderer: *mut c_void,
    ox: f32,
    oy: f32,
) -> HRESULT {
    let me = unsafe { &*(this as *const Win7DWriteTextLayout) };
    if renderer.is_null() {
        return unsafe {
            ((*com_vtbl::<IDWriteTextLayout_Vtbl>(me.raw)).Draw)(me.raw, ctx, renderer, ox, oy)
        };
    }
    let mut proxy = Win7TextRendererProxy {
        vtbl: &TEXT_RENDERER_PROXY_VTBL,
        target: renderer,
        raw_factory: me.raw_factory,
    };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(me.raw)).Draw)(
            me.raw,
            ctx,
            ptr::addr_of_mut!(proxy).cast(),
            ox,
            oy,
        )
    }
}
unsafe extern "system" fn tl_line_metrics(
    this: *mut c_void,
    buf: *mut c_void,
    max: u32,
    actual: *mut u32,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetLineMetrics)(r, buf, max, actual) }
}
unsafe extern "system" fn tl_metrics(this: *mut c_void, out: *mut c_void) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetMetrics)(r, out) }
}
unsafe extern "system" fn tl_overhang(this: *mut c_void, out: *mut c_void) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetOverhangMetrics)(r, out) }
}
unsafe extern "system" fn tl_clusters(
    this: *mut c_void,
    buf: *mut c_void,
    max: u32,
    actual: *mut u32,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).GetClusterMetrics)(r, buf, max, actual) }
}
unsafe extern "system" fn tl_min_w(this: *mut c_void, out: *mut f32) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe { ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).DetermineMinWidth)(r, out) }
}
unsafe extern "system" fn tl_ht_pt(
    this: *mut c_void,
    x: f32,
    y: f32,
    trail: *mut BOOL,
    inside: *mut BOOL,
    metrics: *mut c_void,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).HitTestPoint)(r, x, y, trail, inside, metrics)
    }
}
unsafe extern "system" fn tl_ht_pos(
    this: *mut c_void,
    pos: u32,
    trail: BOOL,
    x: *mut f32,
    y: *mut f32,
    metrics: *mut c_void,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).HitTestTextPosition)(r, pos, trail, x, y, metrics)
    }
}
unsafe extern "system" fn tl_ht_range(
    this: *mut c_void,
    pos: u32,
    len: u32,
    ox: f32,
    oy: f32,
    metrics: *mut c_void,
    max: u32,
    actual: *mut u32,
) -> HRESULT {
    let r = unsafe { (*(this as *const Win7DWriteTextLayout)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteTextLayout_Vtbl>(r)).HitTestTextRange)(
            r, pos, len, ox, oy, metrics, max, actual,
        )
    }
}

static TEXT_LAYOUT_VTBL: IDWriteTextLayout_Vtbl = IDWriteTextLayout_Vtbl {
    base: IDWriteTextFormat_Vtbl {
        base: IUnknown_Vtbl {
            QueryInterface: tl_qi,
            AddRef: tl_add_ref,
            Release: tl_release,
        },
        SetTextAlignment: tl_fwd::set_ta,
        SetParagraphAlignment: tl_fwd::set_pa,
        SetWordWrapping: tl_fwd::set_ww,
        SetReadingDirection: tl_fwd::set_rd,
        SetFlowDirection: tl_fwd::set_fd,
        SetIncrementalTabStop: tl_fwd::set_tab,
        SetTrimming: tl_fwd::set_trim,
        SetLineSpacing: tl_fwd::set_ls,
        GetTextAlignment: tl_fwd::get_ta,
        GetParagraphAlignment: tl_fwd::get_pa,
        GetWordWrapping: tl_fwd::get_ww,
        GetReadingDirection: tl_fwd::get_rd,
        GetFlowDirection: tl_fwd::get_fd,
        GetIncrementalTabStop: tl_fwd::get_tab,
        GetTrimming: tl_fwd::get_trim,
        GetLineSpacing: tl_fwd::get_ls,
        GetFontCollection: tl_fwd::get_fc,
        GetFontFamilyNameLength: tl_fwd::get_ff_len,
        GetFontFamilyName: tl_fwd::get_ff,
        GetFontWeight: tl_fwd::get_fw,
        GetFontStyle: tl_fwd::get_fs,
        GetFontStretch: tl_fwd::get_fst,
        GetFontSize: tl_fwd::get_fsz,
        GetLocaleNameLength: tl_fwd::get_loc_len,
        GetLocaleName: tl_fwd::get_loc,
    },
    SetMaxWidth: tl_set_mw,
    SetMaxHeight: tl_set_mh,
    SetFontCollection: tl_set_fc,
    SetFontFamilyName: tl_set_ff,
    SetFontWeight: tl_set_fw,
    SetFontStyle: tl_set_fs,
    SetFontStretch: tl_set_fst,
    SetFontSize: tl_set_fsz,
    SetUnderline: tl_set_ul,
    SetStrikethrough: tl_set_stk,
    SetDrawingEffect: tl_set_de,
    SetInlineObject: tl_set_io,
    SetTypography: tl_set_typo,
    SetLocaleName: tl_set_loc,
    GetMaxWidth: tl_get_mw,
    GetMaxHeight: tl_get_mh,
    GetFontCollectionLayout: tl_get_fc,
    GetFontFamilyNameLengthLayout: tl_get_u32_r,
    GetFontFamilyNameLayout: tl_get_ff,
    GetFontWeightLayout: tl_get_fw,
    GetFontStyleLayout: tl_get_fs,
    GetFontStretchLayout: tl_get_fst,
    GetFontSizeLayout: tl_get_fsz,
    GetUnderlineLayout: tl_get_ul,
    GetStrikethroughLayout: tl_get_stk,
    GetDrawingEffectLayout: tl_get_de,
    GetInlineObjectLayout: tl_get_io,
    GetTypographyLayout: tl_get_typo,
    GetLocaleNameLengthLayout: tl_get_loc_len,
    GetLocaleNameLayout: tl_get_loc,
    Draw: tl_draw,
    GetLineMetrics: tl_line_metrics,
    GetMetrics: tl_metrics,
    GetOverhangMetrics: tl_overhang,
    GetClusterMetrics: tl_clusters,
    DetermineMinWidth: tl_min_w,
    HitTestPoint: tl_ht_pt,
    HitTestTextPosition: tl_ht_pos,
    HitTestTextRange: tl_ht_range,
};

#[repr(C)]
struct Win7DWriteFactory5 {
    vtbl: *const IDWriteFactory5_Vtbl,
    refs: AtomicU32,
    raw: *mut c_void,
}

#[inline]
fn unwrap_factory(f: *mut c_void) -> *mut c_void {
    if !f.is_null()
        && unsafe { *(f as *const usize) } == ptr::addr_of!(DWRITE_FACTORY5_VTBL) as usize
    {
        unsafe { (*(f as *const Win7DWriteFactory5)).raw }
    } else {
        f
    }
}

unsafe extern "system" fn dw5_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let g = unsafe { *iid };
    if g == IID_IUNKNOWN
        || g == IID_IDWRITE_FACTORY
        || g == IID_IDWRITE_FACTORY1
        || g == IID_IDWRITE_FACTORY2
        || g == IID_IDWRITE_FACTORY3
        || g == IID_IDWRITE_FACTORY4
        || g == IID_IDWRITE_FACTORY5
    {
        unsafe { dw5_add_ref(this) };
        unsafe { *out = this };
        S_OK
    } else {
        let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
        unsafe { ((*com_vtbl::<IUnknown_Vtbl>(raw)).QueryInterface)(raw, iid, out) }
    }
}
unsafe extern "system" fn dw5_add_ref(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const Win7DWriteFactory5)).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn dw5_release(this: *mut c_void) -> u32 {
    let me = unsafe { &*(this as *const Win7DWriteFactory5) };
    let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        let boxed = unsafe { Box::from_raw(this as *mut Win7DWriteFactory5) };
        unsafe { com_release(boxed.raw) };
        0
    } else {
        prev - 1
    }
}
unsafe extern "system" fn dw5_sys_coll(
    this: *mut c_void,
    out: *mut *mut c_void,
    check_updates: BOOL,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut raw_coll = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).GetSystemFontCollection)(
            raw,
            &mut raw_coll,
            check_updates,
        )
    };
    if hr < 0 || raw_coll.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_collection1(raw_coll, raw) };
    S_OK
}
unsafe extern "system" fn dw5_custom_coll(
    this: *mut c_void,
    loader: *mut c_void,
    key: *const c_void,
    key_size: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateCustomFontCollection)(
            raw, loader, key, key_size, out,
        )
    }
}
unsafe extern "system" fn dw5_reg_cl(this: *mut c_void, loader: *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).RegisterFontCollectionLoader)(raw, loader) }
}
unsafe extern "system" fn dw5_unreg_cl(this: *mut c_void, loader: *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).UnregisterFontCollectionLoader)(raw, loader) }
}
unsafe extern "system" fn dw5_file_ref(
    this: *mut c_void,
    path: *const u16,
    ftime: *const FILETIME,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateFontFileReference)(raw, path, ftime, out)
    }
}
unsafe extern "system" fn dw5_custom_file_ref(
    this: *mut c_void,
    key: *const c_void,
    key_size: u32,
    loader: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateCustomFontFileReference)(
            raw, key, key_size, loader, out,
        )
    }
}
unsafe extern "system" fn dw5_create_face(
    this: *mut c_void,
    face_type: i32,
    num_files: u32,
    files: *const *mut c_void,
    index: u32,
    sims: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut raw_face = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateFontFace)(
            raw,
            face_type,
            num_files,
            files,
            index,
            sims,
            &mut raw_face,
        )
    };
    if hr < 0 || raw_face.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let wrapped = unsafe { intern_font_face3(raw_face, ptr::null_mut(), raw) };
    unsafe {
        com_release(raw_face);
        *out = wrapped;
    }
    S_OK
}
unsafe extern "system" fn dw5_render_params(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut raw_rp = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateRenderingParams)(raw, &mut raw_rp)
    };
    if hr < 0 || raw_rp.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    if let Some(rp1) = unsafe { com_qi(raw_rp, &IID_IDWRITE_RENDERING_PARAMS1) } {
        unsafe {
            com_release(raw_rp);
            *out = rp1;
        }
        return S_OK;
    }
    let wrapped = Box::into_raw(Box::new(Win7DWriteRenderingParams1 {
        vtbl: &RENDERING_PARAMS1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_rp,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_mon_rp(
    this: *mut c_void,
    mon: HMONITOR,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut raw_rp = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateMonitorRenderingParams)(raw, mon, &mut raw_rp)
    };
    if hr < 0 || raw_rp.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let wrapped = Box::into_raw(Box::new(Win7DWriteRenderingParams1 {
        vtbl: &RENDERING_PARAMS1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_rp,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_custom_rp(
    this: *mut c_void,
    gamma: f32,
    contrast: f32,
    ct: f32,
    geom: i32,
    mode: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut raw_rp = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateCustomRenderingParams)(
            raw,
            gamma,
            contrast,
            ct,
            geom,
            mode.clamp(0, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC),
            &mut raw_rp,
        )
    };
    if hr < 0 || raw_rp.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let wrapped = Box::into_raw(Box::new(Win7DWriteRenderingParams1 {
        vtbl: &RENDERING_PARAMS1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_rp,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_reg_fl(this: *mut c_void, loader: *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).RegisterFontFileLoader)(raw, loader) }
}
unsafe extern "system" fn dw5_unreg_fl(this: *mut c_void, loader: *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).UnregisterFontFileLoader)(raw, loader) }
}
unsafe extern "system" fn dw5_text_format(
    this: *mut c_void,
    family: *const u16,
    coll: *mut c_void,
    weight: i32,
    style: i32,
    stretch: i32,
    size: f32,
    locale: *const u16,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let raw_coll = unsafe { unwrap_collection(coll) };
    let mut raw_tf = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateTextFormat)(
            raw,
            family,
            raw_coll,
            weight,
            style,
            stretch,
            size,
            locale,
            &mut raw_tf,
        )
    };
    if hr < 0 || raw_tf.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(raw) };
    let wrapped = Box::into_raw(Box::new(Win7DWriteTextFormat1 {
        vtbl: &TEXT_FORMAT1_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_tf,
        raw_factory: raw,
        fallback: AtomicUsize::new(0),
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_typography(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateTypography)(raw, out) }
}
unsafe extern "system" fn dw5_gdi_interop(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).GetGdiInterop)(raw, out) }
}
unsafe extern "system" fn dw5_text_layout(
    this: *mut c_void,
    text: *const u16,
    len: u32,
    fmt: *mut c_void,
    max_w: f32,
    max_h: f32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let raw_fmt = unwrap_text_format(fmt);
    let mut raw_tl = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateTextLayout)(
            raw,
            text,
            len,
            raw_fmt,
            max_w,
            max_h,
            &mut raw_tl,
        )
    };
    if hr < 0 || raw_tl.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(raw) };
    let wrapped = Box::into_raw(Box::new(Win7DWriteTextLayout {
        vtbl: &TEXT_LAYOUT_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_tl,
        raw_factory: raw,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_gdi_layout(
    this: *mut c_void,
    text: *const u16,
    len: u32,
    fmt: *mut c_void,
    w: f32,
    h: f32,
    ppd: f32,
    xform: *const DWRITE_MATRIX,
    gdi_nat: BOOL,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let raw_fmt = unwrap_text_format(fmt);
    let mut raw_tl = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateGdiCompatibleTextLayout)(
            raw,
            text,
            len,
            raw_fmt,
            w,
            h,
            ppd,
            xform,
            gdi_nat,
            &mut raw_tl,
        )
    };
    if hr < 0 || raw_tl.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { com_add_ref(raw) };
    let wrapped = Box::into_raw(Box::new(Win7DWriteTextLayout {
        vtbl: &TEXT_LAYOUT_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_tl,
        raw_factory: raw,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_ellipsis(
    this: *mut c_void,
    fmt: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let raw_fmt = unwrap_text_format(fmt);
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateEllipsisTrimmingSign)(raw, raw_fmt, out) }
}
unsafe extern "system" fn dw5_analyzer(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe { ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateTextAnalyzer)(raw, out) }
}
unsafe extern "system" fn dw5_num_sub(
    this: *mut c_void,
    method: i32,
    locale: *const u16,
    ignore: BOOL,
    out: *mut *mut c_void,
) -> HRESULT {
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateNumberSubstitution)(
            raw, method, locale, ignore, out,
        )
    }
}
unsafe extern "system" fn dw5_gra0(
    this: *mut c_void,
    run: *const DWRITE_GLYPH_RUN,
    ppd: f32,
    xform: *const DWRITE_MATRIX,
    mode: i32,
    measure: i32,
    bx: f32,
    by: f32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || run.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let mut native_run = unsafe { *run };
    native_run.fontFace = unsafe { unwrap_font_face(native_run.fontFace) };
    let clamped_mode = mode.clamp(0, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC);
    unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateGlyphRunAnalysis)(
            raw,
            &native_run,
            ppd,
            xform,
            clamped_mode,
            measure,
            bx,
            by,
            out,
        )
    }
}
unsafe extern "system" fn dw5_eudc(
    _: *mut c_void,
    out: *mut *mut c_void,
    _: BOOL,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn dw5_custom_rp1(
    this: *mut c_void,
    gamma: f32,
    contrast: f32,
    _gray_contrast: f32,
    ct: f32,
    geom: i32,
    mode: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe { dw5_custom_rp(this, gamma, contrast, ct, geom, mode, out) }
}
unsafe extern "system" fn dw5_sys_fb(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = ptr::addr_of!(NOOP_FONT_FALLBACK).cast_mut().cast() };
    S_OK
}
unsafe extern "system" fn dw5_fb_builder(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = ptr::addr_of!(NOOP_FALLBACK_BUILDER).cast_mut().cast() };
    S_OK
}
unsafe extern "system" fn dw5_color2(
    _: *mut c_void,
    _: f32,
    _: f32,
    _: *const DWRITE_GLYPH_RUN,
    _: *const c_void,
    _: i32,
    _: *const DWRITE_MATRIX,
    _: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    DWRITE_E_NOCOLOR
}
unsafe extern "system" fn dw5_custom_rp2(
    this: *mut c_void,
    gamma: f32,
    contrast: f32,
    _gray_contrast: f32,
    ct: f32,
    geom: i32,
    mode: i32,
    _grid: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe { dw5_custom_rp(this, gamma, contrast, ct, geom, mode, out) }
}
unsafe extern "system" fn dw5_gra3(
    this: *mut c_void,
    run: *const DWRITE_GLYPH_RUN,
    xform: *const DWRITE_MATRIX,
    mode: i32,
    measure: i32,
    _grid: i32,
    antialias: i32,
    bx: f32,
    by: f32,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || run.is_null() {
        return E_INVALIDARG;
    }
    let clamped_mode = mode.clamp(0, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC);
    let mut raw_gra = ptr::null_mut();
    let hr = unsafe { dw5_gra0(this, run, 1.0, xform, clamped_mode, measure, bx, by, &mut raw_gra) };
    if hr < 0 || raw_gra.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let convert_gray = antialias == DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE
        && clamped_mode != DWRITE_RENDERING_MODE_ALIASED;
    let wrapped = Box::into_raw(Box::new(Win7GlyphRunAnalysis {
        vtbl: &GLYPH_RUN_ANALYSIS_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_gra,
        grayscale_from_cleartype: convert_gray,
    }));
    unsafe { *out = wrapped.cast() };
    S_OK
}
unsafe extern "system" fn dw5_ffr1(
    _: *mut c_void,
    _: *mut c_void,
    _: u32,
    _: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn dw5_ffr2(
    _: *mut c_void,
    _: *const u16,
    _: *const FILETIME,
    _: u32,
    _: i32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn dw5_sys_font_set(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let mut coll1 = ptr::null_mut();
    let hr = unsafe { dw5_sys_coll(this, &mut coll1, 0) };
    if hr < 0 || coll1.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    let res = unsafe { col1_get_font_set(coll1, out) };
    unsafe { col1_release(coll1) };
    res
}
unsafe extern "system" fn dw5_create_fsb(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let builder = Box::into_raw(Box::new(Win7FontSetBuilder1 {
        vtbl: &FONT_SET_BUILDER1_VTBL,
        refs: AtomicU32::new(1),
        files: Mutex::new(Vec::new()),
    }));
    unsafe { *out = builder.cast() };
    S_OK
}
unsafe extern "system" fn dw5_coll_from_set(
    this: *mut c_void,
    font_set: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || font_set.is_null() {
        return E_INVALIDARG;
    }
    let raw = unsafe { (*(this as *const Win7DWriteFactory5)).raw };
    let files: Vec<SendPtr> =
        if unsafe { *(font_set as *const usize) } == ptr::addr_of!(CUSTOM_FONT_SET_VTBL) as usize {
            let cfs = unsafe { &*(font_set as *const Win7CustomFontSet) };
            cfs.files
                .iter()
                .map(|f| {
                    unsafe { com_add_ref(f.0) };
                    *f
                })
                .collect()
        } else {
            Vec::new()
        };
    let mut guard = collection_store().lock().unwrap_or_else(|e| e.into_inner());
    let key = guard.len() as u32;
    guard.push(files);
    drop(guard);

    let mut raw_coll = ptr::null_mut();
    let hr = unsafe {
        ((*com_vtbl::<IDWriteFactory_Vtbl>(raw)).CreateCustomFontCollection)(
            raw,
            ptr::addr_of!(GLOBAL_COLLECTION_LOADER).cast_mut().cast(),
            ptr::from_ref(&key).cast(),
            4,
            &mut raw_coll,
        )
    };
    if hr < 0 || raw_coll.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr;
    }
    unsafe { *out = wrap_collection1(raw_coll, raw) };
    S_OK
}
unsafe extern "system" fn dw5_sys_coll3(
    this: *mut c_void,
    _include_downloadable: BOOL,
    out: *mut *mut c_void,
    check_updates: BOOL,
) -> HRESULT {
    unsafe { dw5_sys_coll(this, out, check_updates) }
}
unsafe extern "system" fn dw5_dl_queue(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn dw5_color4(
    _: *mut c_void,
    _: Vector2,
    _: *const DWRITE_GLYPH_RUN,
    _: *const c_void,
    _: u32,
    _: i32,
    _: *const DWRITE_MATRIX,
    _: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    DWRITE_E_NOCOLOR
}
unsafe extern "system" fn dw5_origins1(
    _: *mut c_void,
    _: *const DWRITE_GLYPH_RUN,
    _: Vector2,
    _: *mut Vector2,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn dw5_origins2(
    _: *mut c_void,
    _: *const DWRITE_GLYPH_RUN,
    _: i32,
    _: Vector2,
    _: *const DWRITE_MATRIX,
    _: *mut Vector2,
) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn dw5_create_iml(_: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let loader = Box::into_raw(Box::new(Win7InMemoryFontFileLoader {
        vtbl: &IN_MEMORY_LOADER_VTBL,
        refs: AtomicU32::new(1),
        blobs: Mutex::new(Vec::new()),
    }));
    unsafe { *out = loader.cast() };
    S_OK
}
unsafe extern "system" fn dw5_http_loader(
    _: *mut c_void,
    _: *const u16,
    _: *const u16,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}
unsafe extern "system" fn dw5_container(_: *mut c_void, _: *const c_void, _: u32) -> i32 {
    0
}
unsafe extern "system" fn dw5_unpack(
    _: *mut c_void,
    _: i32,
    _: *const c_void,
    _: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
    E_NOTIMPL
}

static DWRITE_FACTORY5_VTBL: IDWriteFactory5_Vtbl = IDWriteFactory5_Vtbl {
    base: IDWriteFactory4_Vtbl {
        base: IDWriteFactory3_Vtbl {
            base: IDWriteFactory2_Vtbl {
                base: IDWriteFactory1_Vtbl {
                    base: IDWriteFactory_Vtbl {
                        base: IUnknown_Vtbl {
                            QueryInterface: dw5_qi,
                            AddRef: dw5_add_ref,
                            Release: dw5_release,
                        },
                        GetSystemFontCollection: dw5_sys_coll,
                        CreateCustomFontCollection: dw5_custom_coll,
                        RegisterFontCollectionLoader: dw5_reg_cl,
                        UnregisterFontCollectionLoader: dw5_unreg_cl,
                        CreateFontFileReference: dw5_file_ref,
                        CreateCustomFontFileReference: dw5_custom_file_ref,
                        CreateFontFace: dw5_create_face,
                        CreateRenderingParams: dw5_render_params,
                        CreateMonitorRenderingParams: dw5_mon_rp,
                        CreateCustomRenderingParams: dw5_custom_rp,
                        RegisterFontFileLoader: dw5_reg_fl,
                        UnregisterFontFileLoader: dw5_unreg_fl,
                        CreateTextFormat: dw5_text_format,
                        CreateTypography: dw5_typography,
                        GetGdiInterop: dw5_gdi_interop,
                        CreateTextLayout: dw5_text_layout,
                        CreateGdiCompatibleTextLayout: dw5_gdi_layout,
                        CreateEllipsisTrimmingSign: dw5_ellipsis,
                        CreateTextAnalyzer: dw5_analyzer,
                        CreateNumberSubstitution: dw5_num_sub,
                        CreateGlyphRunAnalysis: dw5_gra0,
                    },
                    GetEudcFontCollection: dw5_eudc,
                    CreateCustomRenderingParams1: dw5_custom_rp1,
                },
                GetSystemFontFallback: dw5_sys_fb,
                CreateFontFallbackBuilder: dw5_fb_builder,
                TranslateColorGlyphRun2: dw5_color2,
                CreateCustomRenderingParams2: dw5_custom_rp2,
                CreateGlyphRunAnalysis2: dw5_gra3,
            },
            CreateGlyphRunAnalysis3: dw5_gra3,
            CreateCustomRenderingParams3: dw5_custom_rp2,
            CreateFontFaceReference: dw5_ffr1,
            CreateFontFaceReference2: dw5_ffr2,
            GetSystemFontSet: dw5_sys_font_set,
            CreateFontSetBuilder3: dw5_create_fsb,
            CreateFontCollectionFromFontSet: dw5_coll_from_set,
            GetSystemFontCollection3: dw5_sys_coll3,
            GetFontDownloadQueue: dw5_dl_queue,
        },
        TranslateColorGlyphRun4: dw5_color4,
        ComputeGlyphOrigins: dw5_origins1,
        ComputeGlyphOrigins2: dw5_origins2,
    },
    CreateFontSetBuilder5: dw5_create_fsb,
    CreateInMemoryFontFileLoader: dw5_create_iml,
    CreateHttpFontFileLoader: dw5_http_loader,
    AnalyzeContainerType: dw5_container,
    UnpackFontFile: dw5_unpack,
};

#[no_mangle]
pub unsafe extern "system" fn DWriteCreateFactory(
    factory_type: i32,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || iid.is_null() {
        return E_INVALIDARG;
    }
    static SLOT: AtomicUsize = AtomicUsize::new(UNRESOLVED);
    let Some(addr) = (unsafe {
        resolve_cached(&SLOT, &DWRITE_W, true, b"DWriteCreateFactory\0")
    }) else {
        unsafe { *out = ptr::null_mut() };
        return E_NOINTERFACE;
    };
    let create: unsafe extern "system" fn(i32, *const GUID, *mut *mut c_void) -> HRESULT =
        unsafe { core::mem::transmute(addr) };
    let hr = unsafe { create(factory_type, iid, out) };
    if hr >= 0 {
        return hr;
    }
    let mut raw_factory = ptr::null_mut();
    let hr1 = unsafe { create(factory_type, &IID_IDWRITE_FACTORY, &mut raw_factory) };
    if hr1 < 0 || raw_factory.is_null() {
        unsafe { *out = ptr::null_mut() };
        return hr1;
    }
    static LOADER_REGISTERED: OnceLock<()> = OnceLock::new();
    LOADER_REGISTERED.get_or_init(|| unsafe {
        let _ = ((*com_vtbl::<IDWriteFactory_Vtbl>(raw_factory)).RegisterFontCollectionLoader)(
            raw_factory,
            ptr::addr_of!(GLOBAL_COLLECTION_LOADER).cast_mut().cast(),
        );
    });
    let wrapped = Box::into_raw(Box::new(Win7DWriteFactory5 {
        vtbl: &DWRITE_FACTORY5_VTBL,
        refs: AtomicU32::new(1),
        raw: raw_factory,
    }));
    let qi_hr = unsafe { dw5_qi(wrapped.cast(), iid, out) };
    unsafe { dw5_release(wrapped.cast()) };
    qi_hr
}

#[used]
#[export_name = "__imp_DWriteCreateFactory"]
pub static IMP_DWRITE_CREATE_FACTORY: unsafe extern "system" fn(
    i32,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT = DWriteCreateFactory;

#[inline]
pub fn shim_anchor() -> usize {
    ptr::addr_of!(IMP_DWRITE_CREATE_FACTORY) as usize
}
