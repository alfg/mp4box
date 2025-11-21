pub mod boxes;
pub mod parser;
pub mod registry;
pub mod util;
pub mod known_boxes;
// if JsonBox / build_json_for_box currently live in mp4dump.rs, move them to lib:
pub mod json_api;

pub use boxes::{BoxHeader, BoxKey, BoxRef, FourCC, NodeKind};
pub use parser::{parse_children, read_box_header};
pub use registry::{BoxValue, Registry};
pub use json_api::{JsonBox, analyze_file};
