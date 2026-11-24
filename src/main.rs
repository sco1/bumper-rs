//! Entry point for the `bumper` binary.
//!
//! All CLI handling lives in [`sco1_bumper::cli_main`].
use std::process::ExitCode;

fn main() -> ExitCode {
    sco1_bumper::cli_main()
}
