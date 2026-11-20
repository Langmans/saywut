//! Tauri's build step, plus the Windows application manifest for every
//! target. Tauri links against Common Controls v6 (comctl32), which Windows
//! only loads for a binary whose manifest asks for it. tauri-build embeds
//! that manifest in the app binary but not in test binaries, which then fail
//! to start with STATUS_ENTRYPOINT_NOT_FOUND. So the manifest is left to
//! the linker here, which embeds it in binaries and tests alike.

use std::{env, path::Path};

fn main() {
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut windows = tauri_build::WindowsAttributes::new();
    if msvc {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo::rerun-if-changed={}", manifest.display());
        println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri build step");
}
