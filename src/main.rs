use clap::Parser;
use mc_nbt_scanner::{
    cli::CliArgs, counter::Counter, get_region_files, parse_item_args, process_region_file,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::time::Instant;

fn main() {
    let args = CliArgs::parse();
    let queries = if args.all {
        Vec::new()
    } else {
        parse_item_args(&args.items)
    };

    let region_files = match get_region_files(&args.world_path.join("region")) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let start = Instant::now();

    let counter = region_files
        .into_par_iter()
        .map(|path| {
            let mut c = Counter::new();
            process_region_file(&path, &queries, &args, &mut c);
            c
        })
        .reduce(Counter::new, |mut a, b| {
            a.merge(&b);
            a
        });

    match (args.detailed, args.by_id, args.by_nbt) {
        (true, _, _) => {
            println!("Detailed counts (by item + NBT):");
            for (key, &count) in counter.detailed_counts() {
                println!("{key}: {count}");
            }
        }
        (_, true, _) => {
            println!("Counts by item ID:");
            for (id, count) in counter.total_by_id() {
                println!("{id}: {count}");
            }
        }
        (_, _, true) => {
            println!("Counts by NBT only:");
            for (nbt, count) in counter.total_by_nbt() {
                println!("{}: {count}", nbt.unwrap_or_else(|| "No NBT".to_string()));
            }
        }
        _ => {
            println!("Total items matched: {}", counter.total());
        }
    }

    println!("Scan completed in {:?}", start.elapsed());
}
