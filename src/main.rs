//! `qp` executable: thin wrapper around the `quickpick` library.

use std::process::ExitCode;

fn main() -> ExitCode {
    match quickpick::run(quickpick::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("qp: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
