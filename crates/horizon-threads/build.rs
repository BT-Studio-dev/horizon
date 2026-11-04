//! The Legacy bundle (`HORIZON_LAUNCHER_LEGACY=1`, set by
//! `.github/workflows/launcher-legacy.yml`) promises to run on SSE4.1-era
//! processors. The mimalloc bundled by `libmimalloc-sys` calls `__popcnt64`
//! unguarded on MSVC x64, which is an illegal instruction there; mark the
//! build so `lib.rs` hands the game the system allocator instead.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(Horizon_legacy_build)");
    println!("cargo:rerun-if-env-changed=HORIZON_LAUNCHER_LEGACY");
    if std::env::var("HORIZON_LAUNCHER_LEGACY").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=Horizon_legacy_build");
    }
}
