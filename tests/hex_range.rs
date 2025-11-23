use mp4box::hex_range;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn temp_file(bytes: &[u8], name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let mut f = File::create(&path).expect("create temp file failed");
    f.write_all(bytes).expect("write temp data failed");
    path
}

#[test]
fn hex_range_reads_within_bounds() {
    let data: Vec<u8> = (0u8..64u8).collect();
    let path = temp_file(&data, "mp4box_hex_range_reads.bin");

    // Read 16 bytes starting at offset 16
    let dump = hex_range(&path, 16, 16).expect("hex_range failed");

    assert_eq!(dump.offset, 16);
    assert_eq!(dump.length, 16);
    assert!(!dump.hex.is_empty());
}

#[test]
fn hex_range_clamps_to_eof() {
    let data: Vec<u8> = (0u8..32u8).collect();
    let path = temp_file(&data, "mp4box_hex_range_clamp.bin");

    // Ask for 32 bytes from offset 24; only 8 are available.
    let dump = hex_range(&path, 24, 32).expect("hex_range failed");

    assert_eq!(dump.offset, 24);
    assert_eq!(dump.length, 8);
    assert!(!dump.hex.is_empty());
}
