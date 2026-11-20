//! The `saywut` binary; everything runs in the library ([`saywut_cli::run`]).

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(saywut_cli::run(
        std::env::args_os(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))
}
