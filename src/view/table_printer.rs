use super::structures::{Report, ReportItemDetailed, ReportItemId, ReportItemNbt};
use crate::{
    DataType,
    cli::{CliArgs, ViewMode},
};
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use serde::Serialize; // Needed for TItem: Serialize bound in print_report_as_tables

/// Prints the report data as formatted tables.
pub fn print_report_as_tables<TItem>(
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

pub fn print_detailed_counter(items: &[ReportItemDetailed]) {
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

pub fn print_id_map(items: &[ReportItemId]) {
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

pub fn print_nbt_counter(items: &[ReportItemNbt]) {
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
        table.add_row(row_formatter(&item));
    }
    println!("{table}");
}
