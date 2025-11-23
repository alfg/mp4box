use mp4box::boxes::FourCC;
use mp4box::parser::{parse_children, read_box_header};
use std::io::{Cursor, Seek, SeekFrom};

fn make_minimal_file() -> Vec<u8> {
    // [ftyp box]
    // size: 24 (0x18), type: "ftyp", payload: 16 bytes
    let mut v = Vec::new();

    // size = 24
    v.extend_from_slice(&24u32.to_be_bytes());
    v.extend_from_slice(b"ftyp");
    // major brand "isom"
    v.extend_from_slice(b"isom");
    // minor version
    v.extend_from_slice(&512u32.to_be_bytes());
    // one compatible brand "isom"
    v.extend_from_slice(b"isom");

    v
}

#[test]
fn read_single_ftyp_header() {
    let data = make_minimal_file();
    let mut cur = Cursor::new(data);

    let hdr = read_box_header(&mut cur).expect("read_box_header failed");

    assert_eq!(hdr.start, 0);
    assert_eq!(hdr.size, 24);
    assert_eq!(hdr.typ, FourCC(*b"ftyp"));
    assert_eq!(hdr.header_size, 8);
}

#[test]
fn parse_children_no_children_for_leaf() {
    let data = make_minimal_file();
    let mut cur = Cursor::new(data);
    let len = cur.get_ref().len() as u64;

    let hdr = read_box_header(&mut cur).expect("read_box_header failed");
    let end = hdr.start + hdr.size;
    // seek past payload so parse_children sees no additional boxes
    cur.seek(SeekFrom::Start(end)).unwrap();

    let children = parse_children(&mut cur, len).expect("parse_children failed");
    assert!(children.is_empty());
}
