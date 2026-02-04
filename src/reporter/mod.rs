//! Reporter module for output formatting

pub mod console;
pub mod json;
pub mod sarif;

pub use console::ConsoleReporter;
pub use json::JsonReporter;
pub use sarif::SarifReporter;
