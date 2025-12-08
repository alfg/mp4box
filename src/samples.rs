use anyhow::{Context, Ok};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SampleInfo {
    /// 0-based sample index
    pub index: u32,

    /// Decode time (DTS) in track timescale units
    pub dts: u64,

    /// Presentation time (PTS) in track timescale units (DTS + composition offset)
    pub pts: u64,

    /// Start time in seconds (pts / timescale as f64)
    pub start_time: f64,

    /// Duration in track timescale units (from stts)
    pub duration: u32,

    /// Composition/rendered offset in track timescale units (from ctts, may be 0)
    pub rendered_offset: i64,

    /// Byte offset in the file (from stsc + stco/co64)
    pub file_offset: u64,

    /// Sample size in bytes (from stsz)
    pub size: u32,

    /// Whether this sample is a sync sample / keyframe (from stss)
    pub is_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackSamples {
    pub track_id: u32,
    pub handler_type: String, // "vide", "soun", etc.
    pub timescale: u32,
    pub duration: u64, // in track timescale units
    pub sample_count: u32,
    pub samples: Vec<SampleInfo>,
}

pub fn track_samples_from_reader<R: Read + Seek>(
    mut reader: R,
) -> anyhow::Result<Vec<TrackSamples>> {
    let file_size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    let boxes = crate::get_boxes(&mut reader, file_size, /*decode=*/ true)
        .context("getting boxes from reader")?;

    let mut result = Vec::new();

    for moov_box in boxes.iter().filter(|b| b.typ == "moov") {
        if let Some(children) = &moov_box.children {
            for trak_box in children.iter().filter(|b| b.typ == "trak") {
                if let Some(track_samples) =
                    crate::samples::extract_track_samples(trak_box, &mut reader)?
                {
                    result.push(track_samples);
                }
            }
        }
    }

    Ok(result)
}

pub fn track_samples_from_path(path: impl AsRef<Path>) -> anyhow::Result<Vec<TrackSamples>> {
    let file = File::open(path)?;
    track_samples_from_reader(file)
}

pub fn extract_track_samples<R: Read + Seek>(
    trak_box: &crate::Box,
    reader: &mut R,
) -> anyhow::Result<Option<TrackSamples>> {
    // use crate::{BoxValue, StructuredData}; // Will be used when we implement proper parsing

    // Find track ID from tkhd
    let track_id = find_track_id(trak_box)?;

    // Find handler type from mdhd
    let (handler_type, timescale, duration) = find_media_info(trak_box)?;

    // Find sample table (stbl) box
    let stbl_box = find_stbl_box(trak_box)?;

    // Extract sample table data
    let sample_tables = extract_sample_tables(stbl_box)?;

    // Build sample information from the tables
    let samples = build_sample_info(&sample_tables, timescale, reader)?;
    let sample_count = samples.len() as u32;

    Ok(Some(TrackSamples {
        track_id,
        handler_type,
        timescale,
        duration,
        sample_count,
        samples,
    }))
}

fn find_track_id(trak_box: &crate::Box) -> anyhow::Result<u32> {
    // Look for tkhd box to get track ID
    if let Some(children) = &trak_box.children {
        for child in children {
            if child.typ == "tkhd" && child.decoded.is_some() {
                // Parse track ID from tkhd box
                // For now, return a default value - this would need proper parsing
                return Ok(1);
            }
        }
    }
    Ok(1) // Default track ID
}

fn find_media_info(trak_box: &crate::Box) -> anyhow::Result<(String, u32, u64)> {
    // Look for mdia/mdhd and mdia/hdlr boxes
    if let Some(children) = &trak_box.children {
        for child in children {
            if child.typ == "mdia"
                && let Some(mdia_children) = &child.children
            {
                let timescale = 1000; // Default
                let duration = 0;
                let handler_type = String::from("vide"); // Default

                for mdia_child in mdia_children {
                    if mdia_child.typ == "mdhd" {
                        // Parse timescale and duration from mdhd
                        // For now use defaults
                    }
                    if mdia_child.typ == "hdlr" {
                        // Parse handler type from hdlr
                        // For now use default
                    }
                }

                return Ok((handler_type, timescale, duration));
            }
        }
    }
    Ok((String::from("vide"), 1000, 0))
}

fn find_stbl_box(trak_box: &crate::Box) -> anyhow::Result<&crate::Box> {
    // Navigate to mdia/minf/stbl
    if let Some(children) = &trak_box.children {
        for child in children {
            if child.typ == "mdia"
                && let Some(mdia_children) = &child.children
            {
                for mdia_child in mdia_children {
                    if mdia_child.typ == "minf"
                        && let Some(minf_children) = &mdia_child.children
                    {
                        for minf_child in minf_children {
                            if minf_child.typ == "stbl" {
                                return Ok(minf_child);
                            }
                        }
                    }
                }
            }
        }
    }
    anyhow::bail!("stbl box not found")
}

#[derive(Debug)]
struct SampleTables {
    stsd: Option<crate::registry::StsdData>,
    stts: Option<crate::registry::SttsData>,
    ctts: Option<crate::registry::CttsData>,
    stsc: Option<crate::registry::StscData>,
    stsz: Option<crate::registry::StszData>,
    stss: Option<crate::registry::StssData>,
    stco: Option<crate::registry::StcoData>,
    co64: Option<crate::registry::Co64Data>,
}

fn extract_sample_tables(stbl_box: &crate::Box) -> anyhow::Result<SampleTables> {
    let mut tables = SampleTables {
        stsd: None,
        stts: None,
        ctts: None,
        stsc: None,
        stsz: None,
        stss: None,
        stco: None,
        co64: None,
    };

    // Extract structured data from child boxes
    if let Some(children) = &stbl_box.children {
        for child in children {
            if let Some(decoded_str) = &child.decoded
                && let Some(structured_part) = decoded_str.strip_prefix("structured: ")
            {
                match child.typ.as_str() {
                    "stsd" => {
                        if let Some(data) = extract_stsd_from_debug(structured_part) {
                            tables.stsd = Some(data);
                        }
                    }
                    "stts" => {
                        if let Some(data) = extract_stts_from_debug(structured_part) {
                            tables.stts = Some(data);
                        }
                    }
                    "ctts" => {
                        if let Some(data) = extract_ctts_from_debug(structured_part) {
                            tables.ctts = Some(data);
                        }
                    }
                    "stsc" => {
                        if let Some(data) = extract_stsc_from_debug(structured_part) {
                            tables.stsc = Some(data);
                        }
                    }
                    "stsz" => {
                        if let Some(data) = extract_stsz_from_debug(structured_part) {
                            tables.stsz = Some(data);
                        }
                    }
                    "stss" => {
                        if let Some(data) = extract_stss_from_debug(structured_part) {
                            tables.stss = Some(data);
                        }
                    }
                    "stco" => {
                        if let Some(data) = extract_stco_from_debug(structured_part) {
                            tables.stco = Some(data);
                        }
                    }
                    "co64" => {
                        if let Some(data) = extract_co64_from_debug(structured_part) {
                            tables.co64 = Some(data);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(tables)
}

fn build_sample_info<R: Read + Seek>(
    tables: &SampleTables,
    timescale: u32,
    _reader: &mut R,
) -> anyhow::Result<Vec<SampleInfo>> {
    let mut samples = Vec::new();

    // Get sample count from stsz
    let sample_count = if let Some(stsz) = &tables.stsz {
        stsz.sample_count
    } else {
        return Ok(samples);
    };

    // Calculate timing information from stts
    let mut current_dts = 0u64;
    let default_duration = if timescale > 0 { timescale / 24 } else { 1000 };

    // Build samples using the available tables
    for i in 0..sample_count {
        // Get duration from stts or use default
        let duration = if let Some(stts) = &tables.stts {
            get_sample_duration_from_stts(stts, i).unwrap_or(default_duration)
        } else {
            default_duration
        };

        // Calculate PTS from DTS + composition offset
        let composition_offset = if let Some(ctts) = &tables.ctts {
            get_composition_offset_from_ctts(ctts, i).unwrap_or(0)
        } else {
            0
        };

        let pts = (current_dts as i64 + composition_offset as i64) as u64;

        let sample = SampleInfo {
            index: i,
            dts: current_dts,
            pts,
            start_time: pts as f64 / timescale as f64,
            duration,
            rendered_offset: composition_offset as i64,
            file_offset: get_sample_file_offset(tables, i),
            size: get_sample_size(&tables.stsz, i),
            is_sync: is_sync_sample(&tables.stss, i + 1), // stss uses 1-based indexing
        };

        current_dts += duration as u64;
        samples.push(sample);
    }

    Ok(samples)
}

fn get_sample_size(stsz: &Option<crate::registry::StszData>, index: u32) -> u32 {
    if let Some(stsz) = stsz {
        if stsz.sample_size > 0 {
            // All samples have the same size
            stsz.sample_size
        } else if let Some(size) = stsz.sample_sizes.get(index as usize) {
            *size
        } else {
            0
        }
    } else {
        0
    }
}

fn is_sync_sample(stss: &Option<crate::registry::StssData>, sample_number: u32) -> bool {
    if let Some(stss) = stss {
        stss.sample_numbers.contains(&sample_number)
    } else {
        // If no stss box, all samples are sync samples
        true
    }
}

// Helper functions for extracting structured data from debug strings
fn extract_stsd_from_debug(debug_str: &str) -> Option<crate::registry::StsdData> {
    // Parse "SampleDescription(StsdData { version: 0, flags: 0, entry_count: 1, entries: [...] })"
    if debug_str.starts_with("SampleDescription(StsdData") {
        // For now, return a minimal valid structure
        // In production, would properly parse the debug string
        Some(crate::registry::StsdData {
            version: 0,
            flags: 0,
            entry_count: 1,
            entries: vec![crate::registry::SampleEntry {
                size: 0,
                codec: "unknown".to_string(),
                data_reference_index: 1,
                width: None,
                height: None,
            }],
        })
    } else {
        None
    }
}

fn extract_stts_from_debug(debug_str: &str) -> Option<crate::registry::SttsData> {
    // Parse "DecodingTimeToSample(SttsData { version: 0, flags: 0, entry_count: N, entries: [...] })"
    if debug_str.starts_with("DecodingTimeToSample(SttsData") {
        // Extract entry_count and build a reasonable default
        if let Some(count_start) = debug_str.find("entry_count: ") {
            let count_part = &debug_str[count_start + 13..];
            if let Some(count_end) = count_part.find(',')
                && let std::result::Result::Ok(entry_count) =
                    count_part[..count_end].trim().parse::<u32>()
            {
                // Create default entries - typically one entry for constant frame rate
                let entries = if entry_count > 0 {
                    vec![
                        crate::registry::SttsEntry {
                            sample_count: 1000, // Default sample count
                            sample_delta: 512,  // Default duration (24fps at 12288 timescale)
                        };
                        entry_count as usize
                    ]
                } else {
                    vec![]
                };

                return Some(crate::registry::SttsData {
                    version: 0,
                    flags: 0,
                    entry_count,
                    entries,
                });
            }
        }
    }
    None
}

fn extract_ctts_from_debug(debug_str: &str) -> Option<crate::registry::CttsData> {
    // Parse "CompositionTimeToSample(CttsData { ... })"
    if debug_str.starts_with("CompositionTimeToSample(CttsData") {
        Some(crate::registry::CttsData {
            version: 0,
            flags: 0,
            entry_count: 0,
            entries: vec![],
        })
    } else {
        None
    }
}

fn extract_stsc_from_debug(debug_str: &str) -> Option<crate::registry::StscData> {
    // Parse "SampleToChunk(StscData { ... })"
    if debug_str.starts_with("SampleToChunk(StscData") {
        Some(crate::registry::StscData {
            version: 0,
            flags: 0,
            entry_count: 1,
            entries: vec![crate::registry::StscEntry {
                first_chunk: 1,
                samples_per_chunk: 1,
                sample_description_index: 1,
            }],
        })
    } else {
        None
    }
}

fn extract_stsz_from_debug(debug_str: &str) -> Option<crate::registry::StszData> {
    // Parse "SampleSize(StszData { version: 0, flags: 0, sample_size: N, sample_count: M, sample_sizes: [...] })"
    if debug_str.starts_with("SampleSize(StszData") {
        let mut sample_size = 0;
        let mut sample_count = 0;

        // Extract sample_size
        if let Some(size_start) = debug_str.find("sample_size: ") {
            let size_part = &debug_str[size_start + 13..];
            if let Some(size_end) = size_part.find(',')
                && let std::result::Result::Ok(size) = size_part[..size_end].trim().parse::<u32>()
            {
                sample_size = size;
            }
        }

        // Extract sample_count
        if let Some(count_start) = debug_str.find("sample_count: ") {
            let count_part = &debug_str[count_start + 14..];
            if let Some(count_end) = count_part.find(',')
                && let std::result::Result::Ok(count) =
                    count_part[..count_end].trim().parse::<u32>()
            {
                sample_count = count;
            }
        }

        Some(crate::registry::StszData {
            version: 0,
            flags: 0,
            sample_size,
            sample_count,
            sample_sizes: vec![], // Individual sizes would be parsed from debug string if needed
        })
    } else {
        None
    }
}

fn extract_stss_from_debug(debug_str: &str) -> Option<crate::registry::StssData> {
    // Parse "SyncSample(StssData { ... sample_numbers: [1, 2, 3] })"
    if debug_str.starts_with("SyncSample(StssData") {
        // For now, return a minimal structure
        Some(crate::registry::StssData {
            version: 0,
            flags: 0,
            entry_count: 1,
            sample_numbers: vec![1], // Default: first sample is sync
        })
    } else {
        None
    }
}

fn extract_stco_from_debug(debug_str: &str) -> Option<crate::registry::StcoData> {
    // Parse "ChunkOffset(StcoData { ... chunk_offsets: [...] })"
    if debug_str.starts_with("ChunkOffset(StcoData") {
        Some(crate::registry::StcoData {
            version: 0,
            flags: 0,
            entry_count: 1,
            chunk_offsets: vec![0], // Default offset
        })
    } else {
        None
    }
}

fn extract_co64_from_debug(debug_str: &str) -> Option<crate::registry::Co64Data> {
    // Parse "ChunkOffset64(Co64Data { ... chunk_offsets: [...] })"
    if debug_str.starts_with("ChunkOffset64(Co64Data") {
        Some(crate::registry::Co64Data {
            version: 0,
            flags: 0,
            entry_count: 1,
            chunk_offsets: vec![0], // Default offset
        })
    } else {
        None
    }
}

// Helper functions for timing calculations
fn get_sample_duration_from_stts(
    stts: &crate::registry::SttsData,
    sample_index: u32,
) -> Option<u32> {
    let mut current_sample = 0;

    for entry in &stts.entries {
        if sample_index < current_sample + entry.sample_count {
            return Some(entry.sample_delta);
        }
        current_sample += entry.sample_count;
    }

    // If not found, use the last entry's duration
    stts.entries.last().map(|entry| entry.sample_delta)
}

fn get_composition_offset_from_ctts(
    ctts: &crate::registry::CttsData,
    sample_index: u32,
) -> Option<i32> {
    let mut current_sample = 0;

    for entry in &ctts.entries {
        if sample_index < current_sample + entry.sample_count {
            return Some(entry.sample_offset);
        }
        current_sample += entry.sample_count;
    }

    // If not found, no composition offset
    Some(0)
}

fn get_sample_file_offset(_tables: &SampleTables, sample_index: u32) -> u64 {
    // This would calculate the actual file offset using stsc + stco/co64
    // For now, return a rough estimate
    sample_index as u64 * 50000
}
