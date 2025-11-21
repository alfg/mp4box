use crate::{
    boxes::{BoxRef, NodeKind},
    parser::read_box_header,
    registry::{default_registry, Registry, BoxValue},
};
use byteorder::ReadBytesExt;
use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Serialize)]
pub struct JsonBox {
    pub offset: u64,
    pub size: u64,
    pub typ: String,
    pub uuid: Option<String>,
    pub version: Option<u8>,
    pub flags: Option<u32>,
    pub kind: String,
    pub full_name: String,
    pub decoded: Option<String>,
    pub children: Option<Vec<JsonBox>>,
}

/// Synchronous analysis function: parse MP4 and return a box tree.
/// This is what you’ll call from Tauri in a blocking task.
pub fn analyze_file(path: impl AsRef<Path>, decode: bool) -> anyhow::Result<Vec<JsonBox>> {
    let mut f = File::open(&path)?;
    let file_len = f.metadata()?.len();

    // parse top-level boxes
    let mut boxes = Vec::new();
    while f.stream_position()? < file_len {
        let h = read_box_header(&mut f)?;
        let box_end = if h.size == 0 { file_len } else { h.start + h.size };

        let kind = if crate::known_boxes::KnownBox::from(h.typ).is_container() {
            f.seek(SeekFrom::Start(h.start + h.header_size))?;
            NodeKind::Container(crate::parser::parse_children(&mut f, box_end)?)
        } else if crate::known_boxes::KnownBox::from(h.typ).is_full_box() {
            f.seek(SeekFrom::Start(h.start + h.header_size))?;
            let version = f.read_u8()?;
            let mut fl = [0u8; 3];
            f.read_exact(&mut fl)?;
            let flags = ((fl[0] as u32) << 16) | ((fl[1] as u32) << 8) | (fl[2] as u32);
            let data_offset = f.stream_position()?;
            let data_len = box_end.saturating_sub(data_offset);
            NodeKind::FullBox {
                version,
                flags,
                data_offset,
                data_len,
            }
        } else {
            let data_offset = h.start + h.header_size;
            let data_len = box_end.saturating_sub(data_offset);
            if &h.typ.0 == b"uuid" {
                NodeKind::Unknown { data_offset, data_len }
            } else {
                NodeKind::Leaf { data_offset, data_len }
            }
        };

        f.seek(SeekFrom::Start(box_end))?;
        boxes.push(BoxRef { hdr: h, kind });
    }

    // build JSON tree
    let reg = default_registry();
    let mut f2 = File::open(&path)?; // fresh handle for decoding
    let json_boxes = boxes
        .iter()
        .map(|b| build_json_for_box(&mut f2, b, decode, &reg))
        .collect();

    Ok(json_boxes)
}

fn payload_region(b: &BoxRef) -> Option<(crate::boxes::BoxKey, u64, u64)> {
    let key = if &b.hdr.typ.0 == b"uuid" {
        crate::boxes::BoxKey::Uuid(b.hdr.uuid.unwrap())
    } else {
        crate::boxes::BoxKey::FourCC(b.hdr.typ)
    };

    match &b.kind {
        NodeKind::FullBox {
            data_offset,
            data_len,
            ..
        } => Some((key, *data_offset, *data_len)),
        NodeKind::Leaf { .. } | NodeKind::Unknown { .. } => {
            let hdr = &b.hdr;
            if hdr.size == 0 {
                return None;
            }
            let off = hdr.start + hdr.header_size;
            let len = hdr.size.saturating_sub(hdr.header_size);
            if len == 0 {
                return None;
            }
            Some((key, off, len))
        }
        NodeKind::Container(_) => None,
    }
}

fn decode_value(
    f: &mut File,
    b: &BoxRef,
    reg: &Registry,
) -> Option<String> {
    let (key, off, len) = payload_region(b)?;
    if len == 0 {
        return None;
    }

    if f.seek(SeekFrom::Start(off)).is_err() {
        return None;
    }
    let mut limited = f.take(len);

    if let Some(res) = reg.decode(&key, &mut limited, &b.hdr) {
        match res {
            Ok(BoxValue::Text(s)) => Some(s),
            Ok(BoxValue::Bytes(bytes)) => Some(format!("{} bytes", bytes.len())),
            Err(e) => Some(format!("[decode error: {}]", e)),
        }
    } else {
        None
    }
}

fn build_json_for_box(
    f: &mut File,
    b: &BoxRef,
    decode: bool,
    reg: &Registry,
) -> JsonBox {
    let hdr = &b.hdr;
    let uuid_str = hdr.uuid.map(|u| {
        u.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    });

    let kb = crate::known_boxes::KnownBox::from(hdr.typ);
    let full_name = kb.full_name().to_string();

    let (version, flags, kind_str, children) = match &b.kind {
        NodeKind::FullBox { version, flags, .. } => (
            Some(*version),
            Some(*flags),
            "full".to_string(),
            None,
        ),
        NodeKind::Leaf { .. } => (None, None, "leaf".to_string(), None),
        NodeKind::Unknown { .. } => (None, None, "unknown".to_string(), None),
        NodeKind::Container(kids) => {
            let child_nodes = kids
                .iter()
                .map(|c| build_json_for_box(f, c, decode, reg))
                .collect();
            (None, None, "container".to_string(), Some(child_nodes))
        }
    };

    let decoded = if decode {
        decode_value(f, b, reg)
    } else {
        None
    };

    JsonBox {
        offset: hdr.start,
        size: hdr.size,
        typ: hdr.typ.to_string(),
        uuid: uuid_str,
        version,
        flags,
        kind: kind_str,
        full_name,
        decoded,
        children,
    }
}
