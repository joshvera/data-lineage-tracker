use data_lineage_tracker::DataLineageTracker;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let mut tracker = DataLineageTracker::new();
    match tracker.analyze_file(&args[1]) {
        Ok(analyzed_tracker) => {
            analyzed_tracker.print_lineage();
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}