use std::env;
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::process::Command;

/// Embeds a Common Controls v6 dependency as a manifest resource into every
/// linked target of this crate — including unit-test executables.
///
/// `tauri`'s `common-controls-v6` feature only embeds the manifest into bin
/// targets, so `cargo test` binaries would load comctl32 5.x and die with
/// STATUS_ENTRYPOINT_NOT_FOUND on `TaskDialogIndirect`. We keep that feature
/// off, let tauri-build skip its own app manifest, and embed exactly one
/// manifest ourselves via `windres`.
#[cfg(target_os = "windows")]
fn embed_common_controls_manifest() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    let manifest = out_dir.join("asb-manifest.xml");
    fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <asmv3:application xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
    <asmv3:windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>
"#,
    )
    .expect("write manifest");
    let rc = out_dir.join("asb-manifest.rc");
    fs::write(&rc, "1 24 \"asb-manifest.xml\"\n").expect("write rc");

    let object = out_dir.join("asb-manifest.o");
    let status = Command::new("windres")
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&object)
        .current_dir(&out_dir)
        .status()
        .expect("windres is available in the GNU toolchain");
    assert!(status.success(), "windres failed to compile the manifest");

    // Applies to bins, tests, and benches of this crate alike.
    println!("cargo:rustc-link-arg={}", object.display());
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", manifest.display());
}

fn main() {
    #[cfg(target_os = "windows")]
    let attrs = {
        // Skip tauri-build's own app manifest; ours above covers every target
        // and a second copy makes the GNU linker merge .rsrc sections fail.
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    };
    #[cfg(not(target_os = "windows"))]
    let attrs = tauri_build::Attributes::new();
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
    #[cfg(target_os = "windows")]
    embed_common_controls_manifest();
}
