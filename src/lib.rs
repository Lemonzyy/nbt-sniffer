pub mod cli;
pub mod counter;
pub mod nbt_utils;
pub mod tree;
pub mod view;

use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use cli::{CliArgs, ItemFilter};
use counter::{Counter, CounterMap};
use flate2::read::GzDecoder;
use mca::RegionReader;
use nbt_utils::{convert_simdnbt_to_valence_nbt, get_entity_pos_string};
use ptree::print_tree;
use serde::{Deserialize, Serialize};
use tree::ItemSummaryNode;
use valence_nbt::Value;

const CHUNK_PER_REGION_SIDE: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scope {
    pub dimension: String,
    pub data_type: DataType,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
    strum::EnumIter,
)]
pub enum DataType {
    #[strum(to_string = "Block Entity")]
    BlockEntity,
    #[strum(to_string = "Entity")]
    Entity,
    #[strum(to_string = "Player Data")]
    Player,
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

pub fn process_task(
    task: ScanTask,
    queries: &[ItemFilter],
    args: &CliArgs,
    user_cache: &HashMap<String, String>,
) -> CounterMap {
    let mut counter = Counter::new();
    match task.scope.data_type {
        DataType::BlockEntity => process_region_file(&task, queries, args, &mut counter),
        DataType::Entity => process_entities_file(&task, queries, args, &mut counter),
        DataType::Player => process_player_file(&task, queries, args, &mut counter, user_cache),
    }
    let mut map = CounterMap::new();
    map.merge_scope(task.scope, &counter);
    map
}

/// Generic function to process a region file, iterating through its chunks
/// and applying a given chunk processing function.
fn process_any_region_file<F>(
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
    process_chunk_fn: F,
) where
    F: Fn(&mca::RawChunk, usize, usize, &ScanTask, &[ItemFilter], &CliArgs, &mut Counter),
{
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

    let region_reader = match RegionReader::new(&data) {
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
            let chunk_data = match region_reader.get_chunk(cx, cy) {
                Ok(Some(c)) => c,
                Ok(None) => continue, // No chunk data
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Failed to get chunk ({cx}, {cy}) from {}: {e}",
                            region_file_path.display()
                        );
                    }
                    continue;
                }
            };
            process_chunk_fn(&chunk_data, cx, cy, task, item_queries, cli_args, counter);
        }
    }
}

/// Scans one region file for block entities.
pub fn process_region_file(
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    process_any_region_file(
        task,
        item_queries,
        cli_args,
        counter,
        process_chunk_for_block_entities,
    );
}

/// Scans one region file for regular entities.
/// Also merges all found items into the global `counter`.
pub fn process_entities_file(
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    process_any_region_file(
        task,
        item_queries,
        cli_args,
        counter,
        process_chunk_for_entities,
    );
}

/// Generic function to process NBT data from a chunk for a list of compounds.
#[allow(clippy::too_many_arguments)] // TODO refactor
fn process_chunk_nbt_list<F>(
    chunk_data: &mca::RawChunk,
    cy: usize,
    cx: usize,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
    nbt_list_name: &str,
    process_nbt_compound_fn: F,
) where
    F: Fn(simdnbt::borrow::NbtCompound, &ScanTask, &[ItemFilter], &CliArgs, &mut Counter),
{
    let region_file_path = &task.path;
    let decompressed_data = match chunk_data.decompress() {
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
    let mut cursor = Cursor::new(decompressed_data.as_slice());
    let nbt_root = match simdnbt::borrow::read(&mut cursor) {
        Ok(simdnbt::borrow::Nbt::Some(nbt)) => nbt,
        Ok(simdnbt::borrow::Nbt::None) => {
            if cli_args.verbose {
                eprintln!(
                    "No NBT data found in chunk ({cx}, {cy}) in {}",
                    region_file_path.display()
                );
            }
            return;
        }
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

    let Some(compounds_list) = nbt_root.list(nbt_list_name).and_then(|l| l.compounds()) else {
        // If the list is not found or is not a list of compounds, this is normal (e.g., chunk with no relevant entities).
        return;
    };

    for nbt_compound in compounds_list {
        process_nbt_compound_fn(nbt_compound, task, item_queries, cli_args, counter);
    }
}

/// Processes a single chunk for block entities.
fn process_chunk_for_block_entities(
    chunk_data: &mca::RawChunk,
    cx: usize,
    cy: usize,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    process_chunk_nbt_list(
        chunk_data,
        cx,
        cy,
        task,
        item_queries,
        cli_args,
        counter,
        "block_entities", // NBT key for block entities in a chunk
        process_block_entity,
    );
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
    process_chunk_nbt_list(
        chunk_data,
        cx,
        cy,
        task,
        item_queries,
        cli_args,
        counter,
        "Entities", // NBT key for entities in a chunk
        process_single_entity,
    );
}

/// Processes a player data file (.dat or level.dat for the player section).
fn process_player_file(
    task: &ScanTask,
    queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
    user_cache: &HashMap<String, String>,
) {
    let file_path = &task.path;
    let file_data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            if cli_args.verbose {
                eprintln!("Failed to read player file {}: {e}", file_path.display());
            }
            return;
        }
    };

    let mut decompressor = GzDecoder::new(file_data.as_slice());
    let mut decompressed_data = Vec::new();
    if let Err(e) = decompressor.read_to_end(&mut decompressed_data) {
        if cli_args.verbose {
            eprintln!(
                "Failed to decompress player file {}: {e}",
                file_path.display(),
            );
        }
        return;
    }

    let mut cursor = Cursor::new(decompressed_data.as_slice());
    let nbt_root_container = match simdnbt::borrow::read(&mut cursor) {
        Ok(nbt) => nbt,
        Err(e) => {
            if cli_args.verbose {
                eprintln!(
                    "Failed to read NBT for player file {}: {e}",
                    file_path.display(),
                );
            }
            return;
        }
    };

    let nbt_root = match nbt_root_container {
        simdnbt::borrow::Nbt::Some(nbt) => nbt,
        simdnbt::borrow::Nbt::None => {
            if cli_args.verbose {
                eprintln!("No NBT data found in player file {}", file_path.display());
            }
            return;
        }
    };

    let (player_nbt_compound_opt, source_id, base_location_str): (
        Option<simdnbt::borrow::NbtCompound>,
        String,
        String,
    ) = if file_path
        .file_name()
        .is_some_and(|name| name == "level.dat")
    {
        // Handle level.dat for single-player
        nbt_root
            .compound(nbt_utils::NBT_KEY_PLAYER_DATA)
            .and_then(|data_compound| data_compound.compound(nbt_utils::NBT_KEY_PLAYER))
            .map_or_else(
                || {
                    if cli_args.verbose {
                        eprintln!(
                            "Player data not found in level.dat: {}",
                            file_path.display()
                        );
                    }
                    (None, "".to_string(), "".to_string())
                },
                |player_data| {
                    (
                        Some(player_data),
                        "Player (level.dat)".to_string(),
                        "level.dat".to_string(),
                    )
                },
            )
    } else {
        // Handle individual <uuid>.dat files
        let player_uuid = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("UnknownPlayer")
            .to_string();

        // Attempt to get player name from user_cache
        // player_uuid from file_stem is usually without hyphens.
        // user_cache keys are stored as hyphenated lowercase UUIDs.
        let display_name = uuid::Uuid::parse_str(&player_uuid)
            .ok()
            .and_then(|u| user_cache.get(&u.to_string())) // Uuid::to_string() is hyphenated lowercase
            .map_or_else(
                || player_uuid.clone(),
                |name| format!("{name} ({player_uuid})"),
            );

        (
            Some(nbt_root.as_compound()),
            display_name,
            file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
    };

    if let Some(player_nbt) = player_nbt_compound_opt {
        let location_str = get_entity_pos_string(&player_nbt).unwrap_or(base_location_str); // Player NBT also has "Pos"

        process_player_nbt_compound(
            player_nbt,
            task,
            queries,
            cli_args,
            counter,
            &source_id,
            &location_str,
        );
    }
}

/// Extracts the single-player's UUID string from the level.dat file, if present.
pub fn extract_single_player_uuid_from_level_dat(
    level_dat_path: &Path,
    cli_args: &CliArgs,
) -> Option<String> {
    if let Ok(file_data) = std::fs::read(level_dat_path) {
        let mut decompressor = GzDecoder::new(file_data.as_slice());
        let mut decompressed_data = Vec::new();
        if decompressor.read_to_end(&mut decompressed_data).is_ok() {
            let mut cursor = Cursor::new(decompressed_data.as_slice());
            // We need to ensure nbt_root lives as long as player_compound if we don't copy
            // Since get_uuid_from_nbt takes a reference, this is fine.
            if let Ok(simdnbt::borrow::Nbt::Some(nbt_root)) = simdnbt::borrow::read(&mut cursor) {
                if let Some(player_compound) = nbt_root
                    .compound(nbt_utils::NBT_KEY_PLAYER_DATA)
                    .and_then(|data_compound| data_compound.compound(nbt_utils::NBT_KEY_PLAYER))
                {
                    return nbt_utils::get_uuid_from_nbt(&player_compound);
                } else if cli_args.verbose {
                    eprintln!(
                        "Player NBT compound (Data.Player) not found in {}.",
                        level_dat_path.display()
                    );
                }
            }
        }
    }
    None
}

/// Processes the NBT compound for a single player's data.
fn process_player_nbt_compound(
    player_nbt: simdnbt::borrow::NbtCompound,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
    source_id: &str,
    location_str: &str,
) {
    let mut summary_nodes = Vec::new();

    if let Some(item_list) = player_nbt
        .list(nbt_utils::NBT_KEY_INVENTORY)
        .and_then(|l| l.compounds())
    {
        for item_compound in item_list {
            collect_summary_node(
                &item_compound,
                cli_args,
                item_queries,
                &mut summary_nodes,
                counter,
            );
        }
    }

    if let Some(item_list) = player_nbt
        .list(nbt_utils::NBT_KEY_ENDER_ITEMS)
        .and_then(|l| l.compounds())
    {
        for item_compound in item_list {
            collect_summary_node(
                &item_compound,
                cli_args,
                item_queries,
                &mut summary_nodes,
                counter,
            );
        }
    }

    if let Some(holder_compound) = player_nbt.compound(nbt_utils::NBT_KEY_EQUIPMENT) {
        for (_key_in_holder, value_nbt) in holder_compound.iter() {
            if let Some(actual_item_compound) = value_nbt.compound() {
                collect_summary_node(
                    &actual_item_compound,
                    cli_args,
                    item_queries,
                    &mut summary_nodes,
                    counter,
                );
            }
        }
    }

    print_per_source_summary_if_enabled(
        cli_args,
        &task.scope.dimension,
        source_id,
        location_str,
        summary_nodes,
    );
}

/// Prints a per-source summary tree if the corresponding CLI flag is enabled.
fn print_per_source_summary_if_enabled(
    cli_args: &CliArgs,
    dimension: &str,
    source_id: &str,
    source_location: &str,
    summary_nodes: Vec<ItemSummaryNode>, // Consumes the nodes
) {
    if cli_args.per_source_summary && !summary_nodes.is_empty() {
        let root_label = format!("[{dimension}] {source_id} @ {source_location}");
        let mut root = ItemSummaryNode::new_root(root_label, summary_nodes);
        root.collapse_leaves_recursive();
        if let Err(e) = print_tree(&root) {
            // Handle error from print_tree, e.g., by logging to stderr
            eprintln!("Error printing tree summary for {source_id}: {e}");
        }
    }
}

/// Processes a single entity's NBT data.
fn process_single_entity(
    entity_nbt: simdnbt::borrow::NbtCompound,
    task: &ScanTask,
    queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let Some(id_str) = entity_nbt.string(nbt_utils::NBT_KEY_ID) else {
        return;
    };
    let id = id_str.to_string();
    let pos_str =
        get_entity_pos_string(&entity_nbt).unwrap_or_else(|| "Unknown Position".to_string());

    let mut summary_nodes = Vec::new();
    for list_field_name in &[nbt_utils::NBT_KEY_ITEMS, nbt_utils::NBT_KEY_INVENTORY] {
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

    if let Some(item_compound) = entity_nbt.compound(nbt_utils::NBT_KEY_ITEM) {
        collect_summary_node(
            &item_compound,
            cli_args,
            queries,
            &mut summary_nodes,
            counter,
        );
    }

    if let Some(holder_compound) = entity_nbt.compound(nbt_utils::NBT_KEY_EQUIPMENT) {
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

    if let Some(passengers_list) = entity_nbt
        .list(nbt_utils::NBT_KEY_PASSENGERS)
        .and_then(|l| l.compounds())
    {
        for passenger_nbt in passengers_list {
            // Recursively process each passenger.
            // The passenger's items will be added to the current entity's summary_nodes
            // and the global_counter. This is generally fine as the per-source summary
            // is for the top-level entity being processed from the chunk.
            process_single_entity(passenger_nbt, task, queries, cli_args, counter);
        }
    }

    print_per_source_summary_if_enabled(
        cli_args,
        &task.scope.dimension,
        &id,
        &pos_str,
        summary_nodes,
    );
}

fn process_block_entity(
    block_entity: simdnbt::borrow::NbtCompound,
    task: &ScanTask,
    item_queries: &[ItemFilter],
    cli_args: &CliArgs,
    counter: &mut Counter,
) {
    let id = block_entity
        .string(nbt_utils::NBT_KEY_ID)
        .unwrap()
        .to_string();
    let x = block_entity.int("x").unwrap();
    let y = block_entity.int("y").unwrap();
    let z = block_entity.int("z").unwrap();

    let mut summary_nodes = Vec::new();
    if let Some(items) = block_entity
        .list(nbt_utils::NBT_KEY_ITEMS)
        .and_then(|l| l.compounds())
    {
        for item in items {
            collect_summary_node(&item, cli_args, item_queries, &mut summary_nodes, counter);
        }
    }

    for single_item_field in &["item", "RecordItem", "Book"] {
        if let Some(item) = block_entity.compound(single_item_field) {
            collect_summary_node(&item, cli_args, item_queries, &mut summary_nodes, counter);
        }
    }

    let location_str = format!("{x} {y} {z}");
    print_per_source_summary_if_enabled(
        cli_args,
        &task.scope.dimension,
        &id,
        &location_str,
        summary_nodes,
    );
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
    let id = item_nbt.string(nbt_utils::NBT_KEY_ID).unwrap().to_string();
    let count = item_nbt.int(nbt_utils::NBT_KEY_COUNT).unwrap_or(1) as u64;

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

    if let Some(components) = item_nbt.compound(nbt_utils::NBT_KEY_COMPONENTS) {
        if let Some(nested_list) = components
            .list(nbt_utils::NBT_KEY_MINECRAFT_CONTAINER)
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
            .list(nbt_utils::NBT_KEY_MINECRAFT_BUNDLE_CONTENTS)
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
            .compound(nbt_utils::NBT_KEY_COMPONENTS)
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

#[cfg(test)]
mod tests {
    use super::nbt_is_subset;
    use valence_nbt::Value;
    use valence_nbt::snbt::from_snbt_str;

    fn parse(s: &str) -> Value {
        from_snbt_str(s).expect("Failed to parse SNBT")
    }

    #[test]
    fn simple_compound_subset() {
        let sup = parse("{a:1, b:2, c:3}");
        let sub = parse("{a:1, c:3}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn compound_missing_key_should_fail() {
        let sup = parse("{a:1, b:2}");
        let sub = parse("{a:1, c:3}");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn unordered_list_subset() {
        let sup = parse("[1, 2, 3, 4]");
        let sub = parse("[4, 2]");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn list_insufficient_elements_should_fail() {
        let sup = parse("[1, 2, 2]");
        let sub = parse("[2, 2, 2]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn nested_structures_subset() {
        let sup = parse("{x:{y:[{z:1}, {z:2}]}, w:5}");
        let sub = parse("{x:{y:[{z:2}]}}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn primitive_equality_match_and_mismatch() {
        let sup = parse("123");
        let sub = parse("123");
        assert!(nbt_is_subset(&sup, &sub));

        let sup2 = parse("123");
        let sub2 = parse("456");
        assert!(!nbt_is_subset(&sup2, &sub2));
    }

    #[test]
    fn mismatched_types_should_fail() {
        let sup = parse("{a:1}");
        let sub = parse("[1]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn empty_list_subset() {
        let sup = parse("[1,2,3]");
        let sub = parse("[]");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn non_empty_list_on_empty_should_fail() {
        let sup = parse("[]");
        let sub = parse("[1]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn empty_compound_subset() {
        let sup = parse("{a:1}");
        let sub = parse("{}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn byte_array_exact_match() {
        let sup = parse("[I;1,2,3]");
        let sub = parse("[I;1,2,3]");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn byte_array_partial_should_fail() {
        let sup = parse("[I;1,2,3]");
        let sub = parse("[I;2,3]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn byte_array_missing_element_should_fail() {
        let sup = parse("[I;1,2]");
        let sub = parse("[I;1,2,3]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn mixed_list_types_should_fail_to_parse() {
        let res = valence_nbt::snbt::from_snbt_str("[1, \"a\"]");
        assert!(res.is_err(), "Mixed-type list unexpectedly parsed");
    }

    #[test]
    fn int_array_vs_byte_array_should_fail() {
        let sup = parse("[I;1,2,3]");
        let sub = parse("[B;1b,2b,3b]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn nested_empty_compound() {
        let sup = parse("{a:{b:{}}}");
        let sub = parse("{a:{b:{}}}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn deeply_nested_empty_list() {
        let sup = parse("{a:{b:[[],[1]]}}");
        let sub = parse("{a:{b:[[]]}}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn numeric_type_coercion_should_fail() {
        let res = valence_nbt::snbt::from_snbt_str("[1b, 2, 3s]");
        assert!(
            res.is_err(),
            "Parser unexpectedly accepted mixed numeric types"
        );
    }

    #[test]
    fn long_array_partial_should_fail() {
        let sup = parse("[L;9223372036854775807l,0l]");
        let sub = parse("[L;0l]");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn empty_string_vs_non_empty_should_fail() {
        let sup = parse("{text:\"\"}");
        let sub = parse("{text:\"something\"}");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn float_vs_double_zero_should_fail() {
        let sup = parse("{val:0.0f}");
        let sub = parse("{val:0.0d}");
        assert!(!nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn compound_with_empty_list_and_nested_empty_compound() {
        let sup = parse("{data:{items:[], meta:{}}}");
        let sub = parse("{data:{items:[]}}");
        assert!(nbt_is_subset(&sup, &sub));
    }

    #[test]
    fn unicode_string_match() {
        let sup = parse("{msg:\"こんにちは\"}");
        let sub = parse("{msg:\"こんにちは\"}");
        assert!(nbt_is_subset(&sup, &sub));
    }
}
