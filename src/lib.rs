//! Automatically increment the project's version number.
//!
//! The binary (`bumper`) is a thin wrapper around this library; all logic lives here so it can be
//! unit tested with injected inputs (current date, output sink) rather than global state.
pub mod config;
pub mod version;
