pub mod cli;
pub mod conversion;
pub mod counter;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use cli::CliArgs;
use conversion::convert_simdnbt_to_valence_nbt;
use counter::Counter;
use mca::RegionReader;
use valence_nbt::Value;

pub const CHUNK_PER_REGION_SIDE: usize = 32;

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

pub fn get_region_files(region_dir: &Path) -> Result<Vec<PathBuf>, String> {
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

pub fn process_region_file(
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

    for cy in 0..CHUNK_PER_REGION_SIDE {
        for cx in 0..CHUNK_PER_REGION_SIDE {
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

                let mut be_counter = Counter::new();

                if let Some(items) = be.list("Items").and_then(|l| l.compounds()) {
                    items
                        .into_iter()
                        .filter(|item| item_matches(item, item_queries))
                        .for_each(|item| {
                            let id = item.string("id").unwrap().to_string();
                            let count = item.int("count").unwrap_or(1) as u64;

                            let nbt_components = item
                                .compound("components")
                                .map(|comp| convert_simdnbt_to_valence_nbt(&comp));

                            be_counter.add(id.clone(), nbt_components.as_ref(), count);
                        });
                }

                if cli_args.per_source_summary && !be_counter.is_empty() {
                    println!("[{source_id} @ {x} {y} {z}]:",);
                    for (item_key, count) in be_counter.detailed_counts() {
                        if cli_args.show_nbt {
                            if let Some(snbt) = &item_key.components_snbt {
                                println!("\t{count}x {} {snbt}", item_key.id);
                            } else {
                                println!("\t{count}x {}", item_key.id);
                            }
                        } else {
                            println!("\t{count}x {}", item_key.id);
                        }
                    }
                }

                counter.merge(&be_counter);
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
                let val = convert_simdnbt_to_valence_nbt(item);
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

/// Returns `true` if `subset` is entirely contained within `superset`.
/// Compounds require key-by-key subset checks; lists treat each element
/// in `subset_list` as needing its own distinct match in `superset_list`.
pub fn nbt_is_subset(superset: &Value, subset: &Value) -> bool {
    match (superset, subset) {
        // Compounds: every (key → sub_value) must match in sup_map
        (Value::Compound(sup_map), Value::Compound(sub_map)) => {
            sub_map.iter().all(|(field, sub_value)| {
                sup_map
                    .get(field)
                    .is_some_and(|sup_value| nbt_is_subset(sup_value, sub_value))
            })
        }

        // Lists with multiplicity: each sub_element must find a *distinct* match
        // in superset_list, so we track which sup indices are already used.
        (Value::List(superset_list), Value::List(subset_list)) => {
            // track used sup elements
            let mut used = vec![false; superset_list.len()];

            subset_list.iter().all(|sub_element| {
                // try to find an unused sup_element matching this sub_element
                if let Some((idx, _)) = superset_list.iter().enumerate().find(|(i, sup_element)| {
                    !used[*i] && nbt_is_subset(&sup_element.to_value(), &sub_element.to_value())
                }) {
                    used[idx] = true;
                    true
                } else {
                    false
                }
            })
        }

        // Everything else: exact equality
        _ => superset == subset,
    }
}
