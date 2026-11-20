//! Embeds every `data/naming/<language>.toml` in the repository, so a speech
//! language gets name recognition by adding a file and nothing else.

use std::{env, fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/naming");
    println!("cargo::rerun-if-changed={}", root.display());

    let mut files: Vec<(String, String)> = fs::read_dir(&root)
        .expect("data/naming exists")
        .map(|entry| entry.expect("readable data/naming entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .map(|path| {
            let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
            let file = path.canonicalize().unwrap().display().to_string();
            (tag, file)
        })
        .collect();
    files.sort();

    let entries: String = files
        .iter()
        .map(|(tag, file)| format!("    ({tag:?}, include_str!({file:?})),\n"))
        .collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("naming.rs");
    fs::write(
        out,
        format!("pub(crate) static LANGUAGES: &[(&str, &str)] = &[\n{entries}];\n"),
    )
    .unwrap();
}
