pub mod aggregation;
pub mod builder;
pub mod item_conversion;
pub mod json_printer;
pub mod structures;
pub mod table_printer;

use std::collections::HashMap;

use crate::{
    cli::{CliArgs, OutputFormat},
    counter::{Counter, CounterMap},
};
use aggregation::{AggregationResult, IsEmpty};
use serde::Serialize;
use serde_json::json;

use builder::generate_report_data;
use item_conversion::{to_detailed_item_entries, to_id_item_entries, to_nbt_item_entries};
use json_printer::print_json_output;
use table_printer::{
    print_detailed_counter, print_id_map, print_nbt_counter, print_report_as_tables,
};

/// Generic helper to generate and output a report based on the view mode.
fn generate_and_output_report<TAggregable, FConvert, FPrintTable, TReportItem>(
    counter_map: &CounterMap,
    args: &CliArgs,
    item_converter: FConvert,
    table_printer: FPrintTable,
    grand_total_calculator: impl Fn(&TAggregable) -> u64,
) where
    TAggregable: aggregation::Aggregable,
    FConvert: Fn(&TAggregable) -> Vec<TReportItem>,
    FPrintTable: FnMut(&[TReportItem]),
    TReportItem: Serialize + Clone,
{
    let data_provider = AggregationResult::<TAggregable>::new(counter_map);
    let grand_total_numeric_count = grand_total_calculator(&data_provider.total_combined);

    let report_data = generate_report_data(
        &data_provider,
        args,
        item_converter,
        grand_total_numeric_count,
    );

    if args.output_format.is_json() {
        let json_value = serde_json::to_value(&report_data).unwrap_or_else(|e| {
            eprintln!("Error serializing report to JSON: {e}");
            json!({ "error": format!("Failed to serialize report: {e}") })
        });
        print_json_output(&json_value, args.output_format == OutputFormat::PrettyJson);
    } else {
        print_report_as_tables(&report_data, args, table_printer);
    }
}

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    generate_and_output_report(
        counter_map,
        args,
        to_detailed_item_entries,
        print_detailed_counter,
        |counter: &Counter| counter.total(),
    );
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    generate_and_output_report(
        counter_map,
        args,
        to_nbt_item_entries,
        print_nbt_counter,
        |counter: &Counter| counter.total(),
    );
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
    generate_and_output_report(
        counter_map,
        args,
        to_id_item_entries,
        print_id_map,
        |map: &HashMap<String, u64>| map.values().sum(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DataType, Scope,
        cli::{CliArgs, OutputFormat, ViewMode},
        counter::{Counter, CounterMap},
        view::aggregation::SummaryDataProvider,
    };
    use std::{collections::HashMap, path::PathBuf};
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
    fn test_aggregation_result_counter_new() {
        let counter_map = create_sample_counter_map();
        let agg_data = AggregationResult::<Counter>::new(&counter_map);

        assert_eq!(agg_data.grouped.len(), 3);
        assert_eq!(
            agg_data.total_combined.total(),
            (10 + 5) + (5 + 15) + 3 + (1 + 1) // 15 + 20 + 3 + 2 = 40
        );
    }

    #[test]
    fn test_aggregation_result_id_counts_new() {
        let counter_map = create_sample_counter_map();
        let agg_id_data = AggregationResult::<HashMap<String, u64>>::new(&counter_map);
        assert_eq!(
            agg_id_data.total_combined.values().sum::<u64>(),
            13 + 5 + 5 + 15 + 1 + 1 // chest(10+3) + furnace(5) + iron_sword(5) + rotten_flesh(15) + diamond_sword(1) + ender_pearl(1)
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

        let data_provider = AggregationResult::<HashMap<String, u64>>::new(&counter_map);
        let mut printed_labels_counts: HashMap<String, usize> = HashMap::new();

        // Case 1: No dimension/type flags
        let report_data_case1 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.get_total_combined_summary().values().sum(),
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
            Some(&1) // Only grand total
        );
        printed_labels_counts.clear();

        // Case 2: Per dimension only
        args.per_dimension_summary = true;
        let report_data_case2 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.get_total_combined_summary().values().sum(),
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
            Some(&4) // 3 dimensions + grand total
        );
        printed_labels_counts.clear();
        args.per_dimension_summary = false;

        // Case 3: Per data type only
        args.per_data_type_summary = true;
        let report_data_case3 = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            data_provider.get_total_combined_summary().values().sum(),
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
            Some(&4) // 3 data types + grand total
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
            data_provider.get_total_combined_summary().values().sum(),
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
        let data_provider = AggregationResult::<Counter>::new(&counter_map);

        let report_data = generate_report_data(
            &data_provider,
            &args,
            to_detailed_item_entries,
            grand_total_numeric_count,
        );

        let json_value = serde_json::to_value(&report_data).unwrap();

        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert!(obj.get("per_dimension_summary").is_none());
        assert!(obj.get("per_data_type_summary").is_none());
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

        let data_provider = AggregationResult::<HashMap<String, u64>>::new(&counter_map);
        let grand_total_numeric_count = data_provider.get_total_combined_summary().values().sum();

        let report_data = generate_report_data(
            &data_provider,
            &args,
            to_id_item_entries,
            grand_total_numeric_count,
        );
        let json_value = serde_json::to_value(&report_data).unwrap();

        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert!(obj.get("per_dimension_summary").is_some());
        assert!(obj.get("per_data_type_summary").is_some());
        assert!(obj.get("per_dimension_detail").is_some());
        assert!(obj.get("grand_total").is_some());
        assert_eq!(
            obj.get("grand_total_count"),
            Some(&json!(grand_total_numeric_count))
        );

        let per_dim = obj
            .get("per_dimension_summary")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(per_dim.contains_key("overworld"));
        let overworld_summary = per_dim.get("overworld").unwrap().as_array().unwrap();
        if !overworld_summary.is_empty() {
            let first_item = overworld_summary[0].as_object().unwrap();
            assert!(first_item.contains_key("count"));
            assert!(first_item.contains_key("id"));
            assert!(!first_item.contains_key("nbt")); // ById view doesn't have NBT in items
        }

        // Check that per_data_type_summary keys are serialized as strings by serde_json
        let per_type_summary = obj
            .get("per_data_type_summary")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(per_type_summary.contains_key("BlockEntity")); // strum::Display is "Block Entity", but serde_json uses variant name for enum keys
        assert!(per_type_summary.contains_key("Entity"));
        assert!(per_type_summary.contains_key("Player"));
    }
}
