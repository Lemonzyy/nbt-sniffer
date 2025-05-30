use std::{io::Cursor, path::PathBuf, time::Instant};

use clap::{ArgGroup, Parser};
use mca::RegionReader;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

const REGION_SIZE_IN_CHUNK: usize = 32;

/// Count items in a Minecraft world (1.21.5+), with optional per-item NBT filters and coordinates
#[derive(Parser, Debug)]
#[command(group(ArgGroup::new("mode").required(true).args(["all", "items"])))]
struct Args {
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
        long_help = "Specify items to count, each in the form: ITEM_ID[:key=value,...]\n\nExamples:\n\n--item minecraft:diamond\n--item minecraft:shulker_box:CustomName=MyBox,Lock=secret"
    )]
    items: Vec<String>,

    /// Also print each matching item's full NBT blob
    #[arg(long = "with-nbt")]
    pub with_nbt: bool,

    /// Also output the coordinates of each matching item
    #[arg(long)]
    with_coords: bool,
}

/// Represents a query for an item and its optional NBT filters
#[derive(Debug)]
pub struct ItemQuery {
    pub id: String,
    pub nbt_query: Option<String>,
}

/// Parse raw CLI `item` arguments into `ItemQuery` structs
/// Each entry is of form `ITEM_ID[namespace:key=value,...]`
pub fn parse_item_queries(raw: &[String]) -> Vec<ItemQuery> {
    raw.into_iter()
        .map(|entry| {
            let mut id = entry.clone();
            let mut nbt_query = None;

            if let Some(start) = entry.find('[') {
                if let Some(end) = entry.rfind(']') {
                    id = entry[..start].to_string();
                    let nbt = entry[start + 1..end].to_string();
                    if !nbt.is_empty() {
                        nbt_query = Some(nbt);
                    }
                }
            }

            if !id.starts_with("minecraft:") {
                id = format!("minecraft:{id}");
            }

            ItemQuery { id, nbt_query }
        })
        .collect()
}
fn main() {
    let args = Args::parse();
    let queries = if args.all {
        Vec::new()
    } else {
        parse_item_queries(&args.items)
    };

    let region_files = match collect_region_files(&args.world_path.join("region")) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let start = Instant::now();

    let total: usize = region_files
        .par_iter()
        .map(|path| process_region(path, &queries, &args))
        .sum();

    println!("Total matches: {}", total);
    println!("Took {:?}", start.elapsed());
}

fn collect_region_files(region_dir: &PathBuf) -> Result<Vec<PathBuf>, String> {
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
                if path.extension().map_or(false, |ext| ext == "mca") {
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

fn process_region(path: &PathBuf, queries: &[ItemQuery], args: &Args) -> usize {
    let data = std::fs::read(path).unwrap_or_default();
    let region = match RegionReader::new(&data) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let mut count = 0;

    for cy in 0..REGION_SIZE_IN_CHUNK {
        for cx in 0..REGION_SIZE_IN_CHUNK {
            if let Ok(Some(chunk)) = region.get_chunk(cx, cy) {
                if let Ok(decompressed) = chunk.decompress() {
                    let mut cursor = Cursor::new(decompressed.as_slice());
                    if let Ok(nbt) = simdnbt::borrow::read(&mut cursor) {
                        if let Some(block_entities) = nbt
                            .unwrap()
                            .list("block_entities")
                            .and_then(|l| l.compounds())
                        {
                            for be in block_entities {
                                let x = be.int("x").unwrap();
                                let y = be.int("y").unwrap();
                                let z = be.int("z").unwrap();
                                if let Some(items) = be.list("Items").and_then(|l| l.compounds()) {
                                    for item in items {
                                        count += match_item(&item, queries, args, (x, y, z));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    count
}

fn match_item(
    item: &simdnbt::borrow::NbtCompound,
    queries: &[ItemQuery],
    args: &Args,
    coordinates: (i32, i32, i32),
) -> usize {
    let id = item.string("id").map(|s| s.to_string_lossy().to_string());
    if id.is_none() {
        return 0;
    }
    let id = id.unwrap();
    let count = item.int("count").unwrap_or(1);

    let mut matched = false;
    if queries.is_empty() {
        matched = true;
    } else {
        for q in queries {
            if q.id == id {
                matched = true;
                if let Some(ref nbt_q) = q.nbt_query {
                    let nbt_str = format!("{:?}", item);

                    if !nbt_str.contains(nbt_q) {
                        matched = false;
                    }
                }
                if matched {
                    break;
                }
            }
        }
    }

    if matched {
        let (x, y, z) = coordinates;
        if args.with_nbt {
            println!("[{x} {y} {z}] {count}x {id} NBT={item:?}");
        } else if args.with_coords {
            println!("[{x} {y} {z}] {count}x {id}");
        }
        count as usize
    } else {
        0
    }
}
