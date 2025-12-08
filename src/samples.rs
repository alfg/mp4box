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
    use crate::registry::StructuredData;
    
    // Look for mdia/mdhd and mdia/hdlr boxes
    if let Some(children) = &trak_box.children {
        for child in children {
            if child.typ == "mdia"
                && let Some(mdia_children) = &child.children
            {
                let mut timescale = 1000; // Default
                let mut duration = 0; // Default
                let mut handler_type = String::from("vide"); // Default

                for mdia_child in mdia_children {
                    if mdia_child.typ == "mdhd" {
                        // Parse timescale and duration from mdhd
                        if let Some(StructuredData::MediaHeader(mdhd_data)) = &mdia_child.structured_data {
                            timescale = mdhd_data.timescale;
                            duration = mdhd_data.duration as u64;
                        }
                    }
                    if mdia_child.typ == "hdlr" {
                        // Parse handler type from hdlr
                        if let Some(StructuredData::HandlerReference(hdlr_data)) = &mdia_child.structured_data {
                            handler_type = hdlr_data.handler_type.clone();
                        }
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

    // Extract structured data directly from child boxes
    if let Some(children) = &stbl_box.children {
        for child in children {
            if let Some(structured_data) = &child.structured_data {
                match structured_data {
                    crate::registry::StructuredData::SampleDescription(data) => {
                        tables.stsd = Some(data.clone());
                    }
                    crate::registry::StructuredData::DecodingTimeToSample(data) => {
                        tables.stts = Some(data.clone());
                    }
                    crate::registry::StructuredData::CompositionTimeToSample(data) => {
                        tables.ctts = Some(data.clone());
                    }
                    crate::registry::StructuredData::SampleToChunk(data) => {
                        tables.stsc = Some(data.clone());
                    }
                    crate::registry::StructuredData::SampleSize(data) => {
                        tables.stsz = Some(data.clone());
                    }
                    crate::registry::StructuredData::SyncSample(data) => {
                        tables.stss = Some(data.clone());
                    }
                    crate::registry::StructuredData::ChunkOffset(data) => {
                        tables.stco = Some(data.clone());
                    }
                    crate::registry::StructuredData::ChunkOffset64(data) => {
                        tables.co64 = Some(data.clone());
                    }
                    // MediaHeader and HandlerReference are not sample table data, ignore them
                    crate::registry::StructuredData::MediaHeader(_) => {},
                    crate::registry::StructuredData::HandlerReference(_) => {},
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

fn get_sample_file_offset(tables: &SampleTables, sample_index: u32) -> u64 {
    // Calculate actual file offset using stsc + stco/co64 + stsz
    
    let stsc = match &tables.stsc {
        Some(data) => data,
        None => return 0, // No chunk mapping available
    };
    
    let stsz = match &tables.stsz {
        Some(data) => data,
        None => return 0, // No sample sizes available
    };
    
    // Get chunk offsets (prefer 64-bit if available)
    let chunk_offsets: Vec<u64> = if let Some(co64) = &tables.co64 {
        co64.chunk_offsets.clone()
    } else if let Some(stco) = &tables.stco {
        stco.chunk_offsets.iter().map(|&offset| offset as u64).collect()
    } else {
        return 0; // No chunk offsets available
    };
    
    // Find which chunk contains this sample (1-based sample indexing in MP4)
    let target_sample = sample_index + 1;
    let mut current_sample = 1u32;
    let mut chunk_index = 0usize;
    let mut samples_per_chunk = 0u32;
    
    for (i, entry) in stsc.entries.iter().enumerate() {
        // Calculate how many samples are covered by previous chunks with this entry's configuration
        let next_first_chunk = if i + 1 < stsc.entries.len() {
            stsc.entries[i + 1].first_chunk
        } else {
            chunk_offsets.len() as u32 + 1 // Beyond last chunk
        };
        
        samples_per_chunk = entry.samples_per_chunk;
        let chunks_with_this_config = next_first_chunk - entry.first_chunk;
        let samples_in_this_range = chunks_with_this_config * samples_per_chunk;
        
        if current_sample + samples_in_this_range > target_sample {
            // Target sample is in this range
            let sample_offset_in_range = target_sample - current_sample;
            chunk_index = (entry.first_chunk - 1) as usize + (sample_offset_in_range / samples_per_chunk) as usize;
            break;
        }
        
        current_sample += samples_in_this_range;
    }
    
    if chunk_index >= chunk_offsets.len() {
        return 0; // Chunk index out of bounds
    }
    
    // Get the base offset of the chunk
    let chunk_offset = chunk_offsets[chunk_index];
    
    // Calculate which sample within the chunk we want
    let sample_in_chunk = ((target_sample - current_sample) % samples_per_chunk) as usize;
    
    // Sum up the sizes of preceding samples in this chunk to get the offset within chunk
    let mut offset_in_chunk = 0u64;
    let chunk_start_sample = current_sample as usize;
    
    // Handle both fixed and variable sample sizes
    if stsz.sample_size > 0 {
        // Fixed sample size for all samples
        offset_in_chunk = sample_in_chunk as u64 * stsz.sample_size as u64;
    } else if !stsz.sample_sizes.is_empty() {
        // Variable sample sizes
        for i in 0..sample_in_chunk {
            let sample_idx = chunk_start_sample + i;
            if sample_idx < stsz.sample_sizes.len() {
                offset_in_chunk += stsz.sample_sizes[sample_idx] as u64;
            }
        }
    }
    
    chunk_offset + offset_in_chunk
}
