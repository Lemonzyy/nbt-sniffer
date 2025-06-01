use std::collections::{BTreeMap, HashMap};

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use csv::Writer;

use crate::{
    DataType,
    cli::CliArgs,
    counter::{Counter, CounterMap},
    escape_nbt_string,
};

struct AggregatedData {
    grouped: BTreeMap<String, BTreeMap<DataType, Counter>>,
    total_block_entity: Counter,
    total_entity: Counter,
    total_combined: Counter,
}

impl AggregatedData {
    fn new(counter_map: &CounterMap) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<DataType, Counter>> = BTreeMap::new();
        let mut total_block_entity = Counter::new();
        let mut total_entity = Counter::new();
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
            }
            total_combined.merge(counter);
        }

        Self {
            grouped,
            total_block_entity,
            total_entity,
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
    type ItemSummary: Clone; // e.g., Counter or HashMap<String, u64>

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>>;
    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary;
    fn get_total_entity_summary(&self) -> &Self::ItemSummary;
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
    total_combined: HashMap<String, u64>,
}

impl AggregatedIdCountsData {
    fn new(counter_map: &CounterMap) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<DataType, HashMap<String, u64>>> =
            BTreeMap::new();
        let mut total_block_entity: HashMap<String, u64> = HashMap::new();
        let mut total_entity: HashMap<String, u64> = HashMap::new();
        let mut total_combined: HashMap<String, u64> = HashMap::new();

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
                }
            }
        }
        Self {
            grouped,
            total_block_entity,
            total_entity,
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
        (false, false) => print_fn_for_summary(provider.get_total_combined_summary(), "Total"),
        (true, false) => {
            for dimension in provider.get_grouped_data().keys() {
                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                print_fn_for_summary(&combined_dim_summary, &format!("Dimension: {dimension}"));
            }
            print_fn_for_summary(provider.get_total_combined_summary(), "Total");
        }
        (false, true) => {
            print_fn_for_summary(provider.get_total_block_entity_summary(), "Block Entity");
            print_fn_for_summary(provider.get_total_entity_summary(), "Entity");
            print_fn_for_summary(provider.get_total_combined_summary(), "Total");
        }
        (true, true) => {
            for (dimension, types_map) in provider.get_grouped_data() {
                println!("\nDimension: {dimension}");

                if let Some(summary_item) = types_map.get(&DataType::BlockEntity) {
                    print_fn_for_summary(summary_item, "Block Entity");
                }
                if let Some(summary_item) = types_map.get(&DataType::Entity) {
                    print_fn_for_summary(summary_item, "Entity");
                }

                let combined_dim_summary = provider.calculate_dimension_combined_summary(dimension);
                print_fn_for_summary(&combined_dim_summary, "Summary");
            }

            print_fn_for_summary(provider.get_total_block_entity_summary(), "\nBlock Entity");
            print_fn_for_summary(provider.get_total_entity_summary(), "Entity");
            print_fn_for_summary(provider.get_total_combined_summary(), "Total");
        }
    }
}

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedData::new(counter_map);
    execute_summary_printing(&data_provider, args, |counter_summary, label| {
        if !label.starts_with('\n') {
            println!("{label}:");
        } else {
            println!("{}:", &label[1..]);
        }
        print_detailed_counter(counter_summary, args);
    });
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedData::new(counter_map);
    execute_summary_printing(&data_provider, args, |counter_summary, label| {
        let display_label = match label {
            "Block Entity" => "Total Block Entity",
            "Entity" => "Total Entity",
            _ => label,
        };

        if !label.starts_with('\n') && !label.starts_with("Total") && !label.starts_with("Summary")
        {
            println!("{display_label}:");
        } else {
            println!("{}:", &display_label.trim_start_matches('\n'));
        }
        print_nbt_counter(counter_summary, args);
    });
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
    let data_provider = AggregatedIdCountsData::new(counter_map);
    execute_summary_printing(&data_provider, args, |id_map_summary, label| {
        let mut effective_label = label.to_string();
        if label.starts_with('\n') {
            effective_label = label.trim_start_matches('\n').to_string();
        }
        println!("{effective_label}:");
        print_id_map(id_map_summary, args);
    });
}

fn print_detailed_counter(counter: &Counter, args: &CliArgs) {
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

    print_table_or_csv(
        &["Count", "ID", "NBT"],
        detailed_vec,
        args,
        |(id, nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt);
            (
                vec![count.to_string(), id.clone(), nbt_str.clone()],
                vec![Cell::new(count), Cell::new(id), Cell::new(nbt_str)],
            )
        },
        Some(2),
    );
}

fn print_id_map(map: &HashMap<String, u64>, args: &CliArgs) {
    let mut vec: Vec<_> = map.iter().map(|(id, &count)| (id.clone(), count)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    print_table_or_csv(
        &["Count", "Item ID"],
        vec,
        args,
        |(id, count)| {
            (
                vec![count.to_string(), id.clone()],
                vec![Cell::new(count), Cell::new(id)],
            )
        },
        None,
    );
}

fn print_nbt_counter(counter: &Counter, args: &CliArgs) {
    let mut by_nbt_vec: Vec<_> = counter.total_by_nbt().into_iter().collect();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    print_table_or_csv(
        &["Count", "NBT"],
        by_nbt_vec,
        args,
        |(nbt_opt, count)| {
            let nbt_str = format_nbt_string(nbt_opt);
            (
                vec![count.to_string(), nbt_str.clone()],
                vec![Cell::new(count), Cell::new(nbt_str)],
            )
        },
        Some(1),
    );
}

fn print_table_or_csv<T, F>(
    headers: &[&str],
    data: Vec<T>,
    args: &CliArgs,
    mut formatter: F,
    left_align_col: Option<usize>,
) where
    F: FnMut(&T) -> (Vec<String>, Vec<Cell>),
{
    if args.csv {
        let mut wtr = Writer::from_writer(std::io::stdout());
        wtr.write_record(headers)
            .expect("Failed to write CSV headers");
        for item in data {
            let (fields, _) = formatter(&item);
            wtr.write_record(fields).expect("Failed to write CSV row");
        }
        wtr.flush().expect("Failed to flush CSV writer");
    } else {
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
            let (_, cells) = formatter(&item);
            table.add_row(cells);
        }
        println!("{table}");
    }
}

fn format_nbt_string(nbt_opt: &Option<String>) -> String {
    nbt_opt
        .as_deref()
        .map(escape_nbt_string)
        .unwrap_or_else(|| "No NBT".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DataType, Scope,
        cli::{CliArgs, ViewMode},
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
        ); // 5 damaged iron swords
        counter_ow_e.add("minecraft:rotten_flesh".to_string(), None, 15); // 15 rotten flesh
        map.merge_scope(scope_ow_e, &counter_ow_e);

        let scope_nether_be = Scope {
            dimension: "nether".to_string(),
            data_type: DataType::BlockEntity,
        };
        let mut counter_nether_be = Counter::new();
        counter_nether_be.add("minecraft:chest".to_string(), None, 3); // Another chest
        map.merge_scope(scope_nether_be, &counter_nether_be);

        map
    }

    #[test]
    fn test_aggregated_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_data = AggregatedData::new(&counter_map);

        // Check grouped data
        assert_eq!(agg_data.grouped.len(), 2); // overworld, nether
        assert_eq!(agg_data.grouped.get("overworld").unwrap().len(), 2); // BlockEntity, Entity
        assert_eq!(
            agg_data
                .grouped
                .get("overworld")
                .unwrap()
                .get(&DataType::BlockEntity)
                .unwrap()
                .total(),
            15
        ); // 10 chests + 5 furnaces
        assert_eq!(
            agg_data
                .grouped
                .get("overworld")
                .unwrap()
                .get(&DataType::Entity)
                .unwrap()
                .total(),
            20 // 5 iron_swords + 15 rotten_flesh
        ); // 20 zombies
        assert_eq!(
            agg_data
                .grouped
                .get("nether")
                .unwrap()
                .get(&DataType::BlockEntity)
                .unwrap()
                .total(),
            3
        ); // 3 chests

        // Check totals
        assert_eq!(agg_data.total_block_entity.total(), 15 + 3);
        assert_eq!(agg_data.total_entity.total(), 20);
        assert_eq!(agg_data.total_combined.total(), 15 + 3 + 20);

        // Check dimension_combined
        assert_eq!(agg_data.dimension_combined("overworld").total(), 15 + 20);
        assert_eq!(agg_data.dimension_combined("nether").total(), 3);
        assert_eq!(agg_data.dimension_combined("unknown_dim").total(), 0);
    }

    #[test]
    fn test_aggregated_id_counts_data_new() {
        let counter_map = create_sample_counter_map();
        let agg_id_data = AggregatedIdCountsData::new(&counter_map);

        // Check grouped data
        let ov_be_counts = agg_id_data
            .grouped
            .get("overworld")
            .unwrap()
            .get(&DataType::BlockEntity)
            .unwrap();
        assert_eq!(ov_be_counts.get("minecraft:chest"), Some(&10));
        assert_eq!(ov_be_counts.get("minecraft:furnace"), Some(&5));

        let ov_e_counts = agg_id_data
            .grouped
            .get("overworld")
            .unwrap()
            .get(&DataType::Entity)
            .unwrap();
        assert_eq!(ov_e_counts.get("minecraft:iron_sword"), Some(&5));
        assert_eq!(ov_e_counts.get("minecraft:rotten_flesh"), Some(&15));

        // Check totals
        assert_eq!(
            agg_id_data.total_block_entity.get("minecraft:chest"),
            Some(&13)
        ); // 10 + 3
        assert_eq!(
            agg_id_data.total_block_entity.get("minecraft:furnace"),
            Some(&5)
        );
        assert_eq!(
            agg_id_data.total_entity.get("minecraft:iron_sword"),
            Some(&5)
        );
        assert_eq!(
            agg_id_data.total_entity.get("minecraft:rotten_flesh"),
            Some(&15)
        );
        assert_eq!(agg_id_data.total_combined.get("minecraft:chest"), Some(&13));
        assert_eq!(
            agg_id_data.total_combined.get("minecraft:iron_sword"),
            Some(&5)
        );
        assert_eq!(
            agg_id_data.total_combined.get("minecraft:rotten_flesh"),
            Some(&15)
        );

        // Check dimension_combined
        let ov_dim_combined = agg_id_data.dimension_combined("overworld");
        assert_eq!(ov_dim_combined.get("minecraft:chest"), Some(&10));
        assert_eq!(ov_dim_combined.get("minecraft:furnace"), Some(&5));
        assert_eq!(ov_dim_combined.get("minecraft:iron_sword"), Some(&5));
        assert_eq!(ov_dim_combined.get("minecraft:rotten_flesh"), Some(&15));
    }

    // Helper for execute_summary_printing tests
    fn mock_cli_args() -> CliArgs {
        CliArgs {
            world_path: PathBuf::from("dummy"),
            all: true,
            items: vec![],
            view: ViewMode::ById,
            csv: false,
            show_nbt: false,
            per_source_summary: false,
            per_dimension_summary: false,
            per_data_type_summary: false,
            verbose: false,
        }
    }

    #[test]
    fn test_execute_summary_printing_logic() {
        let counter_map = create_sample_counter_map();
        let data_provider = AggregatedIdCountsData::new(&counter_map); // Using IdCounts for simplicity
        let mut printed_labels: Vec<String> = Vec::new();

        // Case 1: No per_dimension or per_data_type
        let mut args = mock_cli_args();
        {
            let mut print_fn_case1 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case1);
        }
        assert_eq!(printed_labels, vec!["Total"]);
        printed_labels.clear();

        // Case 2: Per dimension only
        args.per_dimension_summary = true;
        {
            let mut print_fn_case2 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case2);
        }
        assert_eq!(
            printed_labels,
            vec!["Dimension: nether", "Dimension: overworld", "Total"]
        );
        printed_labels.clear();
        args.per_dimension_summary = false; // Reset

        // Case 3: Per data type only
        args.per_data_type_summary = true;
        {
            let mut print_fn_case3 = |_: &HashMap<String, u64>, label: &str| {
                printed_labels.push(label.to_string());
            };
            execute_summary_printing(&data_provider, &args, &mut print_fn_case3);
        }
        assert_eq!(printed_labels, vec!["Block Entity", "Entity", "Total"]);
        printed_labels.clear();

        // Case 4: Both per_dimension and per_data_type
        // Note: The order of dimensions from BTreeMap keys() will be alphabetical.
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
                "Block Entity", // (within nether, due to println! in execute_summary_printing)
                "Summary",      // (for nether)
                "Block Entity", // (within overworld)
                "Entity",       // (within overworld)
                "Summary",      // (for overworld)
                "\nBlock Entity",
                "Entity",
                "Total"
            ]
        );
    }
}
