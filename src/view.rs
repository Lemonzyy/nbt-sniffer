use std::collections::{BTreeMap, HashMap};

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use serde_json::{Value as JsonValue, json};

use crate::{
    DataType,
    cli::{CliArgs, OutputFormat},
    counter::{Counter, CounterMap},
    escape_nbt_string,
};

/// A helper trait to check if a summary data structure is empty.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for Counter {
    fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

impl IsEmpty for CounterMap {
    fn is_empty(&self) -> bool {
        self.iter().all(|(_, counter)| counter.is_empty())
    }
}

impl<K, V> IsEmpty for HashMap<K, V> {
    fn is_empty(&self) -> bool {
        HashMap::is_empty(self)
    }
}
struct AggregatedData {
    grouped: BTreeMap<String, BTreeMap<DataType, Counter>>,
    total_block_entity: Counter,
    total_entity: Counter,
    total_player_data: Counter,
    total_combined: Counter,
}

impl AggregatedData {
    fn new(counter_map: &CounterMap) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<DataType, Counter>> = BTreeMap::new();
        let mut total_block_entity = Counter::new();
        let mut total_entity = Counter::new();
        let mut total_player_data = Counter::new();
        let mut total_combined = Counter::new();

        for (scope, counter) in counter_map.iter() {
            grouped
                .entry(scope.dimension.clone())
                .or_default()
                .entry(scope.data_type.clone())
                .or_default()
                .merge(counter);

            match scope.data_type {
                DataType::BlockEntity => total_block_entity.merge(counter),
                DataType::Entity => total_entity.merge(counter),
                DataType::Player => total_player_data.merge(counter),
            }
            total_combined.merge(counter);
        }

        Self {
            grouped,
            total_block_entity,
            total_entity,
            total_player_data,
            total_combined,
        }
    }

    fn dimension_combined(&self, dimension: &str) -> Counter {
        let mut combined = Counter::new();
        if let Some(types_map) = self.grouped.get(dimension) {
            for counter in types_map.values() {
                combined.merge(counter);
            }
        }
        combined
    }
}

/// Trait to provide summary data in a generic way for different views.
trait SummaryDataProvider {
    type ItemSummary: Clone + IsEmpty; // e.g., Counter or HashMap<String, u64>

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>>;
    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary;
    fn get_total_entity_summary(&self) -> &Self::ItemSummary;
    fn get_total_player_data_summary(&self) -> &Self::ItemSummary;
    fn get_total_combined_summary(&self) -> &Self::ItemSummary;
    fn calculate_dimension_combined_summary(&self, dimension: &str) -> Self::ItemSummary;
}

impl SummaryDataProvider for AggregatedData {
    type ItemSummary = Counter;

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>> {
        &self.grouped
    }
    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary {
        &self.total_block_entity
    }
    fn get_total_entity_summary(&self) -> &Self::ItemSummary {
        &self.total_entity
    }
    fn get_total_player_data_summary(&self) -> &Self::ItemSummary {
        &self.total_player_data
    }
    fn get_total_combined_summary(&self) -> &Self::ItemSummary {
        &self.total_combined
    }
    fn calculate_dimension_combined_summary(&self, dimension: &str) -> Self::ItemSummary {
        self.dimension_combined(dimension)
    }
}

/// Holds aggregated counts summarized by item ID for various scopes.
struct AggregatedIdCountsData {
    grouped: BTreeMap<String, BTreeMap<DataType, HashMap<String, u64>>>,
    total_block_entity: HashMap<String, u64>,
    total_entity: HashMap<String, u64>,
    total_player_data: HashMap<String, u64>,
    total_combined: HashMap<String, u64>,
}

impl AggregatedIdCountsData {
    fn new(counter_map: &CounterMap) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<DataType, HashMap<String, u64>>> =
            BTreeMap::new();
        let mut total_block_entity = HashMap::new();
        let mut total_entity = HashMap::new();
        let mut total_player_data = HashMap::new();
        let mut total_combined = HashMap::new();

        for (scope, counter) in counter_map.iter() {
            let current_total_by_id = counter.total_by_id();
            let dim_data_map = grouped
                .entry(scope.dimension.clone())
                .or_default()
                .entry(scope.data_type.clone())
                .or_default();

            for (id, count) in &current_total_by_id {
                *dim_data_map.entry(id.clone()).or_default() += *count;
                *total_combined.entry(id.clone()).or_default() += *count;
                match scope.data_type {
                    DataType::BlockEntity => {
                        *total_block_entity.entry(id.clone()).or_default() += *count;
                    }
                    DataType::Entity => {
                        *total_entity.entry(id.clone()).or_default() += *count;
                    }
                    DataType::Player => {
                        *total_player_data.entry(id.clone()).or_default() += *count;
                    }
                }
            }
        }
        Self {
            grouped,
            total_block_entity,
            total_entity,
            total_player_data,
            total_combined,
        }
    }

    fn dimension_combined(&self, dimension: &str) -> HashMap<String, u64> {
        let mut combined = HashMap::new();
        if let Some(types_map) = self.grouped.get(dimension) {
            for id_map in types_map.values() {
                for (id, count) in id_map {
                    *combined.entry(id.clone()).or_default() += *count;
                }
            }
        }
        combined
    }
}

impl SummaryDataProvider for AggregatedIdCountsData {
    type ItemSummary = HashMap<String, u64>;

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>> {
        &self.grouped
    }
    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary {
        &self.total_block_entity
    }
    fn get_total_entity_summary(&self) -> &Self::ItemSummary {
        &self.total_entity
    }
    fn get_total_player_data_summary(&self) -> &Self::ItemSummary {
        &self.total_player_data
    }
    fn get_total_combined_summary(&self) -> &Self::ItemSummary {
        &self.total_combined
    }
    fn calculate_dimension_combined_summary(&self, dimension: &str) -> Self::ItemSummary {
        self.dimension_combined(dimension)
    }
}

/// Generic function to print summaries based on the provided data provider and CLI arguments.
fn execute_summary_printing<P, F>(provider: &P, args: &CliArgs, mut print_fn_for_summary: F)
where
    P: SummaryDataProvider,
    F: FnMut(&P::ItemSummary, &str),
{
    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {}
        (true, false) => {
            for dimension in provider.get_grouped_data().keys() {
                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                if !combined_dim_summary.is_empty() {
                    print_fn_for_summary(&combined_dim_summary, &format!("Dimension: {dimension}"));
                }
            }
        }
        (false, true) => {
            let be_summary = provider.get_total_block_entity_summary();
            if !be_summary.is_empty() {
                print_fn_for_summary(be_summary, "Block Entity");
            }
            let e_summary = provider.get_total_entity_summary();
            if !e_summary.is_empty() {
                print_fn_for_summary(e_summary, "Entity");
            }
            let p_summary = provider.get_total_player_data_summary();
            if !p_summary.is_empty() {
                print_fn_for_summary(p_summary, "Player Data");
            }
        }
        (true, true) => {
            for (dimension, types_map) in provider.get_grouped_data() {
                println!("\nDimension: {dimension}");

                if let Some(summary_item) = types_map.get(&DataType::BlockEntity)
                    && !summary_item.is_empty()
                {
                    print_fn_for_summary(summary_item, "Block Entity");
                }

                if let Some(summary_item) = types_map.get(&DataType::Entity)
                    && !summary_item.is_empty()
                {
                    print_fn_for_summary(summary_item, "Entity");
                }

                if let Some(summary_item) = types_map.get(&DataType::Player)
                    && !summary_item.is_empty()
                {
                    print_fn_for_summary(summary_item, "Player Data");
                }

                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                if !combined_dim_summary.is_empty() {
                    print_fn_for_summary(&combined_dim_summary, "Summary");
                }
            }

            let be_summary_total = provider.get_total_block_entity_summary();
            if !be_summary_total.is_empty() {
                print_fn_for_summary(be_summary_total, "\nBlock Entity");
            }
            let e_summary_total = provider.get_total_entity_summary();
            if !e_summary_total.is_empty() {
                print_fn_for_summary(e_summary_total, "Entity");
            }
            let p_summary_total = provider.get_total_player_data_summary();
            if !p_summary_total.is_empty() {
                print_fn_for_summary(p_summary_total, "Player Data");
            }
        }
    }
    let total_summary = provider.get_total_combined_summary();
    if !total_summary.is_empty() {
        print_fn_for_summary(total_summary, "Total");
    }
}

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    if args.output_format.is_json() {
        let grand_total_numeric_count = counter_map.combined().total();
        let data_provider = AggregatedData::new(counter_map);
        let json_output = generate_json_summary(
            &data_provider,
            args,
            get_detailed_counter_json,
            grand_total_numeric_count,
        );
        print_json_output(&json_output, args.output_format == OutputFormat::PrettyJson);
    } else {
        let data_provider = AggregatedData::new(counter_map);
        execute_summary_printing(&data_provider, args, |counter_summary, label| {
            if !label.starts_with('\n') {
                println!("{label}:");
            } else {
                println!("{}:", &label[1..]);
            }
            print_detailed_counter(counter_summary);
        });
    }
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedData::new(counter_map);
    if args.output_format.is_json() {
        let grand_total_numeric_count = counter_map.combined().total();
        let json_output = generate_json_summary(
            &data_provider,
            args,
            get_nbt_counter_json,
            grand_total_numeric_count,
        );
        print_json_output(&json_output, args.output_format == OutputFormat::PrettyJson);
    } else {
        execute_summary_printing(&data_provider, args, |counter_summary, label| {
            let display_label = match label {
                "Block Entity" => "Total Block Entity",
                "Entity" => "Total Entity",
                "Player Data" => "Total Player Data",
                _ => label,
            };

            if !label.starts_with('\n')
                && !label.starts_with("Total")
                && !label.starts_with("Summary")
            {
                println!("{display_label}:");
            } else {
                println!("{}:", &display_label.trim_start_matches('\n'));
            }
            print_nbt_counter(counter_summary);
        });
    }
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedIdCountsData::new(counter_map);
    if args.output_format.is_json() {
        let grand_total_numeric_count = counter_map.combined().total();
        let json_output = generate_json_summary(
            &data_provider,
            args,
            get_id_map_json,
            grand_total_numeric_count,
        );
        print_json_output(&json_output, args.output_format == OutputFormat::PrettyJson);
    } else {
        execute_summary_printing(&data_provider, args, |id_map_summary, label| {
            let mut effective_label = label.to_string();
            if label.starts_with('\n') {
                effective_label = label.trim_start_matches('\n').to_string();
            }
            println!("{effective_label}:");
            print_id_map(id_map_summary);
        });
    }
}

fn print_json_output(json_value: &JsonValue, pretty: bool) {
    let result = if pretty {
        serde_json::to_string_pretty(json_value)
    } else {
        serde_json::to_string(json_value)
    };

    match result {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("Error serializing to JSON: {e}");
        }
    }
}

fn print_detailed_counter(counter: &Counter) {
    let mut detailed_vec: Vec<_> = counter
        .detailed_counts()
        .iter()
        .map(|(item_key, &count)| (item_key.id.clone(), item_key.components_snbt.clone(), count))
        .collect();

    detailed_vec.sort_by(|(a_id, a_nbt, a_count), (b_id, b_nbt, b_count)| {
        b_count
            .cmp(a_count)
            .then_with(|| a_id.cmp(b_id))
            .then_with(|| a_nbt.cmp(b_nbt))
    });

    print_table(
        &["Count", "ID", "NBT"],
        detailed_vec,
        |(id, nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt);
            vec![Cell::new(count), Cell::new(id), Cell::new(nbt_str)]
        },
        Some(2),
    );
}

fn print_id_map(map: &HashMap<String, u64>) {
    let mut vec: Vec<_> = map.iter().map(|(id, &count)| (id.clone(), count)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    print_table(
        &["Count", "Item ID"],
        vec,
        |(id, count)| vec![Cell::new(count), Cell::new(id)],
        None,
    );
}

fn print_nbt_counter(counter: &Counter) {
    let mut by_nbt_vec: Vec<_> = counter.total_by_nbt().into_iter().collect();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    print_table(
        &["Count", "NBT"],
        by_nbt_vec,
        |(nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt);
            vec![Cell::new(count), Cell::new(nbt_str)]
        },
        Some(1),
    );
}

fn print_table<T, F>(
    headers: &[&str],
    data: Vec<T>,
    mut formatter: F,
    left_align_col: Option<usize>,
) where
    F: FnMut(&T) -> Vec<Cell>,
{
    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|&h| Cell::new(h).set_alignment(CellAlignment::Center)),
        );

    if let Some(col_idx) = left_align_col
        && let Some(col) = table.column_mut(col_idx)
    {
        col.set_cell_alignment(CellAlignment::Left);
    }

    for item in data {
        let cells = formatter(&item);
        table.add_row(cells);
    }
    println!("{table}");
}

fn format_nbt_string(nbt_opt: &Option<String>) -> String {
    nbt_opt
        .as_deref()
        .map(escape_nbt_string)
        .unwrap_or_else(|| "No NBT".into())
}

// For data from view_detailed
fn get_detailed_counter_json(counter: &Counter) -> JsonValue {
    let mut detailed_vec: Vec<_> = counter
        .detailed_counts()
        .iter()
        .map(|(item_key, &count)| (item_key.id.clone(), item_key.components_snbt.clone(), count))
        .collect();

    detailed_vec.sort_by(|(a_id, a_nbt, a_count), (b_id, b_nbt, b_count)| {
        b_count
            .cmp(a_count)
            .then_with(|| a_id.cmp(b_id))
            .then_with(|| a_nbt.cmp(b_nbt))
    });

    let json_array: Vec<JsonValue> = detailed_vec
        .iter()
        .map(|(id, nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt); // Reuse existing helper
            json!({
                "count": count,
                "id": id,
                "nbt": nbt_str
            })
        })
        .collect();
    JsonValue::Array(json_array)
}

// For data from view_by_id
fn get_id_map_json(map: &HashMap<String, u64>) -> JsonValue {
    let mut vec: Vec<_> = map.iter().map(|(id, &count)| (id.clone(), count)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let json_array: Vec<JsonValue> = vec
        .iter()
        .map(|(id, count)| {
            json!({
                "count": count,
                "id": id
            })
        })
        .collect();
    JsonValue::Array(json_array)
}

// For data from view_by_nbt
fn get_nbt_counter_json(counter: &Counter) -> JsonValue {
    let mut by_nbt_vec: Vec<_> = counter.total_by_nbt().into_iter().collect();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    let json_array: Vec<JsonValue> = by_nbt_vec
        .iter()
        .map(|(nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt); // Reuse existing helper
            json!({
                "count": count,
                "nbt": nbt_str
            })
        })
        .collect();
    JsonValue::Array(json_array)
}

fn generate_json_summary<P, F>(
    provider: &P,
    args: &CliArgs,
    item_summary_to_json: F,
    grand_total_numeric_count: u64,
) -> JsonValue
where
    P: SummaryDataProvider,
    F: Fn(&P::ItemSummary) -> JsonValue,
{
    let mut root_map = serde_json::Map::new();

    // Helper to conditionally insert JSON data if the summary item is not empty
    let insert_json_if_not_empty =
        |map: &mut serde_json::Map<String, JsonValue>,
         key: String,
         summary_item: &P::ItemSummary| {
            if !summary_item.is_empty() {
                map.insert(key, item_summary_to_json(summary_item));
            }
        };

    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {
            // Only grand total will be added later
        }
        (true, false) => {
            // Per dimension summary only
            let mut dim_summaries = serde_json::Map::new();
            for dimension in provider.get_grouped_data().keys() {
                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                insert_json_if_not_empty(
                    &mut dim_summaries,
                    dimension.clone(),
                    &combined_dim_summary,
                );
            }
            if !dim_summaries.is_empty() {
                root_map.insert(
                    "per_dimension".to_string(),
                    JsonValue::Object(dim_summaries),
                );
            }
        }
        (false, true) => {
            // Per data type summary only
            let mut type_summaries = serde_json::Map::new();
            insert_json_if_not_empty(
                &mut type_summaries,
                "BlockEntity".to_string(),
                provider.get_total_block_entity_summary(),
            );
            insert_json_if_not_empty(
                &mut type_summaries,
                "Entity".to_string(),
                provider.get_total_entity_summary(),
            );
            insert_json_if_not_empty(
                &mut type_summaries,
                "PlayerData".to_string(),
                provider.get_total_player_data_summary(),
            );
            if !type_summaries.is_empty() {
                root_map.insert(
                    "per_data_type".to_string(),
                    JsonValue::Object(type_summaries),
                );
            }
        }
        (true, true) => {
            // 1. "per_dimension" (dimension -> its total summary)
            // This is the same data as when only --per-dimension-summary is used.
            let mut per_dimension_totals = serde_json::Map::new();
            for dimension in provider.get_grouped_data().keys() {
                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                insert_json_if_not_empty(
                    &mut per_dimension_totals,
                    dimension.clone(),
                    &combined_dim_summary,
                );
            }
            if !per_dimension_totals.is_empty() {
                root_map.insert(
                    "per_dimension".to_string(),
                    JsonValue::Object(per_dimension_totals),
                );
            }

            // 2. "per_data_type" (data_type -> its overall total summary)
            // This is the same data as when only --per-data-type-summary is used.
            let mut per_data_type_totals = serde_json::Map::new();
            insert_json_if_not_empty(
                &mut per_data_type_totals,
                "BlockEntity".to_string(),
                provider.get_total_block_entity_summary(),
            );
            insert_json_if_not_empty(
                &mut per_data_type_totals,
                "Entity".to_string(),
                provider.get_total_entity_summary(),
            );
            insert_json_if_not_empty(
                &mut per_data_type_totals,
                "PlayerData".to_string(),
                provider.get_total_player_data_summary(),
            );
            if !per_data_type_totals.is_empty() {
                root_map.insert(
                    "per_data_type".to_string(),
                    JsonValue::Object(per_data_type_totals),
                );
            }

            // 3. "per_dimension_detail" (dimension -> data_type -> summary for that specific combo)
            // This provides the most granular breakdown. It does *not* include the dimension's overall summary again,
            // as that is now consistently in the "per_dimension" field.
            let mut per_dimension_detail_breakdown = serde_json::Map::new();
            for (dimension, types_map) in provider.get_grouped_data() {
                let mut current_dim_data_breakdown = serde_json::Map::new();
                if let Some(summary_item) = types_map.get(&DataType::BlockEntity) {
                    insert_json_if_not_empty(
                        &mut current_dim_data_breakdown,
                        "BlockEntity".to_string(),
                        summary_item,
                    );
                }
                if let Some(summary_item) = types_map.get(&DataType::Entity) {
                    insert_json_if_not_empty(
                        &mut current_dim_data_breakdown,
                        "Entity".to_string(),
                        summary_item,
                    );
                }
                if let Some(summary_item) = types_map.get(&DataType::Player) {
                    insert_json_if_not_empty(
                        &mut current_dim_data_breakdown,
                        "PlayerData".to_string(),
                        summary_item,
                    );
                }

                // The dimension's combined summary is already part of the "per_dimension" map.
                // No need to add a "Summary" field within this breakdown.

                if !current_dim_data_breakdown.is_empty() {
                    per_dimension_detail_breakdown.insert(
                        dimension.clone(),
                        JsonValue::Object(current_dim_data_breakdown),
                    );
                }
            }
            if !per_dimension_detail_breakdown.is_empty() {
                root_map.insert(
                    "per_dimension_detail".to_string(),
                    JsonValue::Object(per_dimension_detail_breakdown),
                );
            }
        }
    }

    // Always add grand total
    let total_summary_items = provider.get_total_combined_summary();
    if !total_summary_items.is_empty() {
        // Ensure grand_total is not empty before adding
        root_map.insert(
            "grand_total".to_string(),
            item_summary_to_json(total_summary_items),
        );
    }
    // Always add the numeric grand total count
    root_map.insert(
        "grand_total_count".to_string(),
        json!(grand_total_numeric_count),
    );

    JsonValue::Object(root_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DataType, Scope,
        cli::{CliArgs, OutputFormat, ViewMode},
        counter::{Counter, CounterMap},
    };
    use std::path::PathBuf;
    use valence_nbt::Value;

    fn nbt_val(s: &str) -> Value {
        valence_nbt::snbt::from_snbt_str(s).expect("Failed to parse SNBT for test")
    }

    fn create_sample_counter_map() -> CounterMap {
        let mut map = CounterMap::new();

        let scope_ow_be = Scope {
            dimension: "overworld".to_string(),
            data_type: DataType::BlockEntity,
        };
        let mut counter_ow_be = Counter::new();
        counter_ow_be.add("minecraft:chest".to_string(), None, 10);
        counter_ow_be.add("minecraft:furnace".to_string(), None, 5);
        map.merge_scope(scope_ow_be, &counter_ow_be);

        let scope_ow_e = Scope {
            dimension: "overworld".to_string(),
            data_type: DataType::Entity,
        };
        let mut counter_ow_e = Counter::new();
        let nbt_damaged_sword = nbt_val("{components:{\"minecraft:damage\":50}}");
        counter_ow_e.add(
            "minecraft:iron_sword".to_string(),
            Some(&nbt_damaged_sword),
            5,
        );
        counter_ow_e.add("minecraft:rotten_flesh".to_string(), None, 15);
        map.merge_scope(scope_ow_e, &counter_ow_e);

        let scope_nether_be = Scope {
            dimension: "nether".to_string(),
            data_type: DataType::BlockEntity,
        };
        let mut counter_nether_be = Counter::new();
        counter_nether_be.add("minecraft:chest".to_string(), None, 3);
        map.merge_scope(scope_nether_be, &counter_nether_be);

        let scope_player = Scope {
            dimension: "playerdata".to_string(),
            data_type: DataType::Player,
        };
        let mut counter_player = Counter::new();
        counter_player.add("minecraft:diamond_sword".to_string(), None, 1);
        // For testing total_by_id, let's use a simple count for an item that might have NBT
        // The actual NBT string for components would be more complex.
        let nbt_ender_pearls_comp =
            nbt_val("{components:{\"minecraft:custom_data\":{stack_size:16}}}");
        counter_player.add(
            "minecraft:ender_pearl".to_string(),
            Some(&nbt_ender_pearls_comp),
            1,
        ); // Represents 1 item stack with NBT indicating 16
        map.merge_scope(scope_player, &counter_player);

        map
    }

    #[test]
    fn test_aggregated_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_data = AggregatedData::new(&counter_map);

        assert_eq!(agg_data.grouped.len(), 3); // overworld, nether, playerdata
        assert_eq!(agg_data.grouped.get("overworld").unwrap().len(), 2);
        assert_eq!(agg_data.grouped.get("nether").unwrap().len(), 1);
        assert_eq!(agg_data.grouped.get("playerdata").unwrap().len(), 1);

        assert_eq!(
            agg_data
                .grouped
                .get("overworld")
                .unwrap()
                .get(&DataType::BlockEntity)
                .unwrap()
                .total(),
            15
        );
        assert_eq!(
            agg_data
                .grouped
                .get("overworld")
                .unwrap()
                .get(&DataType::Entity)
                .unwrap()
                .total(),
            20
        );
        assert_eq!(
            agg_data
                .grouped
                .get("nether")
                .unwrap()
                .get(&DataType::BlockEntity)
                .unwrap()
                .total(),
            3
        );
        assert_eq!(
            agg_data
                .grouped
                .get("playerdata")
                .unwrap()
                .get(&DataType::Player)
                .unwrap()
                .total(),
            2
        ); // 1 sword + 1 pearl stack item

        assert_eq!(agg_data.total_block_entity.total(), 15 + 3);
        assert_eq!(agg_data.total_entity.total(), 20);
        assert_eq!(agg_data.total_player_data.total(), 2);
        assert_eq!(agg_data.total_combined.total(), 15 + 3 + 20 + 2);

        assert_eq!(agg_data.dimension_combined("overworld").total(), 15 + 20);
        assert_eq!(agg_data.dimension_combined("nether").total(), 3);
        assert_eq!(agg_data.dimension_combined("playerdata").total(), 2);
    }

    #[test]
    fn test_aggregated_id_counts_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_id_data = AggregatedIdCountsData::new(&counter_map);

        let ov_be_counts = agg_id_data
            .grouped
            .get("overworld")
            .unwrap()
            .get(&DataType::BlockEntity)
            .unwrap();
        assert_eq!(ov_be_counts.get("minecraft:chest"), Some(&10));

        let player_counts = agg_id_data
            .grouped
            .get("playerdata")
            .unwrap()
            .get(&DataType::Player)
            .unwrap();
        assert_eq!(player_counts.get("minecraft:diamond_sword"), Some(&1));
        assert_eq!(player_counts.get("minecraft:ender_pearl"), Some(&1)); // Counts item stacks

        assert_eq!(
            agg_id_data.total_block_entity.get("minecraft:chest"),
            Some(&13)
        );
        assert_eq!(
            agg_id_data.total_entity.get("minecraft:iron_sword"),
            Some(&5)
        );
        assert_eq!(
            agg_id_data.total_player_data.get("minecraft:diamond_sword"),
            Some(&1)
        );
        assert_eq!(
            agg_id_data.total_player_data.get("minecraft:ender_pearl"),
            Some(&1)
        );

        assert_eq!(agg_id_data.total_combined.get("minecraft:chest"), Some(&13));
        assert_eq!(
            agg_id_data.total_combined.get("minecraft:diamond_sword"),
            Some(&1)
        );

        let player_dim_combined = agg_id_data.dimension_combined("playerdata");
        assert_eq!(player_dim_combined.get("minecraft:diamond_sword"), Some(&1));
    }

    fn mock_cli_args() -> CliArgs {
        CliArgs {
            world_path: PathBuf::from("dummy"),
            all: true,
            items: vec![],
            view: ViewMode::ById,
            show_nbt: false,
            per_source_summary: false,
            per_dimension_summary: false,
            per_data_type_summary: false,
            verbose: false,
            output_format: OutputFormat::Table
        }
    }

    #[test]
    fn test_execute_summary_printing_logic() {
        let counter_map = create_sample_counter_map();
        let data_provider = AggregatedIdCountsData::new(&counter_map);
        let mut printed_labels: Vec<String> = Vec::new();

        let mut args = mock_cli_args();
        {
            let mut print_fn_case1 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case1);
        }
        assert_eq!(printed_labels, vec!["Total"]);
        printed_labels.clear();

        args.per_dimension_summary = true;
        {
            let mut print_fn_case2 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case2);
        }
        // Order from BTreeMap: nether, overworld, playerdata
        assert_eq!(
            printed_labels,
            vec![
                "Dimension: nether",
                "Dimension: overworld",
                "Dimension: playerdata",
                "Total"
            ]
        );
        printed_labels.clear();
        args.per_dimension_summary = false;

        args.per_data_type_summary = true;
        {
            let mut print_fn_case3 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case3);
        }
        assert_eq!(
            printed_labels,
            vec!["Block Entity", "Entity", "Player Data", "Total"]
        );
        printed_labels.clear();

        args.per_dimension_summary = true;
        args.per_data_type_summary = true;
        {
            let mut print_fn_case4 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case4);
        }
        assert_eq!(
            printed_labels,
            vec![
                // Dimension: nether
                "Block Entity",
                "Summary",
                // Dimension: overworld
                "Block Entity",
                "Entity",
                "Summary",
                // Dimension: playerdata
                "Player Data",
                "Summary",
                // Totals
                "\nBlock Entity",
                "Entity",
                "Player Data",
                "Total"
            ]
        );
    }
}
