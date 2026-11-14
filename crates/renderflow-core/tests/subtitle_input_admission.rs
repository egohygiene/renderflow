//! Compile the exact owning command and its focused tests independently of
//! unrelated crate-wide cfg(test) modules. No implementation is copied here.
pub use renderflow::{process, timed_text};

#[path = "../src/commands/subtitles.rs"]
pub mod subtitle_command;
