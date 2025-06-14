use super::{aggregation::SummaryDataProvider, structures::Report};
use crate::{DataType, cli::CliArgs, view::IsEmpty};
use serde::Serialize;
use std::collections::HashMap;
use strum::IntoEnumIterator;

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
        // Ensure the combined summary for the dimension is not empty before adding
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
) -> Option<HashMap<DataType, Vec<TItem>>>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    let mut type_summaries_map = HashMap::new();

    for data_type in DataType::iter() {
        let summary_item = match data_type {
            DataType::BlockEntity => provider.get_total_block_entity_summary(),
            DataType::Entity => provider.get_total_entity_summary(),
            DataType::Player => provider.get_total_player_data_summary(),
        };
        if !summary_item.is_empty() {
            type_summaries_map.insert(data_type, to_item_entries(summary_item));
        }
    }

    if type_summaries_map.is_empty() {
        None
    } else {
        Some(type_summaries_map)
    }
}

fn build_per_dimension_detail_section<P, TItem, F>(
    provider: &P,
    to_item_entries: &F,
) -> Option<HashMap<String, HashMap<DataType, Vec<TItem>>>>
where
    P: SummaryDataProvider,
    TItem: Serialize,
    F: Fn(&P::ItemSummary) -> Vec<TItem>,
{
    let mut per_dimension_detail_map = HashMap::new();
    for (dimension, types_map) in provider.get_grouped_data() {
        let mut current_dim_data_type_map = HashMap::new();

        for data_type in DataType::iter() {
            if let Some(summary_item) = types_map.get(&data_type)
                && !summary_item.is_empty()
            {
                current_dim_data_type_map.insert(data_type, to_item_entries(summary_item));
            }
        }

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

pub fn generate_report_data<P, TItem, F>(
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
        per_dimension_summary: args
            .per_dimension_summary
            .then(|| build_per_dimension_summary_section(provider, &to_item_entries))
            .flatten(),
        per_data_type_summary: args
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
