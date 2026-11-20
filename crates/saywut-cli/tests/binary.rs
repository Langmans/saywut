//! Runs the built `saywut` binary, so `main` is exercised as well.

use std::process::Command;

#[test]
fn version_exits_cleanly() {
    let output = Command::new(env!("CARGO_BIN_EXE_saywut"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("saywut "));
}
