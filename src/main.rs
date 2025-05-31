use clap::Parser;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use mc_nbt_scanner::{
    cli::{CliArgs, ViewMode},
    counter::Counter,
    escape_nbt_string, get_region_files, parse_item_args, process_region_file,
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
            eprintln!("Error reading region folder: {err}");
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

    match args.view {
        ViewMode::Detailed => {
            let mut detailed_vec = counter.detailed_counts().iter().collect::<Vec<_>>();
            detailed_vec.sort_by(|(a_key, a_count), (b_key, b_count)| {
                b_count.cmp(a_count).then_with(|| a_key.id.cmp(&b_key.id))
            });

            let mut table = Table::new();
            table.load_preset(presets::UTF8_FULL);
            table.set_content_arrangement(ContentArrangement::Dynamic);

            table.set_header(vec![
                Cell::new("Count").add_attribute(comfy_table::Attribute::Bold),
                Cell::new("Item").add_attribute(comfy_table::Attribute::Bold),
                Cell::new("NBT").add_attribute(comfy_table::Attribute::Bold),
            ]);

            if let Some(col) = table.column_mut(2) {
                col.set_cell_alignment(CellAlignment::Left);
            }

            for (key, &count) in detailed_vec {
                let mut row = vec![Cell::new(count), Cell::new(&key.id)];

                if let Some(snbt) = &key.components_snbt {
                    row.push(Cell::new(snbt));
                }

                table.add_row(row);
            }

            println!("{table}");
        }

        ViewMode::ById => {
            let mut by_id_vec = counter.total_by_id().into_iter().collect::<Vec<_>>();
            by_id_vec.sort_by(|(a_id, a_count), (b_id, b_count)| {
                b_count.cmp(a_count).then_with(|| a_id.cmp(b_id))
            });

            let mut table = Table::new();
            table.load_preset(presets::UTF8_FULL);
            table.set_content_arrangement(ContentArrangement::Dynamic);

            table.set_header(vec![
                Cell::new("Count").add_attribute(comfy_table::Attribute::Bold),
                Cell::new("Item ID").add_attribute(comfy_table::Attribute::Bold),
            ]);

            for (id, count) in by_id_vec {
                table.add_row(vec![Cell::new(count), Cell::new(id)]);
            }

            println!("{table}");
        }

        ViewMode::ByNbt => {
            let mut by_nbt_vec = counter.total_by_nbt().into_iter().collect::<Vec<_>>();
            by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
                b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
            });

            let mut table = Table::new();
            table.load_preset(presets::UTF8_FULL);
            table.set_content_arrangement(ContentArrangement::Dynamic);

            table.set_header(vec![
                Cell::new("Count").add_attribute(comfy_table::Attribute::Bold),
                Cell::new("NBT").add_attribute(comfy_table::Attribute::Bold),
            ]);

            if let Some(col) = table.column_mut(1) {
                col.set_cell_alignment(CellAlignment::Left);
            }

            for (nbt_opt, count) in by_nbt_vec {
                let nbt_str = nbt_opt
                    .map(|n| escape_nbt_string(&n))
                    .unwrap_or_else(|| "No NBT".into());
                table.add_row(vec![Cell::new(count), Cell::new(nbt_str)]);
            }

            println!("{table}");
        }
    }

    println!("\nTotal items matched: {}", counter.total());
    println!("Scan completed in {:?}", start.elapsed());
}
