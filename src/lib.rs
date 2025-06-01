pub mod cli;
pub mod conversion;
pub mod counter;
pub mod tree;
pub mod view;

use std::{
    fmt,
    io::Cursor,
    path::{Path, PathBuf},
};

use cli::{CliArgs, ItemFilter};
use conversion::convert_simdnbt_to_valence_nbt;
use counter::{Counter, CounterMap};
use mca::RegionReader;
use ptree::print_tree;
use tree::ItemSummaryNode;
use valence_nbt::Value;

pub const CHUNK_PER_REGION_SIDE: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scope {
    pub dimension: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DataType {
    BlockEntity,
    Entity,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DataType::BlockEntity => "Block Entity",
                DataType::Entity => "Entity",
            }
        )
    }
}

pub struct ScanTask {
    pub path: PathBuf,
    pub scope: Scope,
}

pub fn list_mca_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Error: failed to read directory '{}': {e}", dir.display()))?;

    let mut mca_files = Vec::new();
    for entry_res in entries {
        match entry_res {
            Ok(de) => {
                let path = de.path();
                if path.extension().and_then(|e| e.to_str()) == Some("mca") {
                    mca_files.push(path);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to read an entry in '{}': {}",
                    dir.display(),
                    e
                );
            }
        }
    }
    Ok(mca_files)
}

pub fn process_task(task: ScanTask, queries: &[ItemFilter], args: &CliArgs) -> CounterMap {
    let mut counter = Counter::new();
    match task.scope.data_type {
        DataType::BlockEntity => process_region_file(&task, queries, args, &mut counter),
        DataType::Entity => process_entities_file(&task, queries, args, &mut counter),
    }
    let mut map = CounterMap::new();
    map.merge_scope(task.scope, &counter);
    map
}

/// Scans one region file, recursively collects nested items inside block-entity inventories,
/// then prints a collapsed tree for each block-entity (if `--per-source-summary` is set).  
/// Also merges all found items into the global `counter`.
pub fn process_region_file(
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let region_file_path = &task.path;
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

            process_chunk_for_block_entities(&chunk, cy, cx, task, item_queries, cli_args, counter);
        }
    }
}

/// Scans one region file for entities, recursively collects nested items they might contain,
/// then prints a collapsed tree for each entity (if `--per-source-summary` is set).
/// Also merges all found items into the global `counter`.
pub fn process_entities_file(
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let region_file_path = &task.path;
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
            let chunk_data = match region.get_chunk(cx, cy) {
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

            process_chunk_for_entities(&chunk_data, cx, cy, task, item_queries, cli_args, counter);
        }
    }
}

/// Processes a single chunk for block entities.
fn process_chunk_for_block_entities(
    chunk: &mca::RawChunk,
    cy: usize,
    cx: usize,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let region_file_path = &task.path;
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
        process_block_entity(block_entity, task, item_queries, cli_args, counter);
    }
}

/// Processes a single chunk for regular entities.
fn process_chunk_for_entities(
    chunk_data: &mca::RawChunk,
    cx: usize,
    cy: usize,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let region_file_path = &task.path;
    let decompressed = match chunk_data.decompress() {
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

    let nbt_root = match simdnbt::borrow::read(&mut cursor) {
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

    let Some(entities) = nbt_root.list("Entities").and_then(|l| l.compounds()) else {
        return;
    };

    for entity_nbt in entities {
        process_single_entity(entity_nbt, task, item_queries, cli_args, counter);
    }
}

/// Helper to get a formatted string for an entity's position.
fn get_entity_pos_string(entity_nbt: &simdnbt::borrow::NbtCompound) -> String {
    entity_nbt
        .list("Pos")
        .and_then(|pos_list| pos_list.doubles())
        .filter(|doubles| doubles.len() >= 3)
        .map_or_else(
            || "Unknown Pos".to_string(),
            |doubles| format!("{:.2} {:.2} {:.2}", doubles[0], doubles[1], doubles[2]),
        )
}

/// Processes a single entity's NBT data.
fn process_single_entity(
    entity_nbt: simdnbt::borrow::NbtCompound,
    task: &ScanTask,
    queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let Some(id_str) = entity_nbt.string("id") else {
        return;
    };
    let id = id_str.to_string();
    let pos_str = get_entity_pos_string(&entity_nbt);

    let mut summary_nodes = Vec::new();

    for list_field_name in &["Items", "Inventory"] {
        if let Some(item_list) = entity_nbt.list(list_field_name).and_then(|l| l.compounds()) {
            for item_compound in item_list {
                collect_summary_node(
                    &item_compound,
                    cli_args,
                    queries,
                    &mut summary_nodes,
                    counter,
                );
            }
        }
    }

    if let Some(item_compound) = entity_nbt.compound("Item") {
        collect_summary_node(
            &item_compound,
            cli_args,
            queries,
            &mut summary_nodes,
            counter,
        );
    }

    if let Some(holder_compound) = entity_nbt.compound("equipment") {
        for (_key_in_holder, value_nbt) in holder_compound.iter() {
            if let Some(actual_item_compound) = value_nbt.compound() {
                collect_summary_node(
                    &actual_item_compound,
                    cli_args,
                    queries,
                    &mut summary_nodes,
                    counter,
                );
            }
        }
    }

    if let Some(passengers_list) = entity_nbt.list("Passengers").and_then(|l| l.compounds()) {
        for passenger_nbt in passengers_list {
            // Recursively process each passenger.
            // The passenger's items will be added to the current entity's summary_nodes
            // and the global_counter. This is generally fine as the per-source summary
            // is for the top-level entity being processed from the chunk.
            process_single_entity(passenger_nbt, task, queries, cli_args, counter);
        }
    }

    if cli_args.per_source_summary && !summary_nodes.is_empty() {
        let root_label = format!("[{}] {id} @ {pos_str}", task.scope.dimension);
        let mut root = ItemSummaryNode::new_root(root_label, summary_nodes);

        root.collapse_leaves_recursive();

        print_tree(&root).unwrap();
    }
}

fn process_block_entity(
    block_entity: simdnbt::borrow::NbtCompound,
    task: &ScanTask,
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
        let root_label = format!("[{}] {id} @ {x} {y} {z}", task.scope.dimension);
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
