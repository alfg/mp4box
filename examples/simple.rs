use std::env;

// This example demonstrates how to use the `hex_range` function from the `mp4box` crate
// to read and display a hex dump of a specified range of bytes from an MP4 file.
// The user provides the file path, offset, and length as command-line arguments.
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <file> <offset> <length>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    let offset: u64 = args[2].parse()?;
    let length: u64 = args[3].parse()?;

    let mut file = std::fs::File::open(file_path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();

    let dump = mp4box::hex_range(&mut file, file_size, offset, length)?;
    println!("{}", dump.hex);

    Ok(())
}
