pub mod cli;
pub mod conversion;
pub mod counter;
pub mod tree;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use cli::CliArgs;
use conversion::convert_simdnbt_to_valence_nbt;
use counter::Counter;
use mca::RegionReader;
use ptree::print_tree;
use tree::ItemSummaryNode;
use valence_nbt::Value;

pub const CHUNK_PER_REGION_SIDE: usize = 32;

/// Represents a query for an item and its optional NBT filters
#[derive(Debug)]
pub struct ItemFilter {
    pub id: Option<String>,
    pub required_nbt: Option<Value>,
}

/// Parse raw CLI `item` arguments into `ItemFilter` structs
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

            let id = if id_str.is_empty() {
                None
            } else if id_str.contains(':') {
                Some(id_str.to_string())
            } else {
                Some(format!("minecraft:{id_str}"))
            };

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

/// Scans one region file, recursively collects nested items inside block-entity inventories,
/// then prints a collapsed tree for each block-entity (if `--per-source-summary` is set).  
/// Also merges all found items into the global `counter`.
pub fn process_region_file(
    region_file_path: &Path,
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

            process_chunk(
                &chunk,
                cy,
                cx,
                region_file_path,
                item_queries,
                cli_args,
                counter,
            );
        }
    }
}

fn process_chunk(
    chunk: &mca::RawChunk,
    cy: usize,
    cx: usize,
    region_file_path: &Path,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let decompressed = match chunk.decompress() {
        Ok(d) => d,
        Err(e) => {
            if cli_args.verbose {
                eprintln!(
                    "Failed to decompress chunk ({cx}, {cy}) in {}: {e}",
                    region_file_path.display()
                );
            }
            return;
        }
    };
    let mut cursor = Cursor::new(decompressed.as_slice());
    let nbt = match simdnbt::borrow::read(&mut cursor) {
        Ok(n) => n.unwrap(),
        Err(e) => {
            if cli_args.verbose {
                eprintln!(
                    "Failed to read NBT data for chunk ({cx}, {cy}) in {}: {e}",
                    region_file_path.display()
                );
            }
            return;
        }
    };

    let Some(block_entities) = nbt.list("block_entities").and_then(|l| l.compounds()) else {
        return;
    };

    for block_entity in block_entities {
        process_block_entity(block_entity, item_queries, cli_args, counter);
    }
}

fn process_block_entity(
    block_entity: simdnbt::borrow::NbtCompound,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let id = block_entity.string("id").unwrap().to_string();
    let x = block_entity.int("x").unwrap();
    let y = block_entity.int("y").unwrap();
    let z = block_entity.int("z").unwrap();

    let mut summary_nodes = Vec::new();
    if let Some(items) = block_entity.list("Items").and_then(|l| l.compounds()) {
        for item in items {
            collect_summary_node(&item, cli_args, item_queries, &mut summary_nodes, counter);
        }
    }

    for field in &["item", "RecordItem", "Book"] {
        if let Some(item) = block_entity.compound(field) {
            collect_summary_node(&item, cli_args, item_queries, &mut summary_nodes, counter);
        }
    }

    if cli_args.per_source_summary && !summary_nodes.is_empty() {
        let root_label = format!("{id} @ {x} {y} {z}");
        let mut root = ItemSummaryNode::new_root(root_label, summary_nodes);

        root.collapse_leaves_recursive();

        print_tree(&root).unwrap();
    }
}

/// Recursively builds an `ItemSummaryNode` for `item_nbt` and all nested children (under `components -> minecraft:container` or `components -> minecraft:bundle_contents`),
/// pushes leaves into `out_nodes`, and also updates the `global_counter`.
fn collect_summary_node(
    item_nbt: &simdnbt::borrow::NbtCompound,
    cli_args: &CliArgs,
    queries: &[ItemFilter],
    out_nodes: &mut Vec<ItemSummaryNode>,
    global_counter: &mut Counter,
) {
    let id = item_nbt.string("id").unwrap().to_string();
    let count = item_nbt.int("count").unwrap_or(1) as u64;

    let matches_filter = if queries.is_empty() {
        true
    } else {
        let valence_nbt = convert_simdnbt_to_valence_nbt(item_nbt);
        queries.iter().any(|q| {
            let id_ok = q.id.as_ref().is_none_or(|qid| qid == &id);
            let nbt_ok = q
                .required_nbt
                .as_ref()
                .is_none_or(|req| nbt_is_subset(&valence_nbt, req));
            id_ok && nbt_ok
        })
    };

    let mut children = Vec::new();

    if let Some(components) = item_nbt.compound("components") {
        if let Some(nested_list) = components
            .list("minecraft:container")
            .and_then(|l| l.compounds())
        {
            for nested_entry in nested_list {
                if let Some(nested_item) = nested_entry.compound("item") {
                    collect_summary_node(
                        &nested_item,
                        cli_args,
                        queries,
                        &mut children,
                        global_counter,
                    );
                }
            }
        }

        if let Some(nested_list) = components
            .list("minecraft:bundle_contents")
            .and_then(|l| l.compounds())
        {
            for nested_entry in nested_list {
                collect_summary_node(
                    &nested_entry,
                    cli_args,
                    queries,
                    &mut children,
                    global_counter,
                );
            }
        }
    }

    if matches_filter {
        let nbt_components = item_nbt
            .compound("components")
            .as_ref()
            .map(convert_simdnbt_to_valence_nbt);

        global_counter.add(id.clone(), nbt_components.as_ref(), count);

        let snbt = if cli_args.show_nbt {
            nbt_components
                .map(|c| valence_nbt::snbt::to_snbt_string(&c))
                .as_deref()
                .map(escape_nbt_string)
        } else {
            None
        };

        let node = ItemSummaryNode::new_item(id.clone(), count, snbt, children);
        out_nodes.push(node);
    } else if !children.is_empty() {
        out_nodes.extend(children);
    }
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

        _ => superset == subset,
    }
}

/// Escape control characters when printing SNBT
pub fn escape_nbt_string(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '\\' => Some("\\\\".to_string()),
            '\n' => Some("\\n".to_string()),
            '\r' => Some("\\r".to_string()),
            '\t' => Some("\\t".to_string()),
            c if c.is_control() => Some(format!("\\u{:04x}", c as u32)),
            _ => Some(c.to_string()),
        })
        .collect::<String>()
}
