//! mp4box
//!
//! A minimal, dependency-light MP4/ISOBMFF parser for Rust.
//!
//! This crate parses the MP4 box tree (including 64-bit “large” boxes and
//! UUID boxes), classifies known box types, and lets you attach custom
//! decoders to interpret payloads.
//!
//! Typical use cases:
//! - CLIs for inspecting MP4 structure (e.g. `mp4dump`)
//! - Tauri / desktop tools that need JSON output for UI
//! - Backend services that need to inspect or validate MP4 files.
//!
//! # Quick start
//!
//! ```no_run
//! use mp4box::analyze_file;
//!
//! fn main() -> anyhow::Result<()> {
//!     let boxes = analyze_file("video.mp4", /*decode=*/ false)?;
//!     println!("Top-level boxes: {}", boxes.len());
//!     Ok(())
//! }
//! ```
//!
//! For a more advanced example, see the `mp4dump` binary in this repository.

pub mod boxes;
pub mod known_boxes;
pub mod parser;
pub mod registry;
pub mod util;
// if JsonBox / build_json_for_box currently live in mp4dump.rs, move them to lib:
pub mod json_api;

pub use boxes::{BoxHeader, BoxKey, BoxRef, FourCC, NodeKind};
pub use json_api::{HexDump, JsonBox, analyze_file, hex_range};
pub use parser::{parse_children, read_box_header};
pub use registry::{BoxValue, Registry};
