#[cfg(test)]
mod tests {
    use mp4box::registry::{SttsDecoder, BoxDecoder, BoxValue, StructuredData};
    use mp4box::boxes::{BoxHeader, FourCC};
    use std::io::Cursor;

    #[test]
    fn test_stts_structured_decoding() {
        // Create mock STTS box data
        let mock_data = vec![
            0, 0, 0, 0,           // version + flags
            0, 0, 0, 2,           // entry_count = 2
            0, 0, 0, 100,         // sample_count = 100
            0, 0, 4, 0,           // sample_delta = 1024
            0, 0, 0, 1,           // sample_count = 1
            0, 0, 2, 0,           // sample_delta = 512
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stts"),
            uuid: None,
            size: 32,
            header_size: 8,
            start: 0,
        };

        let decoder = SttsDecoder;
        let result = decoder.decode(&mut cursor, &header).unwrap();

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
        use mp4box::registry::{StszDecoder};

        // Create mock STSZ box data with individual sample sizes
        let mock_data = vec![
            0, 0, 0, 0,           // version + flags
            0, 0, 0, 0,           // sample_size = 0 (individual sizes)
            0, 0, 0, 3,           // sample_count = 3
            0, 0, 3, 232,         // size = 1000
            0, 0, 7, 208,         // size = 2000  
            0, 0, 11, 184,        // size = 3000
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsz"),
            uuid: None,
            size: 28,
            header_size: 8,
            start: 0,
        };

        let decoder = StszDecoder;
        let result = decoder.decode(&mut cursor, &header).unwrap();

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
        use mp4box::registry::{StscDecoder};

        // Create mock STSC box data
        let mock_data = vec![
            0, 0, 0, 0,           // version + flags
            0, 0, 0, 2,           // entry_count = 2
            0, 0, 0, 1,           // first_chunk = 1
            0, 0, 0, 5,           // samples_per_chunk = 5
            0, 0, 0, 1,           // sample_description_index = 1
            0, 0, 0, 10,          // first_chunk = 10
            0, 0, 0, 3,           // samples_per_chunk = 3
            0, 0, 0, 1,           // sample_description_index = 1
        ];

        let mut cursor = Cursor::new(mock_data);
        let header = BoxHeader {
            typ: FourCC(*b"stsc"),
            uuid: None,
            size: 36,
            header_size: 8,
            start: 0,
        };

        let decoder = StscDecoder;
        let result = decoder.decode(&mut cursor, &header).unwrap();

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
}