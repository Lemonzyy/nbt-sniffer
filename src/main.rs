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
            let mut counts = counter.detailed_counts().iter().collect::<Vec<_>>();
            counts.sort_by(|(a_key, a_count), (b_key, b_count)| {
                b_count.cmp(a_count).then_with(|| a_key.id.cmp(&b_key.id))
            });

            for (key, count) in counts {
                println!("\t- {count}x {key}");
            }
        }
        (_, true, _) => {
            println!("Counts by item ID:");
            let mut counts = counter.total_by_id().into_iter().collect::<Vec<_>>();
            counts.sort_by(|(a_id, a_count), (b_id, b_count)| {
                b_count.cmp(a_count).then_with(|| a_id.cmp(b_id))
            });
            for (id, count) in counts {
                println!("\t- {count}x {id}");
            }
        }
        (_, _, true) => {
            println!("Counts by NBT only:");
            let mut counts = counter.total_by_nbt().into_iter().collect::<Vec<_>>();
            counts.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
                b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
            });
            for (nbt, count) in counts {
                println!(
                    "\t- {count}x {}",
                    nbt.unwrap_or_else(|| "No NBT".to_string())
                );
            }
        }
        _ => {}
    }

    println!("Total items matched: {}", counter.total());
    println!("Scan completed in {:?}", start.elapsed());
}
