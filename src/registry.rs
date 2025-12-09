use crate::boxes::{BoxHeader, BoxKey, FourCC};
use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// A value returned from a box decoder.
///
/// Decoders may return either a human-readable text summary, raw bytes, or structured data.
#[derive(Debug, Clone)]
pub enum BoxValue {
    Text(String),
    Bytes(Vec<u8>),
    Structured(StructuredData),
}

/// Structured data for sample table boxes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StructuredData {
    /// Sample Description Box (stsd)
    SampleDescription(StsdData),
    /// Decoding Time-to-Sample Box (stts)
    DecodingTimeToSample(SttsData),
    /// Composition Time-to-Sample Box (ctts)
    CompositionTimeToSample(CttsData),
    /// Sample-to-Chunk Box (stsc)
    SampleToChunk(StscData),
    /// Sample Size Box (stsz)
    SampleSize(StszData),
    /// Sync Sample Box (stss)
    SyncSample(StssData),
    /// Chunk Offset Box (stco)
    ChunkOffset(StcoData),
    /// 64-bit Chunk Offset Box (co64)
    ChunkOffset64(Co64Data),
    /// Media Header Box (mdhd)
    MediaHeader(MdhdData),
    /// Handler Reference Box (hdlr)
    HandlerReference(HdlrData),
    /// Track Header Box (tkhd)
    TrackHeader(TkhdData),
}

/// Sample Description Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StsdData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub entries: Vec<SampleEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SampleEntry {
    pub size: u32,
    pub codec: String,
    pub data_reference_index: u16,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

/// Decoding Time-to-Sample Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SttsData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub entries: Vec<SttsEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SttsEntry {
    pub sample_count: u32,
    pub sample_delta: u32,
}

/// Composition Time-to-Sample Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CttsData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub entries: Vec<CttsEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CttsEntry {
    pub sample_count: u32,
    pub sample_offset: i32, // Can be negative in version 1
}

/// Sample-to-Chunk Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StscData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub entries: Vec<StscEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StscEntry {
    pub first_chunk: u32,
    pub samples_per_chunk: u32,
    pub sample_description_index: u32,
}

/// Sample Size Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StszData {
    pub version: u8,
    pub flags: u32,
    pub sample_size: u32,
    pub sample_count: u32,
    pub sample_sizes: Vec<u32>, // Empty if sample_size > 0
}

/// Sync Sample Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StssData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub sample_numbers: Vec<u32>,
}

/// Chunk Offset Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StcoData {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub chunk_offsets: Vec<u32>,
}

/// 64-bit Chunk Offset Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Co64Data {
    pub version: u8,
    pub flags: u32,
    pub entry_count: u32,
    pub chunk_offsets: Vec<u64>,
}

/// Media Header Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MdhdData {
    pub version: u8,
    pub flags: u32,
    pub creation_time: u32,
    pub modification_time: u32,
    pub timescale: u32,
    pub duration: u32,
    pub language: String,
}

/// Handler Reference Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HdlrData {
    pub version: u8,
    pub flags: u32,
    pub handler_type: String,
    pub name: String,
}

/// Track Header Box data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TkhdData {
    pub version: u8,
    pub flags: u32,
    pub track_id: u32,
    pub duration: u64,
    pub width: f32,
    pub height: f32,
}

/// Trait for custom box decoders.
///
/// A decoder is responsible for interpreting the payload of a specific box
/// (identified by a [`BoxKey`]) and returning a [`BoxValue`].
pub trait BoxDecoder: Send + Sync {
    fn decode(
        &self,
        r: &mut dyn Read,
        hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue>;
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
        version: Option<u8>,
        flags: Option<u32>,
    ) -> Option<anyhow::Result<BoxValue>> {
        self.map
            .get(key)
            .map(|d| d.inner.decode(r, hdr, version, flags))
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
    if code == 0 {
        return "und".to_string();
    }
    let c1 = ((code >> 10) & 0x1F) as u8 + 0x60;
    let c2 = ((code >> 5) & 0x1F) as u8 + 0x60;
    let c3 = (code & 0x1F) as u8 + 0x60;
    format!("{}{}{}", c1 as char, c2 as char, c3 as char,)
}

// ---------- Decoders ----------

// ftyp: major + minor + compatible brands
pub struct FtypDecoder;

impl BoxDecoder for FtypDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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

        // Extract flags as a 24-bit big-endian value
        let flags_bytes = [0, buf[pos], buf[pos + 1], buf[pos + 2]];
        let flags_value = u32::from_be_bytes(flags_bytes);
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
            eprintln!(
                "DEBUG tkhd v1: track_id={}, duration={}",
                track_id, duration
            );
        } else {
            // For version 0, read two 8-byte timestamps then the fields
            let _creation_time = read_u64(&mut pos).unwrap_or(0);
            let _modification_time = read_u64(&mut pos).unwrap_or(0);

            track_id = read_u32(&mut pos).unwrap_or(0);
            let _reserved = read_u32(&mut pos).unwrap_or(0);
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
        let (width, height) = if pos + 8 <= buf.len() {
            let width = u32::from_be_bytes(buf[pos..pos + 4].try_into().unwrap());
            let height = u32::from_be_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
            (width as f32 / 65536.0, height as f32 / 65536.0)
        } else {
            (0.0, 0.0)
        };

        let data = TkhdData {
            version,
            flags: flags_value,
            track_id,
            duration,
            width,
            height,
        };

        Ok(BoxValue::Structured(StructuredData::TrackHeader(data)))
    }
}

// mdhd: timescale, duration, language
pub struct MdhdDecoder;

impl BoxDecoder for MdhdDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let creation_time = r.read_u32::<BigEndian>()?;
        let modification_time = r.read_u32::<BigEndian>()?;
        let timescale = r.read_u32::<BigEndian>()?;
        let duration = r.read_u32::<BigEndian>()?;
        let language_code = r.read_u16::<BigEndian>()?;
        let _pre_defined = r.read_u16::<BigEndian>()?;

        let lang = lang_from_u16(language_code);

        let data = MdhdData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            creation_time,
            modification_time,
            timescale,
            duration,
            language: lang,
        };

        Ok(BoxValue::Structured(StructuredData::MediaHeader(data)))
    }
}

// hdlr: handler type + name
pub struct HdlrDecoder;

impl BoxDecoder for HdlrDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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

        let data = HdlrData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            handler_type: handler_str.to_string(),
            name,
        };

        Ok(BoxValue::Structured(StructuredData::HandlerReference(data)))
    }
}

// sidx: segment index summary
pub struct SidxDecoder;

impl BoxDecoder for SidxDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        use byteorder::{BigEndian, ReadBytesExt};

        // stsd is a FullBox; our reader is already positioned at payload:
        // u32 entry_count
        // [ SampleEntry entries... ]

        let entry_count = r.read_u32::<BigEndian>()?;
        if entry_count == 0 {
            return Ok(BoxValue::Text("entry_count=0".to_string()));
        }

        // First sample entry only (good enough for mp4info-like summary)
        let entry_size = r.read_u32::<BigEndian>()?;

        let mut codec_bytes = [0u8; 4];
        r.read_exact(&mut codec_bytes)?;
        let codec = std::str::from_utf8(&codec_bytes)
            .unwrap_or("????")
            .to_string();

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

        // Create structured data
        let data = StsdData {
            version: _version.unwrap_or(0),
            flags: _flags.unwrap_or(0),
            entry_count,
            entries: vec![SampleEntry {
                size: entry_size,
                codec,
                data_reference_index: 1, // Default value
                width: width.map(|w| w as u16),
                height: height.map(|h| h as u16),
            }],
        };

        Ok(BoxValue::Structured(StructuredData::SampleDescription(
            data,
        )))
    }
}

// stts: time-to-sample
pub struct SttsDecoder;

impl BoxDecoder for SttsDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        // and stripped from the payload. We start directly with the box-specific data.
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut entries = Vec::new();

        for _ in 0..entry_count {
            let sample_count = cur.read_u32::<BigEndian>()?;
            let sample_delta = cur.read_u32::<BigEndian>()?;
            entries.push(SttsEntry {
                sample_count,
                sample_delta,
            });
        }

        let data = SttsData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            entries,
        };

        Ok(BoxValue::Structured(StructuredData::DecodingTimeToSample(
            data,
        )))
    }
}

// stss: sync sample table
pub struct StssDecoder;

impl BoxDecoder for StssDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut sample_numbers = Vec::new();

        for _ in 0..entry_count {
            sample_numbers.push(cur.read_u32::<BigEndian>()?);
        }

        let data = StssData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            sample_numbers,
        };

        Ok(BoxValue::Structured(StructuredData::SyncSample(data)))
    }
}

// ctts: composition time to sample
pub struct CttsDecoder;

impl BoxDecoder for CttsDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut entries = Vec::new();

        for _ in 0..entry_count {
            let sample_count = cur.read_u32::<BigEndian>()?;
            // Note: In version 1, sample_offset can be signed, but since we don't have access
            // to the parsed version here, we assume version 0 behavior (unsigned)
            let sample_offset = cur.read_u32::<BigEndian>()? as i32;
            entries.push(CttsEntry {
                sample_count,
                sample_offset,
            });
        }

        let data = CttsData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            entries,
        };

        Ok(BoxValue::Structured(
            StructuredData::CompositionTimeToSample(data),
        ))
    }
}

// stsc: sample-to-chunk
pub struct StscDecoder;

impl BoxDecoder for StscDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut entries = Vec::new();

        for _ in 0..entry_count {
            let first_chunk = cur.read_u32::<BigEndian>()?;
            let samples_per_chunk = cur.read_u32::<BigEndian>()?;
            let sample_description_index = cur.read_u32::<BigEndian>()?;
            entries.push(StscEntry {
                first_chunk,
                samples_per_chunk,
                sample_description_index,
            });
        }

        let data = StscData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            entries,
        };

        Ok(BoxValue::Structured(StructuredData::SampleToChunk(data)))
    }
}

// stsz: sample sizes
pub struct StszDecoder;

impl BoxDecoder for StszDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let sample_size = cur.read_u32::<BigEndian>()?;
        let sample_count = cur.read_u32::<BigEndian>()?;
        let mut sample_sizes = Vec::new();

        // If sample_size is 0, each sample has its own size
        if sample_size == 0 {
            for _ in 0..sample_count {
                sample_sizes.push(cur.read_u32::<BigEndian>()?);
            }
        }

        let data = StszData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            sample_size,
            sample_count,
            sample_sizes,
        };

        Ok(BoxValue::Structured(StructuredData::SampleSize(data)))
    }
}

// stco: 32-bit chunk offsets
pub struct StcoDecoder;

impl BoxDecoder for StcoDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut chunk_offsets = Vec::new();

        for _ in 0..entry_count {
            chunk_offsets.push(cur.read_u32::<BigEndian>()?);
        }

        let data = StcoData {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            chunk_offsets,
        };

        Ok(BoxValue::Structured(StructuredData::ChunkOffset(data)))
    }
}

// co64: 64-bit chunk offsets
pub struct Co64Decoder;

impl BoxDecoder for Co64Decoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        version: Option<u8>,
        flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let buf = read_all(r)?;
        let mut cur = Cursor::new(&buf);

        // For FullBox types, version and flags are already parsed by the main parser
        let entry_count = cur.read_u32::<BigEndian>()?;
        let mut chunk_offsets = Vec::new();

        for _ in 0..entry_count {
            chunk_offsets.push(cur.read_u64::<BigEndian>()?);
        }

        let data = Co64Data {
            version: version.unwrap_or(0),
            flags: flags.unwrap_or(0),
            entry_count,
            chunk_offsets,
        };

        Ok(BoxValue::Structured(StructuredData::ChunkOffset64(data)))
    }
}

// elst: edit list
pub struct ElstDecoder;

impl BoxDecoder for ElstDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
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
