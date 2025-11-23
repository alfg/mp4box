use std::env;

// Analyze an MP4 file and print the number of top-level boxes.
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    // Use the analyze_file function from the mp4box crate
    let boxes = mp4box::analyze_file(&args[1], /*decode=*/ false)?;
    println!("Top-level boxes: {}", boxes.len());

    // Example: print types of all top-level boxes
    let media_info = boxes.iter().find(|b| b.typ == "moov").and_then(|moov_box| {
        moov_box.children.as_ref().and_then(|children| {
            children.iter().find(|b| {
                b.typ == "trak"
                    && b.children
                        .as_ref()
                        .is_some_and(|c| c.iter().any(|cb| cb.typ == "mdia"))
            })
        })
    });
    if let Some(trak_box) = media_info {
        println!("Found a 'trak' box inside 'moov':");
        if let Some(children) = &trak_box.children {
            for child in children {
                println!(" - Child box type: {}", child.typ);
            }
        }
    } else {
        println!("No 'trak' box found inside 'moov'.");
    }

    Ok(())
}
