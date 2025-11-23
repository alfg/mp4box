use crate::{
    boxes::{BoxRef, NodeKind},
    parser::read_box_header,
    registry::{default_registry, BoxValue, Registry},
    util::{hex_dump, read_slice},
};
use byteorder::ReadBytesExt;
use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// A JSON-serializable representation of a single MP4 box.
///
/// This is designed for use in UIs (e.g. Tauri frontends) and for JSON output
/// in tools like `mp4dump`.
#[derive(Serialize)]
pub struct JsonBox {
    pub offset: u64,
    pub size: u64,
    pub header_size: u64,            // <- new
    pub payload_offset: Option<u64>, // <- new
    pub payload_size: Option<u64>,   // <- new

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
        let box_end = if h.size == 0 {
            file_len
        } else {
            h.start + h.size
        };

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
                NodeKind::Unknown {
                    data_offset,
                    data_len,
                }
            } else {
                NodeKind::Leaf {
                    data_offset,
                    data_len,
                }
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

fn payload_geometry(b: &BoxRef) -> Option<(u64, u64)> {
    match &b.kind {
        NodeKind::FullBox {
            data_offset,
            data_len,
            ..
        } => Some((*data_offset, *data_len)),
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
            Some((off, len))
        }
        NodeKind::Container(_) => None,
    }
}

fn decode_value(f: &mut File, b: &BoxRef, reg: &Registry) -> Option<String> {
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

fn build_json_for_box(f: &mut File, b: &BoxRef, decode: bool, reg: &Registry) -> JsonBox {
    let hdr = &b.hdr;
    let uuid_str = hdr
        .uuid
        .map(|u| u.iter().map(|b| format!("{:02x}", b)).collect::<String>());

    let kb = crate::known_boxes::KnownBox::from(hdr.typ);
    let full_name = kb.full_name().to_string();

    // basic geometry
    let header_size = hdr.header_size;
    let (payload_offset, payload_size) = payload_geometry(b)
        .map(|(off, len)| (Some(off), Some(len)))
        .unwrap_or((None, None));

    let (version, flags, kind_str, children) = match &b.kind {
        NodeKind::FullBox { version, flags, .. } => {
            (Some(*version), Some(*flags), "full".to_string(), None)
        }
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
        header_size,
        payload_offset,
        payload_size,

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

#[derive(Serialize)]
pub struct HexDump {
    pub offset: u64,
    pub length: u64,
    pub hex: String,
}

/// Hex-dump a range of bytes from an MP4 file.
///
/// `max_len` controls the maximum number of bytes to read. This function
/// never reads past EOF; if `offset + max_len` goes beyond the file size,
/// the returned length will be smaller than `max_len`.
///
/// This is useful for building a hex viewer UI:
///
/// ```no_run
/// use mp4box::hex_range;
///
/// fn main() -> anyhow::Result<()> {
///     let dump = hex_range("video.mp4", 0, 256)?;
///     println!("{}", dump.hex);
///     Ok(())
/// }
/// ```
pub fn hex_range<P: AsRef<Path>>(path: P, offset: u64, max_len: u64) -> anyhow::Result<HexDump> {
    use std::cmp::min;

    let path = path.as_ref().to_path_buf();
    let mut f = File::open(&path)?;
    let file_len = f.metadata()?.len();

    // How many bytes are actually available from this offset to EOF.
    let available = file_len.saturating_sub(offset);

    // Don't read past EOF or more than the caller requested.
    let to_read = min(available, max_len);

    // If nothing is available, just return an empty dump.
    if to_read == 0 {
        return Ok(HexDump {
            offset,
            length: 0,
            hex: String::new(),
        });
    }

    let data = read_slice(&mut f, offset, to_read)?;
    let hex_str = hex_dump(&data, offset);

    Ok(HexDump {
        offset,
        length: to_read, // <-- IMPORTANT: actual bytes read, not max_len
        hex: hex_str,
    })
}
