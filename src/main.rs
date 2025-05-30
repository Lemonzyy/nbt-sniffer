mod cli;
mod conversion;
mod counter;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::Instant,
};

use clap::Parser;
use cli::CliArgs;
use conversion::convert_simdnbt_to_valence;
use counter::Counter;
use mca::RegionReader;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use valence_nbt::Value;

const REGION_SIZE_IN_CHUNK: usize = 32;

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

    if args.detailed {
        for (key, &count) in counter.detailed_counts() {
            println!("{key}: {count}");
        }
    } else if args.by_id {
        for (id, count) in counter.total_by_id() {
            println!("{id}: {count}");
        }
    } else if args.by_nbt {
        for (nbt, count) in counter.total_by_nbt() {
            println!("{}: {count}", nbt.unwrap_or_else(|| "No NBT".to_string()));
        }
    } else {
        println!("Total: {}", counter.total());
    }

    println!("Took {:?}", start.elapsed());
}

/// Represents a query for an item and its optional NBT filters
#[derive(Debug)]
pub struct ItemFilter {
    pub id: String,
    pub required_nbt: Option<Value>,
}

/// Parse raw CLI `item` arguments into `ItemQuery` structs
/// Each entry is of form `ITEM_ID{nbt}`
pub fn parse_item_args(raw_items: &[String]) -> Vec<ItemFilter> {
    raw_items
        .iter()
        .map(|entry| {
            let mut id_str = entry.as_str();
            let mut nbt_query = None;

            if let Some(start) = entry.find('{')
                && let Some(end) = entry.rfind('}')
            {
                id_str = &entry[..start];
                let nbt_str = &entry[start..=end];
                if !nbt_str.is_empty() {
                    match valence_nbt::snbt::from_snbt_str(nbt_str) {
                        Ok(parsed) => nbt_query = Some(parsed),
                        Err(e) => eprintln!("Failed to parse SNBT '{nbt_str}': {e}"),
                    }
                }
            }

            let mut id = id_str.to_string();

            if !id.starts_with("minecraft:") {
                id = format!("minecraft:{id}");
            }

            ItemFilter {
                id,
                required_nbt: nbt_query,
            }
        })
        .collect()
}

fn get_region_files(region_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(region_dir).map_err(|e| {
        format!(
            "Error: failed to read region directory '{}': {e}",
            region_dir.display()
        )
    })?;

    let mut region_files = Vec::new();
    for entry in entries {
        match entry {
            Ok(dir_entry) => {
                let path = dir_entry.path();
                if let Some(ext) = path.extension()
                    && ext == "mca"
                {
                    region_files.push(path);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to read an entry in '{}': {}",
                    region_dir.display(),
                    e
                );
            }
        }
    }
    Ok(region_files)
}

fn process_region_file(
    region_file_path: &PathBuf,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let data = match std::fs::read(region_file_path) {
        Ok(d) => d,
        Err(e) => {
            if cli_args.verbose {
                eprintln!("Failed to read file {}: {e}", region_file_path.display());
            }
            return;
        }
    };

    let region = match RegionReader::new(&data) {
        Ok(r) => r,
        Err(e) => {
            if cli_args.verbose {
                eprintln!(
                    "Failed to parse region file {}: {e}",
                    region_file_path.display()
                );
            }
            return;
        }
    };

    for cy in 0..REGION_SIZE_IN_CHUNK {
        for cx in 0..REGION_SIZE_IN_CHUNK {
            let chunk = match region.get_chunk(cx, cy) {
                Ok(Some(c)) => c,
                Ok(None) => {
                    if cli_args.verbose {
                        eprintln!("No chunk at ({cx}, {cy}) in {}", region_file_path.display());
                    }
                    continue;
                }
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Failed to get chunk ({cx}, {cy}) in {}: {e}",
                            region_file_path.display()
                        );
                    }
                    continue;
                }
            };

            let decompressed = match chunk.decompress() {
                Ok(d) => d,
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Failed to decompress chunk ({cx}, {cy}) in {}: {e}",
                            region_file_path.display()
                        );
                    }
                    continue;
                }
            };

            let mut cursor = Cursor::new(decompressed.as_slice());
            let nbt = match simdnbt::borrow::read(&mut cursor) {
                Ok(n) => n,
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Failed to read NBT data for chunk ({cx}, {cy}) in {}: {e}",
                            region_file_path.display()
                        );
                    }
                    continue;
                }
            };

            let nbt = nbt.unwrap();
            let Some(block_entities) = nbt.list("block_entities").and_then(|l| l.compounds())
            else {
                continue;
            };

            for be in block_entities {
                let source_id = be.string("id").unwrap().to_string();
                let x = be.int("x").unwrap();
                let y = be.int("y").unwrap();
                let z = be.int("z").unwrap();

                if let Some(items) = be.list("Items").and_then(|l| l.compounds()) {
                    items
                        .into_iter()
                        .filter(|item| item_matches(item, item_queries))
                        .for_each(|item| {
                            let id = item.string("id").unwrap().to_string();
                            let count = item.int("count").unwrap_or(1) as u64;

                            let nbt_components = item
                                .compound("components")
                                .map(|comp| convert_simdnbt_to_valence(&comp));

                            counter.add(id.clone(), nbt_components.as_ref(), count);

                            if cli_args.show_coords || cli_args.show_nbt {
                                print_match(&source_id, (x, y, z), &item, count, cli_args);
                            }
                        });
                }
            }
        }
    }
}

fn item_matches(item: &simdnbt::borrow::NbtCompound, queries: &[ItemFilter]) -> bool {
    let id = item.string("id").unwrap().to_string();
    if queries.is_empty() {
        return true;
    }
    for q in queries {
        if q.id == id {
            if let Some(ref required) = q.required_nbt {
                let val = convert_simdnbt_to_valence(item);
                if nbt_is_subset(&val, required) {
                    return true;
                }
            } else {
                return true;
            }
        }
    }
    false
}

fn print_match(
    source_name: &str,
    source_position: (i32, i32, i32),
    item: &simdnbt::borrow::NbtCompound,
    count: u64,
    args: &CliArgs,
) {
    let id = item.string("id").unwrap();
    let (x, y, z) = source_position;

    if args.show_nbt {
        let snbt = valence_nbt::snbt::to_snbt_string(&convert_simdnbt_to_valence(item));
        println!("[{source_name} @ {x} {y} {z}] {count}x {id} NBT={snbt}");
    } else if args.show_coords {
        println!("[{source_name} @ {x} {y} {z}] {count}x {id}");
    }
}

/// Returns `true` if `subset` is entirely contained within `superset`.
/// Compounds still require key-by-key matching; lists are treated as unordered sets.
fn nbt_is_subset(superset: &Value, subset: &Value) -> bool {
    match (superset, subset) {
        // Both compounds: every (key → sub_val) must match in sup_map
        (Value::Compound(sup_map), Value::Compound(sub_map)) => {
            sub_map.iter().all(|(field, sub_val)| {
                sup_map
                    .get(field)
                    .map_or(false, |sup_val| nbt_is_subset(sup_val, sub_val))
            })
        }

        // Lists as unordered: each element in subset_list must match *some* element in superset_list
        (Value::List(superset_list), Value::List(subset_list)) => {
            subset_list.iter().all(|pattern_elem| {
                superset_list
                    .iter()
                    .any(|candidate| nbt_is_subset(&candidate.to_value(), &pattern_elem.to_value()))
            })
        }

        // Everything else: require exact equality
        _ => superset == subset,
    }
}
