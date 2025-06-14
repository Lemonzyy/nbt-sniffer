use std::collections::{BTreeMap, HashMap};

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::{
    DataType,
    cli::{CliArgs, OutputFormat, ViewMode},
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

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedData::new(counter_map);
    let grand_total_numeric_count = data_provider.total_combined.total();
    let report_data = generate_report_data(
        &data_provider,
        args,
        to_detailed_item_entries,
        grand_total_numeric_count,
    );

    if args.output_format.is_json() {
        let json_value = serde_json::to_value(&report_data).unwrap_or_else(|e| {
            eprintln!("Error serializing report to JSON: {e}");
            json!({ "error": format!("Failed to serialize report: {e}") })
        });
        print_json_output(&json_value, args.output_format == OutputFormat::PrettyJson);
    } else {
        print_report_as_tables(&report_data, args, print_detailed_counter);
    }
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedData::new(counter_map);
    let grand_total_numeric_count = data_provider.total_combined.total();
    let report_data = generate_report_data(
        &data_provider,
        args,
        to_nbt_item_entries,
        grand_total_numeric_count,
    );

    if args.output_format.is_json() {
        let json_value = serde_json::to_value(&report_data).unwrap_or_else(|e| {
            eprintln!("Error serializing report to JSON: {e}");
            json!({ "error": format!("Failed to serialize report: {e}") })
        });
        print_json_output(&json_value, args.output_format == OutputFormat::PrettyJson);
    } else {
        print_report_as_tables(&report_data, args, print_nbt_counter);
    }
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedIdCountsData::new(counter_map);
    let grand_total_numeric_count = data_provider.total_combined.values().sum();
    let report_data = generate_report_data(
        &data_provider,
        args,
        to_id_item_entries,
        grand_total_numeric_count,
    );

    if args.output_format.is_json() {
        let json_value = serde_json::to_value(&report_data).unwrap_or_else(|e| {
            eprintln!("Error serializing report to JSON: {e}");
            json!({ "error": format!("Failed to serialize report: {e}") })
        });
        print_json_output(&json_value, args.output_format == OutputFormat::PrettyJson);
    } else {
        print_report_as_tables(&report_data, args, print_id_map);
    }
}

fn print_report_as_tables<TItem>(
    report: &Report<TItem>,
    args: &CliArgs,
    mut print_items_fn: impl FnMut(&[TItem]),
) where
    TItem: Clone + Serialize,
{
    let mut print_section = |items: &Vec<TItem>, label: &str| {
        if !items.is_empty() {
            let mut effective_label = label.to_string();
            if label.starts_with('\n') {
                effective_label = label.trim_start_matches('\n').to_string();
            }

            let display_label = if args.view == ViewMode::ByNbt {
                match effective_label.as_str() {
                    "Block Entity" => "Total Block Entity".to_string(),
                    "Entity" => "Total Entity".to_string(),
                    "Player Data" => "Total Player Data".to_string(),
                    _ => effective_label,
                }
            } else {
                effective_label
            };

            let is_by_nbt_special_label = args.view == ViewMode::ByNbt
                && (label == "Block Entity" || label == "Entity" || label == "Player Data");

            if label.starts_with("Dimension:")
                || is_by_nbt_special_label
                || (args.view != ViewMode::ByNbt
                    && !label.starts_with('\n')
                    && label != "Total"
                    && label != "Summary")
            {
                println!("{display_label}:");
            } else if label.starts_with('\n') || label == "Total" || label == "Summary" {
                println!("{}:", display_label.trim_start_matches('\n'));
            }
            // Other cases (like specific data type labels not covered above) might not print a label if not explicitly handled,
            // or rely on the dimension header if inside a (true, true) block.

            print_items_fn(items);
        }
    };

    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {}
        (true, false) => {
            if let Some(per_dimension_data) = &report.per_dimension {
                for (dimension_name, items) in per_dimension_data {
                    print_section(items, &format!("Dimension: {dimension_name}"));
                }
            }
        }
        (false, true) => {
            if let Some(per_data_type_data) = &report.per_data_type {
                if let Some(items) = per_data_type_data.get(&DataType::BlockEntity.to_string()) {
                    print_section(items, "Block Entity");
                }
                if let Some(items) = per_data_type_data.get(&DataType::Entity.to_string()) {
                    print_section(items, "Entity");
                }
                if let Some(items) = per_data_type_data.get(&DataType::Player.to_string()) {
                    print_section(items, "Player Data");
                }
            }
        }
        (true, true) => {
            if let Some(per_dimension_detail_data) = &report.per_dimension_detail {
                for (dimension_name, type_map) in per_dimension_detail_data {
                    println!("\nDimension: {dimension_name}");
                    if let Some(items) = type_map.get(&DataType::BlockEntity.to_string()) {
                        print_section(items, "Block Entity");
                    }
                    if let Some(items) = type_map.get(&DataType::Entity.to_string()) {
                        print_section(items, "Entity");
                    }
                    if let Some(items) = type_map.get(&DataType::Player.to_string()) {
                        print_section(items, "Player Data");
                    }
                    if let Some(per_dimension_data) = &report.per_dimension
                        && let Some(dim_summary_items) = per_dimension_data.get(dimension_name)
                    {
                        print_section(dim_summary_items, "Summary");
                    }
                }
            }
            if let Some(per_data_type_data) = &report.per_data_type {
                if let Some(items) = per_data_type_data.get(&DataType::BlockEntity.to_string()) {
                    print_section(items, "\nBlock Entity");
                }
                if let Some(items) = per_data_type_data.get(&DataType::Entity.to_string()) {
                    print_section(items, "Entity");
                }
                if let Some(items) = per_data_type_data.get(&DataType::Player.to_string()) {
                    print_section(items, "Player Data");
                }
            }
        }
    }
    print_section(&report.grand_total, "Total");
}

/// Helper function to serialize a JsonValue to a string (pretty or compact)
/// and print it to stdout, or print an error to stderr.
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

fn print_detailed_counter(items: &[ReportItemDetailed]) {
    if items.is_empty() {
        return;
    }
    print_table(
        &["Count", "ID", "NBT"],
        items.to_vec(),
        |item| {
            vec![
                Cell::new(item.count),
                Cell::new(&item.id),
                Cell::new(&item.nbt),
            ]
        },
        Some(2),
    );
}

fn print_id_map(items: &[ReportItemId]) {
    if items.is_empty() {
        return;
    }
    print_table(
        &["Count", "Item ID"],
        items.to_vec(),
        |item| vec![Cell::new(item.count), Cell::new(&item.id)],
        None,
    );
}

fn print_nbt_counter(items: &[ReportItemNbt]) {
    if items.is_empty() {
        return;
    }
    print_table(
        &["Count", "NBT"],
        items.to_vec(),
        |item| vec![Cell::new(item.count), Cell::new(&item.nbt)],
        Some(1),
    );
}

fn print_table<T, F>(
    headers: &[&str],
    data: Vec<T>,
    mut row_formatter: F,
    left_align_col: Option<usize>,
) where
    F: FnMut(&T) -> Vec<Cell>,
    T: Clone,
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
        let cells = row_formatter(&item);
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

#[derive(Serialize, Clone)]
struct ReportItemDetailed {
    count: u64,
    id: String,
    nbt: String,
}

#[derive(Serialize, Clone)]
struct ReportItemId {
    count: u64,
    id: String,
}

#[derive(Serialize, Clone)]
struct ReportItemNbt {
    count: u64,
    nbt: String,
}

#[derive(Serialize)]
struct Report<TItem: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    per_dimension: Option<HashMap<String, Vec<TItem>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    per_data_type: Option<HashMap<String, Vec<TItem>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    per_dimension_detail: Option<HashMap<String, HashMap<String, Vec<TItem>>>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    grand_total: Vec<TItem>,
    grand_total_count: u64,
}

fn to_detailed_item_entries(counter: &Counter) -> Vec<ReportItemDetailed> {
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

    detailed_vec
        .iter()
        .map(|(id, nbt_opt, count)| ReportItemDetailed {
            count: *count,
            id: id.clone(),
            nbt: format_nbt_string(nbt_opt),
        })
        .collect()
}

fn to_id_item_entries(map: &HashMap<String, u64>) -> Vec<ReportItemId> {
    let mut vec: Vec<_> = map.iter().map(|(id, &count)| (id.clone(), count)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    vec.iter()
        .map(|(id, count)| ReportItemId {
            count: *count,
            id: id.clone(),
        })
        .collect()
}

fn to_nbt_item_entries(counter: &Counter) -> Vec<ReportItemNbt> {
    let mut by_nbt_vec: Vec<_> = counter.total_by_nbt().into_iter().collect();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    by_nbt_vec
        .iter()
        .map(|(nbt_opt, count)| ReportItemNbt {
            count: *count,
            nbt: format_nbt_string(nbt_opt),
        })
        .collect()
}

fn build_per_dimension_summary_section<P, TItem, F>(
    provider: &P,
    to_item_entries: &F,
) -> Option<HashMap<String, Vec<TItem>>>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    let mut dim_summaries_map = HashMap::new();
    for dimension in provider.get_grouped_data().keys() {
        let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
        if !combined_dim_summary.is_empty() {
            dim_summaries_map.insert(dimension.clone(), to_item_entries(&combined_dim_summary));
        }
    }
    if dim_summaries_map.is_empty() {
        None
    } else {
        Some(dim_summaries_map)
    }
}

fn build_per_data_type_summary_section<P, TItem, F>(
    provider: &P,
    to_item_entries: &F,
) -> Option<HashMap<String, Vec<TItem>>>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    let mut type_summaries_map = HashMap::new();
    let mut insert_if_not_empty = |data_type: DataType, summary_item: &P::ItemSummary| {
        if !summary_item.is_empty() {
            type_summaries_map.insert(data_type.to_string(), to_item_entries(summary_item));
        }
    };

    insert_if_not_empty(
        DataType::BlockEntity,
        provider.get_total_block_entity_summary(),
    );
    insert_if_not_empty(DataType::Entity, provider.get_total_entity_summary());
    insert_if_not_empty(DataType::Player, provider.get_total_player_data_summary());

    if type_summaries_map.is_empty() {
        None
    } else {
        Some(type_summaries_map)
    }
}

fn build_per_dimension_detail_section<P, TItem, F>(
    provider: &P,
    to_item_entries: &F,
) -> Option<HashMap<String, HashMap<String, Vec<TItem>>>>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    let mut per_dimension_detail_map = HashMap::new();
    for (dimension, types_map) in provider.get_grouped_data() {
        let mut current_dim_data_type_map = HashMap::new();
        let mut insert_if_not_empty =
            |data_type: DataType, summary_item_opt: Option<&P::ItemSummary>| {
                if let Some(summary_item) = summary_item_opt
                    && !summary_item.is_empty()
                {
                    current_dim_data_type_map
                        .insert(data_type.to_string(), to_item_entries(summary_item));
                }
            };

        insert_if_not_empty(DataType::BlockEntity, types_map.get(&DataType::BlockEntity));
        insert_if_not_empty(DataType::Entity, types_map.get(&DataType::Entity));
        insert_if_not_empty(DataType::Player, types_map.get(&DataType::Player));

        if !current_dim_data_type_map.is_empty() {
            per_dimension_detail_map.insert(dimension.clone(), current_dim_data_type_map);
        }
    }
    if per_dimension_detail_map.is_empty() {
        None
    } else {
        Some(per_dimension_detail_map)
    }
}

fn generate_report_data<P, TItem, F>(
    provider: &P,
    args: &CliArgs,
    to_item_entries: F,
    grand_total_numeric_count: u64,
) -> Report<TItem>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    Report::<TItem> {
        per_dimension: args
            .per_dimension_summary
            .then(|| build_per_dimension_summary_section(provider, &to_item_entries))
            .flatten(),
        per_data_type: args
            .per_data_type_summary
            .then(|| build_per_data_type_summary_section(provider, &to_item_entries))
            .flatten(),
        per_dimension_detail: (args.per_dimension_summary && args.per_data_type_summary)
            .then(|| build_per_dimension_detail_section(provider, &to_item_entries))
            .flatten(),
        grand_total: {
            let total_summary_items = provider.get_total_combined_summary();
            if !total_summary_items.is_empty() {
                to_item_entries(total_summary_items)
            } else {
                Vec::new()
            }
        },
        grand_total_count: grand_total_numeric_count,
    }
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
        let nbt_ender_pearls_comp =
            nbt_val("{components:{\"minecraft:custom_data\":{stack_size:16}}}");
        counter_player.add(
            "minecraft:ender_pearl".to_string(),
            Some(&nbt_ender_pearls_comp),
            1,
        );
        map.merge_scope(scope_player, &counter_player);

        map
    }

    #[test]
    fn test_aggregated_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_data = AggregatedData::new(&counter_map);

        assert_eq!(agg_data.grouped.len(), 3);
        assert_eq!(
            agg_data.total_combined.total(),
            (10 + 5) + (5 + 15) + 3 + (1 + 1)
        );
    }

    #[test]
    fn test_aggregated_id_counts_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_id_data = AggregatedIdCountsData::new(&counter_map);
        assert_eq!(
            agg_id_data.total_combined.values().sum::<u64>(),
            13 + 5 + 5 + 15 + 1 + 1
        );
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
            output_format: OutputFormat::Table,
        }
    }

    #[test]
    fn test_print_report_as_tables_logic() {
        let counter_map = create_sample_counter_map();
        let mut args = mock_cli_args();
        args.view = ViewMode::ById;

        let data_provider = AggregatedIdCountsData::new(&counter_map);
        let mut printed_labels_counts: HashMap<String, usize> = HashMap::new();

        // Case 1: No dimension/type flags
        let report_data_case1 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.total_combined.values().sum(),
        );
        print_report_as_tables(&report_data_case1, &args, |items| {
            if !items.is_empty() {
                *printed_labels_counts
                    .entry("section_processed_case1".to_string())
                    .or_insert(0) += 1;
            }
        });
        assert_eq!(
            printed_labels_counts.get("section_processed_case1"),
            Some(&1)
        );
        printed_labels_counts.clear();

        // Case 2: Per dimension only
        args.per_dimension_summary = true;
        let report_data_case2 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.total_combined.values().sum(),
        );
        print_report_as_tables(&report_data_case2, &args, |items| {
            if !items.is_empty() {
                *printed_labels_counts
                    .entry("section_processed_case2".to_string())
                    .or_insert(0) += 1;
            }
        });
        assert_eq!(
            printed_labels_counts.get("section_processed_case2"),
            Some(&4)
        );
        printed_labels_counts.clear();
        args.per_dimension_summary = false;

        // Case 3: Per data type only
        args.per_data_type_summary = true;
        let report_data_case3 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.total_combined.values().sum(),
        );
        print_report_as_tables(&report_data_case3, &args, |items| {
            if !items.is_empty() {
                *printed_labels_counts
                    .entry("section_processed_case3".to_string())
                    .or_insert(0) += 1;
            }
        });
        assert_eq!(
            printed_labels_counts.get("section_processed_case3"),
            Some(&4)
        );
        printed_labels_counts.clear();
        args.per_data_type_summary = false;

        // Case 4: Both per dimension and per data type
        args.per_dimension_summary = true;
        args.per_data_type_summary = true;
        let report_data_case4 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.total_combined.values().sum(),
        );
        print_report_as_tables(&report_data_case4, &args, |items| {
            if !items.is_empty() {
                *printed_labels_counts
                    .entry("section_processed_case4".to_string())
                    .or_insert(0) += 1;
            }
        });
        assert_eq!(
            printed_labels_counts.get("section_processed_case4"),
            Some(&11)
        );
    }

    #[test]
    fn test_json_report_serialization_structure_detailed_view_no_flags() {
        let counter_map = create_sample_counter_map();
        let mut args = mock_cli_args();
        args.output_format = OutputFormat::Json;
        args.view = ViewMode::Detailed;

        let grand_total_numeric_count = counter_map.combined().total();
        let data_provider = AggregatedData::new(&counter_map);

        let report_data = generate_report_data(
            &data_provider,
            &args,
            to_detailed_item_entries,
            grand_total_numeric_count,
        );

        let json_value = serde_json::to_value(&report_data).unwrap();

        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert!(obj.get("per_dimension").is_none());
        assert!(obj.get("per_data_type").is_none());
        assert!(obj.get("per_dimension_detail").is_none());
        assert!(obj.get("grand_total").is_some());
        assert_eq!(
            obj.get("grand_total_count"),
            Some(&json!(grand_total_numeric_count))
        );

        let grand_total_arr = obj.get("grand_total").unwrap().as_array().unwrap();
        let expected_distinct_items_in_grand_total = data_provider
            .get_total_combined_summary()
            .detailed_counts()
            .len();
        assert_eq!(
            grand_total_arr.len(),
            expected_distinct_items_in_grand_total
        );
        if !grand_total_arr.is_empty() {
            let first_item = grand_total_arr[0].as_object().unwrap();
            assert!(first_item.contains_key("count"));
            assert!(first_item.contains_key("id"));
            assert!(first_item.contains_key("nbt"));
        }
    }

    #[test]
    fn test_json_report_serialization_structure_by_id_view_all_flags() {
        let counter_map = create_sample_counter_map();
        let mut args = mock_cli_args();
        args.output_format = OutputFormat::Json;
        args.view = ViewMode::ById;
        args.per_dimension_summary = true;
        args.per_data_type_summary = true;

        let data_provider = AggregatedIdCountsData::new(&counter_map);
        let grand_total_numeric_count = data_provider.total_combined.values().sum();

        let report_data = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            grand_total_numeric_count,
        );
        let json_value = serde_json::to_value(&report_data).unwrap();

        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert!(obj.get("per_dimension").is_some());
        assert!(obj.get("per_data_type").is_some());
        assert!(obj.get("per_dimension_detail").is_some());
        assert!(obj.get("grand_total").is_some());
        assert_eq!(
            obj.get("grand_total_count"),
            Some(&json!(grand_total_numeric_count))
        );

        let per_dim = obj.get("per_dimension").unwrap().as_object().unwrap();
        assert!(per_dim.contains_key("overworld"));
        let overworld_summary = per_dim.get("overworld").unwrap().as_array().unwrap();
        if !overworld_summary.is_empty() {
            let first_item = overworld_summary[0].as_object().unwrap();
            assert!(first_item.contains_key("count"));
            assert!(first_item.contains_key("id"));
            assert!(!first_item.contains_key("nbt"));
        }
    }
}
