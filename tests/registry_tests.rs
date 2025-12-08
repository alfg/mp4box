#[cfg(test)]
mod tests {
    use mp4box::boxes::{BoxHeader, BoxKey, FourCC};
    use mp4box::registry::{BoxValue, StructuredData, default_registry};
    use std::io::Cursor;

    #[test]
    fn test_stts_structured_decoding() {
        // Create mock STTS box data (without version/flags - they're parsed separately)
        let mock_data = vec![
            0, 0, 0, 2, // entry_count = 2
            0, 0, 0, 100, // sample_count = 100
            0, 0, 4, 0, // sample_delta = 1024
            0, 0, 0, 1, // sample_count = 1
            0, 0, 2, 0, // sample_delta = 512
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stts"),
            uuid: None,
            size: 32,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stts")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::DecodingTimeToSample(stts_data)) => {
                assert_eq!(stts_data.version, 0);
                assert_eq!(stts_data.flags, 0);
                assert_eq!(stts_data.entry_count, 2);
                assert_eq!(stts_data.entries.len(), 2);

                assert_eq!(stts_data.entries[0].sample_count, 100);
                assert_eq!(stts_data.entries[0].sample_delta, 1024);

                assert_eq!(stts_data.entries[1].sample_count, 1);
                assert_eq!(stts_data.entries[1].sample_delta, 512);
            }
            _ => panic!("Expected structured STTS data"),
        }
    }

    #[test]
    fn test_stsz_structured_decoding() {
        // Create mock STSZ box data with individual sample sizes (without version/flags)
        let mock_data = vec![
            0, 0, 0, 0, // sample_size = 0 (individual sizes)
            0, 0, 0, 3, // sample_count = 3
            0, 0, 3, 232, // size = 1000
            0, 0, 7, 208, // size = 2000
            0, 0, 11, 184, // size = 3000
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsz"),
            uuid: None,
            size: 28,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stsz")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::SampleSize(stsz_data)) => {
                assert_eq!(stsz_data.version, 0);
                assert_eq!(stsz_data.flags, 0);
                assert_eq!(stsz_data.sample_size, 0);
                assert_eq!(stsz_data.sample_count, 3);
                assert_eq!(stsz_data.sample_sizes.len(), 3);

                assert_eq!(stsz_data.sample_sizes[0], 1000);
                assert_eq!(stsz_data.sample_sizes[1], 2000);
                assert_eq!(stsz_data.sample_sizes[2], 3000);
            }
            _ => panic!("Expected structured STSZ data"),
        }
    }

    #[test]
    fn test_stsc_structured_decoding() {
        // Create mock STSC box data (without version/flags)
        let mock_data = vec![
            0, 0, 0, 2, // entry_count = 2
            0, 0, 0, 1, // first_chunk = 1
            0, 0, 0, 5, // samples_per_chunk = 5
            0, 0, 0, 1, // sample_description_index = 1
            0, 0, 0, 10, // first_chunk = 10
            0, 0, 0, 3, // samples_per_chunk = 3
            0, 0, 0, 1, // sample_description_index = 1
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsc"),
            uuid: None,
            size: 36,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stsc")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::SampleToChunk(stsc_data)) => {
                assert_eq!(stsc_data.version, 0);
                assert_eq!(stsc_data.flags, 0);
                assert_eq!(stsc_data.entry_count, 2);
                assert_eq!(stsc_data.entries.len(), 2);

                assert_eq!(stsc_data.entries[0].first_chunk, 1);
                assert_eq!(stsc_data.entries[0].samples_per_chunk, 5);
                assert_eq!(stsc_data.entries[0].sample_description_index, 1);

                assert_eq!(stsc_data.entries[1].first_chunk, 10);
                assert_eq!(stsc_data.entries[1].samples_per_chunk, 3);
                assert_eq!(stsc_data.entries[1].sample_description_index, 1);
            }
            _ => panic!("Expected structured STSC data"),
        }
    }

    #[test]
    fn test_ctts_structured_decoding() {
        // Create mock CTTS box data (without version/flags)
        let mock_data = vec![
            0, 0, 0, 3, // entry_count = 3
            0, 0, 0, 5, // sample_count = 5
            0, 0, 1, 0, // sample_offset = 256
            0, 0, 0, 2, // sample_count = 2
            255, 255, 255, 0, // sample_offset = -256 (signed)
            0, 0, 0, 1, // sample_count = 1
            0, 0, 2, 0, // sample_offset = 512
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"ctts"),
            uuid: None,
            size: 40,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"ctts")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::CompositionTimeToSample(ctts_data)) => {
                assert_eq!(ctts_data.version, 0);
                assert_eq!(ctts_data.flags, 0);
                assert_eq!(ctts_data.entry_count, 3);
                assert_eq!(ctts_data.entries.len(), 3);

                assert_eq!(ctts_data.entries[0].sample_count, 5);
                assert_eq!(ctts_data.entries[0].sample_offset, 256);

                assert_eq!(ctts_data.entries[1].sample_count, 2);
                assert_eq!(ctts_data.entries[1].sample_offset, -256);

                assert_eq!(ctts_data.entries[2].sample_count, 1);
                assert_eq!(ctts_data.entries[2].sample_offset, 512);
            }
            _ => panic!("Expected structured CTTS data"),
        }
    }

    #[test]
    fn test_stss_structured_decoding() {
        // Create mock STSS box data (without version/flags)
        let mock_data = vec![
            0, 0, 0, 4, // entry_count = 4
            0, 0, 0, 1, // sample_number = 1 (keyframe)
            0, 0, 0, 15, // sample_number = 15 (keyframe)
            0, 0, 0, 30, // sample_number = 30 (keyframe)
            0, 0, 0, 45, // sample_number = 45 (keyframe)
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stss"),
            uuid: None,
            size: 28,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stss")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::SyncSample(stss_data)) => {
                assert_eq!(stss_data.version, 0);
                assert_eq!(stss_data.flags, 0);
                assert_eq!(stss_data.entry_count, 4);
                assert_eq!(stss_data.sample_numbers.len(), 4);

                assert_eq!(stss_data.sample_numbers[0], 1);
                assert_eq!(stss_data.sample_numbers[1], 15);
                assert_eq!(stss_data.sample_numbers[2], 30);
                assert_eq!(stss_data.sample_numbers[3], 45);
            }
            _ => panic!("Expected structured STSS data"),
        }
    }

    #[test]
    fn test_stco_structured_decoding() {
        // Create mock STCO box data (without version/flags)
        let mock_data = vec![
            0, 0, 0, 3, // entry_count = 3
            0, 0, 39, 16, // chunk_offset = 10000
            0, 0, 78, 32, // chunk_offset = 20000
            0, 0, 117, 48, // chunk_offset = 30000
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stco"),
            uuid: None,
            size: 28,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stco")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::ChunkOffset(stco_data)) => {
                assert_eq!(stco_data.version, 0);
                assert_eq!(stco_data.flags, 0);
                assert_eq!(stco_data.entry_count, 3);
                assert_eq!(stco_data.chunk_offsets.len(), 3);

                assert_eq!(stco_data.chunk_offsets[0], 10000);
                assert_eq!(stco_data.chunk_offsets[1], 20000);
                assert_eq!(stco_data.chunk_offsets[2], 30000);
            }
            _ => panic!("Expected structured STCO data"),
        }
    }

    #[test]
    fn test_co64_structured_decoding() {
        // Create mock CO64 box data (without version/flags)
        let mock_data = vec![
            0, 0, 0, 2, // entry_count = 2
            0, 0, 0, 0, 0, 0, 39, 16, // chunk_offset = 10000 (64-bit)
            0, 0, 0, 1, 101, 160, 188, 0, // chunk_offset = 6000000000 (64-bit)
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"co64"),
            uuid: None,
            size: 28,
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"co64")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::ChunkOffset64(co64_data)) => {
                assert_eq!(co64_data.version, 0);
                assert_eq!(co64_data.flags, 0);
                assert_eq!(co64_data.entry_count, 2);
                assert_eq!(co64_data.chunk_offsets.len(), 2);

                assert_eq!(co64_data.chunk_offsets[0], 10000);
                assert_eq!(co64_data.chunk_offsets[1], 6000000000);
            }
            _ => panic!("Expected structured CO64 data"),
        }
    }

    #[test]
    fn test_stsd_structured_decoding() {
        // Create mock STSD box data with one video sample entry (without version/flags)
        let mock_data = vec![
            0, 0, 0, 1, // entry_count = 1
            // Sample entry 1 (simplified avc1 entry)
            0, 0, 0, 86, // size = 86 bytes
            b'a', b'v', b'c', b'1', // codec = "avc1"
            0, 0, 0, 0, 0, 0, // reserved
            0, 1, // data_reference_index = 1
            0, 0, // pre_defined
            0, 0, // reserved
            0, 0, 0, 0, 0, 0, 0, 0, // pre_defined[3]
            0, 0, 0, 0, 7, 128, // width = 1920
            4, 56, // height = 1080
            0, 72, 0, 0, // horizresolution = 72 dpi
            0, 72, 0, 0, // vertresolution = 72 dpi
            0, 0, 0, 0, // reserved
            0, 1, // frame_count = 1
            0, 0, 0, 0, 0, 0, 0, 0, // compressorname (32 bytes)
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            24, // depth = 24
            255, 255, // pre_defined
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsd"),
            uuid: None,
            size: 102, // 8 + 4 + 86 + 4 = 102
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stsd")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::SampleDescription(stsd_data)) => {
                assert_eq!(stsd_data.version, 0);
                assert_eq!(stsd_data.flags, 0);
                assert_eq!(stsd_data.entry_count, 1);
                assert_eq!(stsd_data.entries.len(), 1);

                let entry = &stsd_data.entries[0];
                assert_eq!(entry.size, 86);
                assert_eq!(entry.codec, "avc1");
                assert_eq!(entry.data_reference_index, 1); // Default value
                assert_eq!(entry.width, Some(1920));
                assert_eq!(entry.height, Some(1080));
            }
            _ => panic!("Expected structured STSD data"),
        }
    }

    #[test]
    fn test_stsd_audio_structured_decoding() {
        // Create mock STSD box data with one audio sample entry (without version/flags)
        let mock_data = vec![
            0, 0, 0, 1, // entry_count = 1
            // Sample entry 1 (simplified mp4a entry)
            0, 0, 0, 36, // size = 36 bytes
            b'm', b'p', b'4', b'a', // codec = "mp4a"
            0, 0, 0, 0, 0, 0, // reserved
            0, 1, // data_reference_index = 1
            0, 0, 0, 0, // reserved[2]
            0, 0, 0, 0, 0, 2, // channelcount = 2 (stereo)
            0, 16, // samplesize = 16 bits
            0, 0, // pre_defined
            0, 0, // reserved
            172, 68, 0, 0, // samplerate = 44100 Hz (16.16 fixed point)
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsd"),
            uuid: None,
            size: 44, // 8 + 4 + 36 = 48, but we truncate for audio
            header_size: 8,
            start: 0,
        };

        let registry = default_registry();
        let result = registry
            .decode(
                &BoxKey::FourCC(FourCC(*b"stsd")),
                &mut cursor,
                &header,
                Some(0),
                Some(0),
            )
            .unwrap()
            .unwrap();

        match result {
            BoxValue::Structured(StructuredData::SampleDescription(stsd_data)) => {
                assert_eq!(stsd_data.version, 0);
                assert_eq!(stsd_data.flags, 0);
                assert_eq!(stsd_data.entry_count, 1);
                assert_eq!(stsd_data.entries.len(), 1);

                let entry = &stsd_data.entries[0];
                assert_eq!(entry.size, 36);
                assert_eq!(entry.codec, "mp4a");
                assert_eq!(entry.data_reference_index, 1); // Default value
                assert_eq!(entry.width, None); // Audio entries don't have width/height
                assert_eq!(entry.height, None);
            }
            _ => panic!("Expected structured STSD data"),
        }
    }
}
