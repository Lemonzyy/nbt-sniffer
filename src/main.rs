use clap::Parser;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use csv::Writer;
use mc_nbt_scanner::{
    DataType, ScanTask, Scope,
    cli::{CliArgs, ViewMode},
    counter::{Counter, CounterMap},
    escape_nbt_string, list_mca_files, parse_item_args, process_region_file,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

fn main() {
    let args = CliArgs::parse();
    let queries = if args.all {
        Vec::new()
    } else {
        parse_item_args(&args.items)
    };

    let world_root = args.world_path.clone();
    let dimension_roots = get_all_dimension_roots(&world_root);
    if dimension_roots.is_empty() {
        eprintln!(
            "No dimension folders (region/ or entities/) found under {}",
            world_root.display()
        );
        return;
    }

    let mut tasks = Vec::new();
    for dim_path in &dimension_roots {
        let dimension = dim_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap()
            .to_string();

        let region_folder = dim_path.join("region");
        if region_folder.is_dir() {
            match list_mca_files(&region_folder) {
                Ok(region_files) => {
                    for region_file in region_files {
                        tasks.push(ScanTask {
                            path: region_file,
                            scope: Scope {
                                dimension: dimension.clone(),
                                data_type: DataType::BlockEntity,
                            },
                        });
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error reading region folder '{}': {err}",
                        region_folder.display()
                    );
                    return;
                }
            }
        }

        let entities_folder = dim_path.join("entities");
        if entities_folder.is_dir() {
            match list_mca_files(&entities_folder) {
                Ok(entity_files) => {
                    for entity_file in entity_files {
                        tasks.push(ScanTask {
                            path: entity_file,
                            scope: Scope {
                                dimension: dimension.clone(),
                                data_type: DataType::Entity,
                            },
                        });
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error reading entities folder '{}': {err}",
                        entities_folder.display()
                    );
                    return;
                }
            }
        }
    }

    let start = Instant::now();

    let counter_map = tasks
        .into_par_iter()
        .map(|ScanTask { path, scope }| match scope.data_type {
            DataType::BlockEntity => {
                let mut c = Counter::new();
                process_region_file(&path, &queries, &args, &mut c);
                let mut map = CounterMap::new();
                map.merge_scope(scope, &c);
                map
            }
            DataType::Entity => {
                let mut c = Counter::new();
                process_entities_file(&path, &queries, &args, &mut c);
                let mut map = CounterMap::new();
                map.merge_scope(scope, &c);
                map
            }
        })
        .reduce(CounterMap::new, |mut a, b| {
            for (scope, counter) in b.iter() {
                a.merge_scope(scope.clone(), counter);
            }
            a
        });

    match args.view {
        ViewMode::Detailed => {
            for (scope, counter) in counter_map.iter() {
                println!("\n{scope:?}");
                let mut detailed_vec = counter.detailed_counts().iter().collect::<Vec<_>>();
                detailed_vec.sort_by(|(a_key, a_count), (b_key, b_count)| {
                    b_count.cmp(a_count).then_with(|| a_key.id.cmp(&b_key.id))
                });

                if args.csv {
                    print_csv(&["Count", "Item", "NBT"], detailed_vec, |(key, &count)| {
                        let nbt_str = key
                            .components_snbt
                            .as_deref()
                            .map(escape_nbt_string)
                            .unwrap_or_default();
                        vec![count.to_string(), key.id.clone(), nbt_str]
                    });
                } else {
                    let mut table = new_table(&["Count", "Item", "NBT"]);
                    if let Some(col) = table.column_mut(2) {
                        col.set_cell_alignment(CellAlignment::Left);
                    }
                    for (key, &count) in detailed_vec {
                        let nbt_cell = key
                            .components_snbt
                            .as_deref()
                            .map(escape_nbt_string)
                            .unwrap_or_default();
                        table.add_row(vec![
                            Cell::new(count),
                            Cell::new(&key.id),
                            Cell::new(nbt_cell),
                        ]);
                    }
                    println!("{table}");
                }
            }
        }

        ViewMode::ById => {
            let mut by_id_vec = counter_map
                .combined()
                .total_by_id()
                .into_iter()
                .collect::<Vec<_>>();
            by_id_vec.sort_by(|(a_id, a_count), (b_id, b_count)| {
                b_count.cmp(a_count).then_with(|| a_id.cmp(b_id))
            });

            if args.csv {
                print_csv(&["Count", "Item ID"], by_id_vec, |(id, count)| {
                    vec![count.to_string(), id]
                });
            } else {
                let mut table = new_table(&["Count", "Item ID"]);
                for (id, count) in by_id_vec {
                    table.add_row(vec![Cell::new(count), Cell::new(id)]);
                }
                println!("{table}");
            }
        }

        ViewMode::ByNbt => {
            let mut by_nbt_vec = counter_map
                .combined()
                .total_by_nbt()
                .into_iter()
                .collect::<Vec<_>>();
            by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
                b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
            });

            if args.csv {
                print_csv(&["Count", "NBT"], by_nbt_vec, |(nbt_opt, count)| {
                    let nbt_str = nbt_opt
                        .as_deref()
                        .map(escape_nbt_string)
                        .unwrap_or_else(|| "No NBT".into());
                    vec![count.to_string(), nbt_str]
                });
            } else {
                let mut table = new_table(&["Count", "NBT"]);
                if let Some(col) = table.column_mut(1) {
                    col.set_cell_alignment(CellAlignment::Left);
                }
                for (nbt_opt, count) in by_nbt_vec {
                    let nbt_str = nbt_opt
                        .as_deref()
                        .map(escape_nbt_string)
                        .unwrap_or_else(|| "No NBT".into());
                    table.add_row(vec![Cell::new(count), Cell::new(nbt_str)]);
                }
                println!("{table}");
            }
        }
    }

    if !args.csv {
        println!("\nTotal items matched: {}", counter_map.combined().total());
        println!("Scan completed in {:?}", start.elapsed());
    }
}

fn process_entities_file(
    path: &Path,
    queries: &[mc_nbt_scanner::ItemFilter],
    args: &CliArgs,
    c: &mut Counter,
) {
    // TODO
}

fn is_dim_root(dir: &Path) -> bool {
    dir.join("region").is_dir() || dir.join("entities").is_dir()
}

pub fn get_all_dimension_roots(world_root: &Path) -> Vec<PathBuf> {
    WalkDir::new(world_root)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            if is_dim_root(path) {
                true
            } else {
                !path.ancestors().skip(1).any(is_dim_root)
            }
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir() && is_dim_root(entry.path()))
        .map(|entry| entry.into_path())
        .collect()
}

fn new_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    let header_cells = headers
        .iter()
        .map(|h| Cell::new(*h).add_attribute(comfy_table::Attribute::Bold));
    table.set_header(header_cells);
    table
}

fn print_csv<I, F>(headers: &[&str], rows: I, row_fn: F)
where
    I: IntoIterator,
    F: Fn(I::Item) -> Vec<String>,
{
    let mut wtr = Writer::from_writer(std::io::stdout());
    wtr.write_record(headers).expect("Failed to write headers");
    for row in rows {
        wtr.write_record(row_fn(row)).expect("Failed to write row");
    }
    wtr.flush().expect("Failed to flush writer");
}
