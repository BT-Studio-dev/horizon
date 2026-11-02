//! Embeds Horizon's icon (`packaging/icons/horizon.ico`) in the Windows executables, as icon
//! resource 1: Explorer and the task bar show it, and the window takes it as its icon.
//! Also compiles Direct3D 10.1 (`Buffer<uint2>`) fallback shaders for Windows 7 SP1 systems
//! whose GPU driver or stock WARP10 (`d3d10warp.dll` without KB2670838) lacks `StructuredBuffer`.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let ico = format!("{dir}/../packaging/icons/horizon.ico");
    let d3d10_hlsl = format!("{dir}/src/win7/d3d10_shaders.hlsl");
    println!("cargo:rerun-if-changed={ico}");
    println!("cargo:rerun-if-changed={d3d10_hlsl}");
    println!("cargo:rerun-if-env-changed=HORIZON_LAUNCHER_LEGACY");
    println!("cargo:rerun-if-env-changed=GPUI_FXC_PATH");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
        win7_shaders::compile(&d3d10_hlsl);
    }
}

#[rustfmt::skip]
#[cfg(windows)]
mod win7_shaders {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const SHADERS: &[(&str, &str, &str)] = &[
        ("win7_quad_vs.dxbc", "vs_4_1", "quad_vertex"),
        ("win7_quad_ps.dxbc", "ps_4_1", "quad_fragment"),
        ("win7_shadow_vs.dxbc", "vs_4_1", "shadow_vertex"),
        ("win7_shadow_ps.dxbc", "ps_4_1", "shadow_fragment"),
        (
            "win7_path_rasterization_vs.dxbc",
            "vs_4_1",
            "path_rasterization_vertex",
        ),
        (
            "win7_path_rasterization_ps.dxbc",
            "ps_4_1",
            "path_rasterization_fragment",
        ),
        ("win7_path_sprite_vs.dxbc", "vs_4_1", "path_sprite_vertex"),
        ("win7_underline_vs.dxbc", "vs_4_1", "underline_vertex"),
        ("win7_underline_ps.dxbc", "ps_4_1", "underline_fragment"),
        (
            "win7_mono_sprite_vs.dxbc",
            "vs_4_1",
            "monochrome_sprite_vertex",
        ),
        (
            "win7_poly_sprite_vs.dxbc",
            "vs_4_1",
            "polychrome_sprite_vertex",
        ),
        (
            "win7_poly_sprite_ps.dxbc",
            "ps_4_1",
            "polychrome_sprite_fragment",
        ),
    ];

    pub fn compile(hlsl_path: &str) {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
        let fxc = find_fxc_compiler();
        for &(out_name, profile, entry) in SHADERS {
            let out_file = out_dir.join(out_name);
            let output = Command::new(&fxc)
                .args([
                    "/nologo",
                    "/T",
                    profile,
                    "/E",
                    entry,
                    "/O3",
                    "/Fo",
                    out_file.to_str().expect("UTF-8 OUT_DIR"),
                    hlsl_path,
                ])
                .output()
                .unwrap_or_else(|e| panic!("failed to invoke {fxc:?} for {entry}: {e}"));
            if !output.status.success() {
                panic!(
                    "fxc failed for {entry} ({profile}):\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }

    fn find_fxc_compiler() -> PathBuf {
        if let Some(p) = std::env::var_os("GPUI_FXC_PATH")
            .map(PathBuf::from)
            .filter(|p| p.exists())
        {
            return p;
        }
        if let Some(p) = Command::new("where.exe")
            .arg("fxc.exe")
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .map(PathBuf::from)
            })
            .filter(|p| p.exists())
        {
            return p;
        }
        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64",
            _ => "x64",
        };
        let mut roots = Vec::new();
        if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
            roots.push(PathBuf::from(pf86).join(r"Windows Kits\10\bin"));
        }
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            roots.push(PathBuf::from(pf).join(r"Windows Kits\10\bin"));
        }
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin"));
        for bin_dir in roots {
            if let Some(found) = newest_sdk_fxc(&bin_dir, arch) {
                return found;
            }
        }
        panic!("Failed to find Windows SDK fxc.exe");
    }

    fn newest_sdk_fxc(bin_dir: &Path, arch: &str) -> Option<PathBuf> {
        let mut candidates: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(bin_dir)
            .ok()?
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let ver: Vec<u32> = entry
                    .file_name()
                    .to_str()?
                    .split('.')
                    .map(|part| part.parse::<u32>().ok())
                    .collect::<Option<Vec<_>>>()?;
                let exe = path.join(arch).join("fxc.exe");
                exe.exists().then_some((ver, exe))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        candidates.pop().map(|(_, exe)| exe)
    }
}
