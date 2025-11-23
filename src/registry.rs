use crate::boxes::{BoxHeader, BoxKey, FourCC};
use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// A value returned from a box decoder.
///
/// Decoders may return either a human-readable text summary or raw bytes.
#[derive(Debug, Clone)]
pub enum BoxValue {
    Text(String),
    Bytes(Vec<u8>),
}

/// Trait for custom box decoders.
///
/// A decoder is responsible for interpreting the payload of a specific box
/// (identified by a [`BoxKey`]) and returning a [`BoxValue`].
pub trait BoxDecoder: Send + Sync {
    fn decode(&self, r: &mut dyn Read, hdr: &BoxHeader) -> anyhow::Result<BoxValue>;
}

/// Registry of decoders keyed by `BoxKey` (4CC or UUID).
///
/// The registry is immutable once constructed; use [`Registry::with_decoder`]
/// to build it fluently.
pub struct Registry {
    map: HashMap<BoxKey, BoxDecoderEntry>,
}

struct BoxDecoderEntry {
    inner: Box<dyn BoxDecoder>,
    _name: String,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Return a new registry with the given decoder added.
    ///
    /// `name` is human-readable and used only for debugging / logging.
    pub fn with_decoder(mut self, key: BoxKey, name: &str, dec: Box<dyn BoxDecoder>) -> Self {
        self.map.insert(
            key,
            BoxDecoderEntry {
                inner: dec,
                _name: name.to_string(),
            },
        );
        self
    }

    /// Try to decode the payload of a box using a registered decoder.
    ///
    /// Returns `None` if no decoder exists for the given key.
    pub fn decode(
        &self,
        key: &BoxKey,
        r: &mut dyn Read,
        hdr: &BoxHeader,
    ) -> Option<anyhow::Result<BoxValue>> {
        self.map.get(key).map(|d| d.inner.decode(r, hdr))
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------- Helpers ----------

fn read_all(r: &mut dyn Read) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    Ok(buf)
}

fn lang_from_u16(code: u16) -> String {
    let c1 = ((code >> 10) & 0x1F) as u8 + 0x60;
    let c2 = ((code >> 5) & 0x1F) as u8 + 0x60;
    let c3 = (code & 0x1F) as u8 + 0x60;
    format!("{}{}{}", c1 as char, c2 as char, c3 as char,)
}

// ---------- Decoders ----------

// ftyp: major + minor + compatible brands
pub struct FtypDecoder;

impl BoxDecoder for FtypDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        if buf.len() < 8 {
            return Ok(BoxValue::Text(format!(
                "ftyp: payload too short ({} bytes)",
                buf.len()
            )));
        }

        let major = &buf[0..4];
        let mut minor_bytes = [0u8; 4];
        minor_bytes.copy_from_slice(&buf[4..8]);
        let minor = u32::from_be_bytes(minor_bytes);

        let mut brands = Vec::new();
        for chunk in buf[8..].chunks(4) {
            if chunk.len() == 4 {
                brands.push(String::from_utf8_lossy(chunk).to_string());
            }
        }

        Ok(BoxValue::Text(format!(
            "major={} minor={} compatible={:?}",
            String::from_utf8_lossy(major),
            minor,
            brands
        )))
    }
}

// mvhd: timescale + duration
pub struct MvhdDecoder;

impl BoxDecoder for MvhdDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let (timescale, duration) = if version == 1 {
            let _creation = cur.read_u64::<BigEndian>()?;
            let _mod = cur.read_u64::<BigEndian>()?;
            let ts = cur.read_u32::<BigEndian>()?;
            let dur = cur.read_u64::<BigEndian>()?;
            (ts, dur as u64)
        } else {
            let _creation = cur.read_u32::<BigEndian>()?;
            let _mod = cur.read_u32::<BigEndian>()?;
            let ts = cur.read_u32::<BigEndian>()?;
            let dur = cur.read_u32::<BigEndian>()? as u64;
            (ts, dur)
        };

        Ok(BoxValue::Text(format!(
            "timescale={} duration={}",
            timescale, duration
        )))
    }
}

// tkhd: track id, duration, width, height
pub struct TkhdDecoder;

impl BoxDecoder for TkhdDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        if buf.len() < 4 {
            return Ok(BoxValue::Text(format!(
                "tkhd: payload too short ({} bytes)",
                buf.len()
            )));
        }

        let mut pos = 0usize;
        let version = buf[pos];
        pos += 1;
        if pos + 3 > buf.len() {
            return Ok(BoxValue::Text("tkhd: truncated flags".into()));
        }
        pos += 3;

        let read_u32 = |pos: &mut usize| -> Option<u32> {
            if *pos + 4 > buf.len() {
                return None;
            }
            let v = u32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Some(v)
        };
        let read_u64 = |pos: &mut usize| -> Option<u64> {
            if *pos + 8 > buf.len() {
                return None;
            }
            let v = u64::from_be_bytes(buf[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Some(v)
        };

        let track_id;
        let duration;

        if version == 1 {
            // creation_time (8), modification_time (8), track_id (4), reserved (4), duration (8)
            if read_u64(&mut pos).is_none() || read_u64(&mut pos).is_none() {
                return Ok(BoxValue::Text(
                    "tkhd: truncated creation/modification".into(),
                ));
            }
            track_id = read_u32(&mut pos).unwrap_or(0);
            let _ = read_u32(&mut pos); // reserved
            duration = read_u64(&mut pos).unwrap_or(0);
        } else {
            // version 0: creation_time (4), modification_time (4), track_id (4),
            // reserved (4), duration (4)
            if read_u32(&mut pos).is_none() || read_u32(&mut pos).is_none() {
                return Ok(BoxValue::Text(
                    "tkhd: truncated creation/modification".into(),
                ));
            }
            track_id = read_u32(&mut pos).unwrap_or(0);
            let _ = read_u32(&mut pos); // reserved
            duration = read_u32(&mut pos).unwrap_or(0) as u64;
        }

        // reserved[2]
        for _ in 0..2 {
            let _ = read_u32(&mut pos);
        }

        // layer/alt_group/volume/reserved (8 bytes)
        if pos + 8 <= buf.len() {
            pos += 8;
        } else {
            // we still have track/duration, just don't try width/height
            return Ok(BoxValue::Text(format!(
                "track_id={} duration={} (no width/height, short payload)",
                track_id, duration
            )));
        }

        // matrix (36 bytes)
        if pos + 36 <= buf.len() {
            pos += 36;
        } else {
            return Ok(BoxValue::Text(format!(
                "track_id={} duration={} (no width/height, short payload)",
                track_id, duration
            )));
        }

        // width / height
        if pos + 8 <= buf.len() {
            let width = u32::from_be_bytes(buf[pos..pos + 4].try_into().unwrap());
            let height = u32::from_be_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
            Ok(BoxValue::Text(format!(
                "track_id={} duration={} width={} height={}",
                track_id,
                duration,
                width as f32 / 65536.0,
                height as f32 / 65536.0
            )))
        } else {
            Ok(BoxValue::Text(format!(
                "track_id={} duration={} (no width/height, short payload)",
                track_id, duration
            )))
        }
    }
}

// mdhd: timescale, duration, language
pub struct MdhdDecoder;

impl BoxDecoder for MdhdDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        if buf.len() < 4 {
            return Ok(BoxValue::Text(format!(
                "mdhd: payload too short ({} bytes)",
                buf.len()
            )));
        }

        let mut pos = 0usize;
        let version = buf[pos];
        pos += 1;
        if pos + 3 > buf.len() {
            return Ok(BoxValue::Text("mdhd: truncated flags".into()));
        }
        pos += 3;

        let read_u32 = |pos: &mut usize| -> Option<u32> {
            if *pos + 4 > buf.len() {
                return None;
            }
            let v = u32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Some(v)
        };
        let read_u64 = |pos: &mut usize| -> Option<u64> {
            if *pos + 8 > buf.len() {
                return None;
            }
            let v = u64::from_be_bytes(buf[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Some(v)
        };
        let read_u16 = |pos: &mut usize| -> Option<u16> {
            if *pos + 2 > buf.len() {
                return None;
            }
            let v = u16::from_be_bytes(buf[*pos..*pos + 2].try_into().unwrap());
            *pos += 2;
            Some(v)
        };

        let timescale;
        let duration;

        if version == 1 {
            let _ = read_u64(&mut pos);
            let _ = read_u64(&mut pos);
            timescale = read_u32(&mut pos).unwrap_or(0);
            duration = read_u64(&mut pos).unwrap_or(0);
        } else {
            let _ = read_u32(&mut pos);
            let _ = read_u32(&mut pos);
            timescale = read_u32(&mut pos).unwrap_or(0);
            duration = read_u32(&mut pos).unwrap_or(0) as u64;
        }

        // language + pre_defined are optional if payload is short
        let lang = if let Some(lang_code) = read_u16(&mut pos) {
            let _ = read_u16(&mut pos); // pre_defined (ignore if missing)
            lang_from_u16(lang_code)
        } else {
            "???".to_string()
        };

        Ok(BoxValue::Text(format!(
            "timescale={} duration={} language={}",
            timescale, duration, lang
        )))
    }
}

// hdlr: handler type + name
pub struct HdlrDecoder;

impl BoxDecoder for HdlrDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        use byteorder::{BigEndian, ReadBytesExt};

        // pre_defined (4 bytes) + handler_type (4 bytes)
        let _pre_defined = r.read_u32::<BigEndian>()?;
        let mut handler_type = [0u8; 4];
        r.read_exact(&mut handler_type)?;

        // reserved (3 * 4 bytes)
        let mut reserved = [0u8; 12];
        r.read_exact(&mut reserved)?;

        // name: null-terminated string (or just rest of box)
        let mut name_bytes = Vec::new();
        r.read_to_end(&mut name_bytes)?;
        // strip trailing nulls
        while name_bytes.last() == Some(&0) {
            name_bytes.pop();
        }
        let name = String::from_utf8_lossy(&name_bytes).to_string();

        let handler_str = std::str::from_utf8(&handler_type).unwrap_or("????");

        Ok(BoxValue::Text(format!(
            "handler={} name=\"{}\"",
            handler_str, name
        )))
    }
}

// sidx: segment index summary
pub struct SidxDecoder;

impl BoxDecoder for SidxDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let _ref_id = cur.read_u32::<BigEndian>()?;
        let timescale = cur.read_u32::<BigEndian>()?;

        let (earliest, first_offset) = if version == 1 {
            let earliest = cur.read_u64::<BigEndian>()?;
            let first = cur.read_u64::<BigEndian>()?;
            (earliest, first)
        } else {
            let earliest = cur.read_u32::<BigEndian>()? as u64;
            let first = cur.read_u32::<BigEndian>()? as u64;
            (earliest, first)
        };

        let _reserved = cur.read_u16::<BigEndian>()?;
        let ref_count = cur.read_u16::<BigEndian>()?;

        Ok(BoxValue::Text(format!(
            "timescale={} earliest_presentation_time={} first_offset={} references={}",
            timescale, earliest, first_offset, ref_count
        )))
    }
}

// stsd: list sample entry formats, maybe WxH
// ---- stsd decoder: codec + width/height for first entry -----------------
pub struct StsdDecoder;

impl BoxDecoder for StsdDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        use byteorder::{BigEndian, ReadBytesExt};

        // stsd is a FullBox; our reader is already positioned at payload:
        // u32 entry_count
        // [ SampleEntry entries... ]

        let entry_count = r.read_u32::<BigEndian>()?;
        if entry_count == 0 {
            return Ok(BoxValue::Text("entry_count=0".to_string()));
        }

        // First sample entry only (good enough for mp4info-like summary)
        let _entry_size = r.read_u32::<BigEndian>()?;

        let mut codec_bytes = [0u8; 4];
        r.read_exact(&mut codec_bytes)?;
        let codec = std::str::from_utf8(&codec_bytes).unwrap_or("????").to_string();

        // Now we’re at SampleEntry fields.
        // For visual sample entries (avc1/hvc1/etc.), layout is:
        //
        // 6 reserved bytes
        // u16 data_reference_index
        // 16 bytes pre_defined / reserved
        // u16 width
        // u16 height
        //
        // For audio sample entries, this layout is different, so we only
        // try to read width/height for known video codecs.
        let visual_codecs = ["avc1", "hvc1", "hev1", "vp09", "av01"];

        let mut width: Option<u32> = None;
        let mut height: Option<u32> = None;

        if visual_codecs.contains(&codec.as_str()) {
            // Skip reserved + data_reference_index
            let mut skip = [0u8; 6 + 2 + 16];
            r.read_exact(&mut skip)?;

            let w = r.read_u16::<BigEndian>()?;
            let h = r.read_u16::<BigEndian>()?;
            width = Some(w as u32);
            height = Some(h as u32);
        }

        let mut parts = Vec::new();
        parts.push(format!("entry_count={}", entry_count));
        parts.push(format!("codec={}", codec));
        if let Some(w) = width {
            parts.push(format!("width={}", w));
        }
        if let Some(h) = height {
            parts.push(format!("height={}", h));
        }

        Ok(BoxValue::Text(parts.join(" ")))
    }
}

// stts: time-to-sample
pub struct SttsDecoder;

impl BoxDecoder for SttsDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        if buf.len() < 8 {
            return Ok(BoxValue::Text(format!(
                "stts: payload too short ({} bytes)",
                buf.len()
            )));
        }

        let mut pos = 0usize;
        let _version = buf[pos];
        pos += 1;
        if pos + 3 > buf.len() {
            return Ok(BoxValue::Text("stts: truncated flags".into()));
        }
        pos += 3;

        let read_u32 = |pos: &mut usize| -> Option<u32> {
            if *pos + 4 > buf.len() {
                return None;
            }
            let v = u32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Some(v)
        };

        let entry_count = read_u32(&mut pos).unwrap_or(0);

        if entry_count == 0 {
            return Ok(BoxValue::Text("entries=0".into()));
        }

        // best-effort first entry
        let count = read_u32(&mut pos);
        let delta = read_u32(&mut pos);

        if let (Some(c), Some(d)) = (count, delta) {
            Ok(BoxValue::Text(format!(
                "entries={} first: count={} delta={}",
                entry_count, c, d
            )))
        } else {
            Ok(BoxValue::Text(format!(
                "entries={} (no first entry, short payload)",
                entry_count
            )))
        }
    }
}

// stss: sync sample table
pub struct StssDecoder;

impl BoxDecoder for StssDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let _version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let entry_count = cur.read_u32::<BigEndian>()?;
        Ok(BoxValue::Text(format!("sync_sample_count={}", entry_count)))
    }
}

// ctts: composition time to sample
pub struct CttsDecoder;

impl BoxDecoder for CttsDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let entry_count = cur.read_u32::<BigEndian>()?;
        Ok(BoxValue::Text(format!(
            "version={} entries={}",
            version, entry_count
        )))
    }
}

// stsc: sample-to-chunk
pub struct StscDecoder;

impl BoxDecoder for StscDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let _version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut first = None;
        if entry_count > 0 {
            let first_chunk = cur.read_u32::<BigEndian>()?;
            let samples_per_chunk = cur.read_u32::<BigEndian>()?;
            let _sd_idx = cur.read_u32::<BigEndian>()?;
            first = Some((first_chunk, samples_per_chunk));
        }

        Ok(BoxValue::Text(match first {
            Some((fc, spc)) => format!(
                "entries={} first: first_chunk={} samples_per_chunk={}",
                entry_count, fc, spc
            ),
            None => format!("entries={}", entry_count),
        }))
    }
}

// stsz: sample sizes
pub struct StszDecoder;

impl BoxDecoder for StszDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let _version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let sample_size = cur.read_u32::<BigEndian>()?;
        let sample_count = cur.read_u32::<BigEndian>()?;

        Ok(BoxValue::Text(format!(
            "sample_size={} sample_count={}",
            sample_size, sample_count
        )))
    }
}

// stco: 32-bit chunk offsets
pub struct StcoDecoder;

impl BoxDecoder for StcoDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let _version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut first = Vec::new();
        for _ in 0..entry_count.min(3) {
            first.push(cur.read_u32::<BigEndian>()?);
        }

        Ok(BoxValue::Text(format!(
            "entries={} first_offsets={:?}",
            entry_count, first
        )))
    }
}

// co64: 64-bit chunk offsets
pub struct Co64Decoder;

impl BoxDecoder for Co64Decoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        let _version = cur.read_u8()?;
        let _flags = {
            let mut f = [0u8; 3];
            cur.read_exact(&mut f)?;
            ((f[0] as u32) << 16) | ((f[1] as u32) << 8) | (f[2] as u32)
        };

        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut first = Vec::new();
        for _ in 0..entry_count.min(3) {
            first.push(cur.read_u64::<BigEndian>()?);
        }

        Ok(BoxValue::Text(format!(
            "entries={} first_offsets={:?}",
            entry_count, first
        )))
    }
}

// elst: edit list
pub struct ElstDecoder;

impl BoxDecoder for ElstDecoder {
    fn decode(&self, r: &mut dyn Read, _hdr: &BoxHeader) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        if buf.len() < 8 {
            return Ok(BoxValue::Text(format!(
                "elst: payload too short ({} bytes)",
                buf.len()
            )));
        }

        let mut pos = 0usize;
        let version = buf[pos];
        pos += 1;
        if pos + 3 > buf.len() {
            return Ok(BoxValue::Text("elst: truncated flags".into()));
        }
        pos += 3;

        let read_u32 = |pos: &mut usize| -> Option<u32> {
            if *pos + 4 > buf.len() {
                return None;
            }
            let v = u32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Some(v)
        };
        let read_u64 = |pos: &mut usize| -> Option<u64> {
            if *pos + 8 > buf.len() {
                return None;
            }
            let v = u64::from_be_bytes(buf[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Some(v)
        };
        let read_i32 = |pos: &mut usize| -> Option<i32> {
            if *pos + 4 > buf.len() {
                return None;
            }
            let v = i32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Some(v)
        };
        let read_i64 = |pos: &mut usize| -> Option<i64> {
            if *pos + 8 > buf.len() {
                return None;
            }
            let v = i64::from_be_bytes(buf[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Some(v)
        };
        let read_i16 = |pos: &mut usize| -> Option<i16> {
            if *pos + 2 > buf.len() {
                return None;
            }
            let v = i16::from_be_bytes(buf[*pos..*pos + 2].try_into().unwrap());
            *pos += 2;
            Some(v)
        };

        let entry_count = read_u32(&mut pos).unwrap_or(0);

        if entry_count == 0 {
            return Ok(BoxValue::Text(format!("version={} entries=0", version)));
        }

        let (seg_duration, media_time) = if version == 1 {
            let dur = read_u64(&mut pos).unwrap_or(0);
            let mt = read_i64(&mut pos).unwrap_or(0);
            (dur, mt)
        } else {
            let dur = read_u32(&mut pos).unwrap_or(0) as u64;
            let mt = read_i32(&mut pos).unwrap_or(0) as i64;
            (dur, mt)
        };

        let rate_int = read_i16(&mut pos);
        let rate_frac = read_i16(&mut pos);

        match (rate_int, rate_frac) {
            (Some(ri), Some(rf)) => Ok(BoxValue::Text(format!(
                "version={} entries={} first: duration={} media_time={} rate={}/{}",
                version, entry_count, seg_duration, media_time, ri, rf
            ))),
            _ => Ok(BoxValue::Text(format!(
                "version={} entries={} first: duration={} media_time={} (no rate, short payload)",
                version, entry_count, seg_duration, media_time
            ))),
        }
    }
}

// ---------- Default registry ----------
pub fn default_registry() -> Registry {
    use crate::boxes::BoxKey;

    Registry::new()
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"ftyp")),
            "ftyp",
            Box::new(FtypDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"mvhd")),
            "mvhd",
            Box::new(MvhdDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"tkhd")),
            "tkhd",
            Box::new(TkhdDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"mdhd")),
            "mdhd",
            Box::new(MdhdDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"hdlr")),
            "hdlr",
            Box::new(HdlrDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"sidx")),
            "sidx",
            Box::new(SidxDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stsd")),
            "stsd",
            Box::new(StsdDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stts")),
            "stts",
            Box::new(SttsDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stss")),
            "stss",
            Box::new(StssDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"ctts")),
            "ctts",
            Box::new(CttsDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stsc")),
            "stsc",
            Box::new(StscDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stsz")),
            "stsz",
            Box::new(StszDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"stco")),
            "stco",
            Box::new(StcoDecoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"co64")),
            "co64",
            Box::new(Co64Decoder),
        )
        .with_decoder(
            BoxKey::FourCC(FourCC(*b"elst")),
            "elst",
            Box::new(ElstDecoder),
        )
}
