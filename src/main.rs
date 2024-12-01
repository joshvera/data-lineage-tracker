use data_lineage_tracker::DataLineageTracker;
use std::env;
use std::process;

fn main() {
    // Get file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-javascript-file>", args[0]);
        process::exit(1);
    }

    // Initialize tracker with JavaScript language support
    let mut tracker = DataLineageTracker::new();

    // Analyze the file
    match tracker.analyze_file(&args[1]) {
        Ok(()) => {
            println!("\nData Lineage Analysis Results:");
            println!("============================\n");
            tracker.print_lineage();
        }
        Err(e) => {
            eprintln!("Error analyzing file: {}", e);
            process::exit(1);
        }
    }
}
