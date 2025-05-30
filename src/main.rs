mod conversion;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::Instant,
};

use clap::{ArgGroup, Parser};
use conversion::convert_simdnbt_to_valence;
use mca::RegionReader;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use valence_nbt::Value;

const REGION_SIZE_IN_CHUNK: usize = 32;

/// Count items in a Minecraft world (1.21.5+), with optional per-item NBT filters and coordinates
#[derive(Parser, Debug)]
#[command(group(ArgGroup::new("mode").required(true).args(["all", "items"])))]
struct CliArgs {
    #[arg(short, long, value_name = "PATH")]
    world_path: PathBuf,

    /// Count all items
    #[arg(long, group = "mode")]
    all: bool,

    /// Specify items to count
    #[arg(
        long = "item",
        value_name = "ITEM_QUERY",
        group = "mode",
        num_args = 1..,
        long_help = "Specify items to count, each in the form: ITEM_ID{nbt}\n\nExamples:\n\n--item minecraft:diamond\n--item minecraft:shulker_box{components:{\"minecraft:custom_name\":\"Portable Chest\"}}"
    )]
    items: Vec<String>,

    /// Also print each matching item's full NBT blob
    #[arg(long = "with-nbt")]
    pub with_nbt: bool,

    /// Also output the coordinates of each matching item
    #[arg(long)]
    with_coords: bool,

    /// Increase output verbosity
    #[arg(short, long)]
    verbose: bool,
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

    let total: u64 = region_files
        .par_iter()
        .map(|path| count_items_in_region_file(path, &queries, &args))
        .sum();

    println!("Total matches: {total}");
    println!("Took {:?}", start.elapsed());
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

fn count_items_in_region_file(
    region_file_path: &PathBuf,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
) -> u64 {
    let data = match std::fs::read(region_file_path) {
        Ok(d) => d,
        Err(e) => {
            if cli_args.verbose {
                eprintln!("Failed to read file {}: {e}", region_file_path.display());
            }
            return 0;
        }
    };

    let region = match RegionReader::new(&data) {
        Ok(r) => r,
        Err(e) => {
            if cli_args.verbose {
                eprintln!(
                    "Failed to parse region file {}: {e}",
                    region_file_path.display(),
                );
            }
            return 0;
        }
    };

    let mut count = 0;

    for cy in 0..REGION_SIZE_IN_CHUNK {
        for cx in 0..REGION_SIZE_IN_CHUNK {
            match region.get_chunk(cx, cy) {
                Ok(Some(chunk)) => {
                    let decompressed = match chunk.decompress() {
                        Ok(d) => d,
                        Err(e) => {
                            if cli_args.verbose {
                                eprintln!(
                                    "Failed to decompress chunk ({cx}, {cy}) in {}: {e}",
                                    region_file_path.display(),
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
                                    region_file_path.display(),
                                );
                            }
                            continue;
                        }
                    };

                    if let Some(block_entities) = nbt
                        .unwrap()
                        .list("block_entities")
                        .and_then(|l| l.compounds())
                    {
                        for be in block_entities {
                            let id = be.string("id").unwrap().to_string();
                            let x = be.int("x").unwrap();
                            let y = be.int("y").unwrap();
                            let z = be.int("z").unwrap();
                            if let Some(items) = be.list("Items").and_then(|l| l.compounds()) {
                                for item in items {
                                    count += count_matching_item(
                                        &id,
                                        (x, y, z),
                                        &item,
                                        item_queries,
                                        cli_args,
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    if cli_args.verbose {
                        eprintln!("No chunk at ({cx}, {cy}) in {}", region_file_path.display());
                    }
                }
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Failed to get chunk ({cx}, {cy}) in {}: {e}",
                            region_file_path.display(),
                        );
                    }
                }
            }
        }
    }

    count
}

fn count_matching_item(
    source_name: &str,
    source_position: (i32, i32, i32),
    item: &simdnbt::borrow::NbtCompound,
    queries: &[ItemFilter],
    args: &CliArgs,
) -> u64 {
    let id = item.string("id").unwrap().to_string();
    let count = item.int("count").unwrap_or(1);

    let mut valence_nbt_item = None;

    let matched = queries.iter().any(|q| {
        if q.id != id {
            return false;
        }

        if let Some(ref nbt_q) = q.required_nbt {
            let val_item = valence_nbt_item.get_or_insert_with(|| convert_simdnbt_to_valence(item));
            nbt_contains_all(val_item, nbt_q)
        } else {
            true
        }
    });

    if matched || queries.is_empty() {
        let (x, y, z) = source_position;
        if args.with_nbt {
            let val_item = valence_nbt_item.unwrap_or_else(|| convert_simdnbt_to_valence(item));
            let snbt = valence_nbt::snbt::to_snbt_string(&val_item);
            println!("[{source_name} @ {x} {y} {z}] {count}x {id} NBT={snbt}");
        } else if args.with_coords {
            println!("[{source_name} @ {x} {y} {z}] {count}x {id}");
        }
        count as u64
    } else {
        0
    }
}

/// Recursively checks if all key-value pairs in `query` are present in `item`.
fn nbt_contains_all(nbt: &Value, required_nbt: &Value) -> bool {
    match (nbt, required_nbt) {
        (Value::Compound(item_map), Value::Compound(query_map)) => {
            query_map.iter().all(|(key, query_value)| {
                item_map
                    .get(key)
                    .is_some_and(|item_value| nbt_contains_all(item_value, query_value))
            })
        }
        (Value::List(item_list), Value::List(query_list)) => {
            // For lists, ensure each element in the query list is present in the item list.
            query_list.iter().all(|query_elem| {
                item_list.iter().any(|item_elem| {
                    nbt_contains_all(&item_elem.to_value(), &query_elem.to_value())
                })
            })
        }
        _ => nbt == required_nbt,
    }
}
