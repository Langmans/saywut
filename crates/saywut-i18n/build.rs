//! Embeds every `locales/<tag>/saywut.ftl` in the repository, so adding a UI
//! language means adding a folder and nothing else.

use std::{env, fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
    println!("cargo::rerun-if-changed={}", root.display());

    let mut locales: Vec<(String, String)> = fs::read_dir(&root)
        .expect("locales directory exists")
        .map(|entry| entry.expect("readable locales entry").path())
        .filter(|dir| dir.join("saywut.ftl").is_file())
        .map(|dir| {
            let tag = dir.file_name().unwrap().to_string_lossy().into_owned();
            let file = dir.join("saywut.ftl").canonicalize().unwrap();
            (tag, file.display().to_string())
        })
        .collect();
    locales.sort();

    let entries: String = locales
        .iter()
        .map(|(tag, file)| format!("    ({tag:?}, include_str!({file:?})),\n"))
        .collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("locales.rs");
    fs::write(
        out,
        format!("pub(crate) static LOCALES: &[(&str, &str)] = &[\n{entries}];\n"),
    )
    .unwrap();
}
