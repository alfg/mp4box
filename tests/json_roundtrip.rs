use mp4box::analyze_file;
use serde_json::{self, Value};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// Build a minimal MP4-ish file in a temp location:
/// [ftyp] [free] [mdat]
///
/// These are all leaf boxes so we don't need valid nested structures.
fn make_minimal_mp4_file() -> PathBuf {
    // ftyp: size=24, type="ftyp", payload=16 bytes
    let mut data = Vec::new();

    // size (24)
    data.extend_from_slice(&24u32.to_be_bytes());
    // type "ftyp"
    data.extend_from_slice(b"ftyp");
    // major brand "isom"
    data.extend_from_slice(b"isom");
    // minor version 512
    data.extend_from_slice(&512u32.to_be_bytes());
    // one compatible brand "isom" (just padding for test)
    data.extend_from_slice(b"isom");

    // free: size=8, type="free", no payload
    data.extend_from_slice(&8u32.to_be_bytes());
    data.extend_from_slice(b"free");

    // mdat: size=16, type="mdat", 8 bytes payload
    data.extend_from_slice(&16u32.to_be_bytes());
    data.extend_from_slice(b"mdat");
    data.extend_from_slice(&[0u8; 8]); // dummy payload

    let path = std::env::temp_dir().join("mp4box_json_roundtrip_test.mp4");
    let mut f = File::create(&path).expect("create temp file failed");
    f.write_all(&data).expect("write temp data failed");
    path
}

#[test]
fn analyze_and_serialize_to_json() {
    let path = make_minimal_mp4_file();

    // Parse structure (no decoders needed here)
    let boxes = analyze_file(&path, /*decode=*/ false).expect("analyze_file failed");

    // We at least expect the first box to be our ftyp.
    assert!(!boxes.is_empty(), "no boxes returned from analyze_file");
    assert_eq!(boxes[0].typ, "ftyp");

    // ftyp: size 24, header 8, payload 16
    assert_eq!(boxes[0].size, 24);
    assert_eq!(boxes[0].header_size, 8);
    assert_eq!(boxes[0].payload_size, Some(16));

    // Serialize to JSON
    let json_str = serde_json::to_string(&boxes).expect("serialize to JSON failed");

    // Parse back into a generic Value just to inspect fields are present
    let v: Value = serde_json::from_str(&json_str).expect("parse JSON failed");
    assert!(v.is_array());
    let arr = v.as_array().unwrap();

    // Check that the first entry has the expected keys / values
    let first = &arr[0];
    assert_eq!(first["typ"], "ftyp");
    assert_eq!(first["header_size"], 8);
    assert_eq!(first["payload_size"], 16);

    // sanity: must contain full_name and offset fields as well
    assert!(first.get("full_name").is_some());
    assert!(first.get("offset").is_some());
}
