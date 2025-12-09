use mp4box::{BoxValue, StructuredData, get_boxes};
use std::fs::File;

fn main() -> anyhow::Result<()> {
    // Check if a file path is provided
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <mp4_file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();

    // Parse with decoding enabled to get structured data
    let boxes = get_boxes(&mut file, size, true)?;

    println!("Analyzing sample tables in: {}", path);
    analyze_sample_tables(&boxes, 0);

    // Also test the direct parsing example
    println!("\nTesting direct parsing example:");
    example_direct_parsing()?;

    Ok(())
}

fn analyze_sample_tables(boxes: &[mp4box::Box], depth: usize) {
    let indent = "  ".repeat(depth);

    for box_info in boxes {
        // Look for sample table boxes
        if let Some(decoded) = &box_info.decoded {
            match box_info.typ.as_str() {
                "stts" => {
                    println!("{}📊 Decoding Time-to-Sample Box (stts):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured sample timing data", indent);
                        // In practice, you would parse the structured data here
                        // For now we show it's working with structured output
                    }
                }
                "stsc" => {
                    println!("{}🗂️  Sample-to-Chunk Box (stsc):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured chunk mapping data", indent);
                    }
                }
                "stsz" => {
                    println!("{}📏 Sample Size Box (stsz):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured sample size data", indent);
                    }
                }
                "stco" => {
                    println!("{}📍 Chunk Offset Box (stco):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured chunk offset data", indent);
                    }
                }
                "co64" => {
                    println!("{}📍 64-bit Chunk Offset Box (co64):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured 64-bit chunk offset data", indent);
                    }
                }
                "stss" => {
                    println!("{}🎯 Sync Sample Box (stss):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured keyframe data", indent);
                    }
                }
                "ctts" => {
                    println!("{}⏰ Composition Time-to-Sample Box (ctts):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured composition offset data", indent);
                    }
                }
                "stsd" => {
                    println!("{}🎬 Sample Description Box (stsd):", indent);
                    if decoded.starts_with("structured:") {
                        println!("{}   Contains structured codec information", indent);
                    }
                }
                _ => {}
            }
        }

        // Recurse into children
        if let Some(children) = &box_info.children {
            analyze_sample_tables(children, depth + 1);
        }
    }
}

/// Example of how you would access structured data directly from the registry
fn example_direct_parsing() -> anyhow::Result<()> {
    use mp4box::boxes::{BoxHeader, FourCC};
    use mp4box::registry::{BoxDecoder, SttsDecoder};
    use std::io::Cursor;

    // Example: Create a mock STTS box data
    // Note: version/flags are handled by the main parser, decoder receives only payload
    let mock_stts_data = vec![
        0, 0, 0, 2, // entry_count = 2
        0, 0, 0, 100, // sample_count = 100
        0, 0, 4, 0, // sample_delta = 1024
        0, 0, 0, 1, // sample_count = 1
        0, 0, 2, 0, // sample_delta = 512
    ];

    let mut cursor = Cursor::new(mock_stts_data);
    let header = BoxHeader {
        typ: FourCC(*b"stts"),
        uuid: None,
        size: 28, // 20 bytes data + 8 bytes header
        header_size: 8,
        start: 0,
    };

    let decoder = SttsDecoder;
    let result = decoder.decode(&mut cursor, &header, Some(0), Some(0))?;

    match result {
        BoxValue::Structured(StructuredData::DecodingTimeToSample(stts_data)) => {
            println!("Parsed STTS data:");
            println!("  Version: {}", stts_data.version);
            println!("  Flags: {}", stts_data.flags);
            println!("  Entry count: {}", stts_data.entry_count);

            for (i, entry) in stts_data.entries.iter().enumerate() {
                println!(
                    "  Entry {}: {} samples, delta {}",
                    i, entry.sample_count, entry.sample_delta
                );
            }
        }
        _ => println!("Unexpected result type"),
    }

    Ok(())
}
