use super::structures::{Report, ReportItemDetailed, ReportItemId, ReportItemNbt};
use crate::{
    DataType,
    cli::{CliArgs, ViewMode},
};
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use serde::Serialize;
use strum::IntoEnumIterator;

/// Defines the type of section being printed for table output, used to determine titles and formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrintSectionType<'a> {
    DimensionSummary(&'a str), // Dimension name for its overall summary
    GlobalDataTypeSummary(DataType),
    DimensionDataTypeDetail(&'a str, DataType), // Dimension name, DataType
    DimensionOverallSummary(&'a str),           // "Summary" label for a dimension's combined types
    GrandTotal,
}

impl<'a> PrintSectionType<'a> {
    /// Gets the display title for the section and a prefix (e.g., for indentation).
    fn get_title_and_prefix(&self, view_mode: &ViewMode) -> (String, String) {
        match self {
            PrintSectionType::DimensionSummary(dim) => {
                (format!("Dimension: {dim}"), "".to_string())
            }
            PrintSectionType::GlobalDataTypeSummary(dt) => {
                let base_title = dt.to_string();
                if *view_mode == ViewMode::ByNbt {
                    (format!("Total {base_title}"), "".to_string())
                } else {
                    (base_title, "".to_string())
                }
            }
            PrintSectionType::DimensionDataTypeDetail(_dim, dt) => {
                (dt.to_string(), "  ".to_string())
            } // Indent
            PrintSectionType::DimensionOverallSummary(_dim) => {
                ("Summary".to_string(), "  ".to_string())
            } // Indent
            PrintSectionType::GrandTotal => ("Total".to_string(), "".to_string()),
        }
    }
}

/// Helper to print a single section of the report.
fn print_section_content<TItem>(
    items: &[TItem],
    section_type: &PrintSectionType,
    view_mode: &ViewMode,
    print_items_fn: &mut impl FnMut(&[TItem]),
    needs_leading_newline: bool,
) where
    TItem: Clone + Serialize,
{
    if items.is_empty() {
        return;
    }

    let (title, prefix) = section_type.get_title_and_prefix(view_mode);

    if needs_leading_newline {
        println!();
    }

    println!("{prefix}{title}:");
    print_items_fn(items);
}

/// Prints the report data as formatted tables.
pub fn print_report_as_tables<TItem>(
    report: &Report<TItem>,
    args: &CliArgs,
    mut print_items_fn: impl FnMut(&[TItem]),
) where
    TItem: Clone + Serialize,
{
    let mut needs_newline_for_next_major_section = false;

    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {
            // No specific summaries, only grand total will be printed later
        }
        (true, false) => {
            // Only per-dimension summaries
            if let Some(per_dimension_data) = &report.per_dimension_summary {
                for (i, (dimension_name, items)) in per_dimension_data.iter().enumerate() {
                    print_section_content(
                        items,
                        &PrintSectionType::DimensionSummary(dimension_name),
                        &args.view,
                        &mut print_items_fn,
                        i > 0, // Add newline before subsequent dimension summaries
                    );
                    needs_newline_for_next_major_section = true;
                }
            }
        }
        (false, true) => {
            // Only per-data_type summaries (global)
            if let Some(per_data_type_data) = &report.per_data_type_summary {
                for (i, data_type) in DataType::iter().enumerate() {
                    if let Some(items) = per_data_type_data.get(&data_type) {
                        print_section_content(
                            items,
                            &PrintSectionType::GlobalDataTypeSummary(data_type),
                            &args.view,
                            &mut print_items_fn,
                            i > 0, // Add newline before subsequent global type summaries
                        );
                        needs_newline_for_next_major_section = true;
                    }
                }
            }
        }
        (true, true) => {
            // Both per-dimension details and global summaries
            if let Some(per_dimension_detail_data) = &report.per_dimension_detail {
                for (dimension_name, type_map) in per_dimension_detail_data {
                    println!("\nDimension: {dimension_name}"); // Always start a new dimension section with a newline
                    needs_newline_for_next_major_section = true;
                    for data_type in DataType::iter() {
                        if let Some(items) = type_map.get(&data_type) {
                            print_section_content(
                                items,
                                &PrintSectionType::DimensionDataTypeDetail(
                                    dimension_name,
                                    data_type,
                                ),
                                &args.view,
                                &mut print_items_fn,
                                false, // No extra newline within a dimension's details
                            );
                        }
                    }
                    // Print "Summary" for the dimension
                    if let Some(per_dimension_data) = &report.per_dimension_summary
                        && let Some(dim_summary_items) = per_dimension_data.get(dimension_name)
                    {
                        print_section_content(
                            dim_summary_items,
                            &PrintSectionType::DimensionOverallSummary(dimension_name),
                            &args.view,
                            &mut print_items_fn,
                            false, // No extra newline for the dimension's own summary
                        );
                    }
                }
            }
            // Print global data type summaries
            if let Some(per_data_type_data) = &report.per_data_type_summary {
                let mut first_global_summary_printed = false;
                for data_type in DataType::iter() {
                    if let Some(items) = per_data_type_data.get(&data_type) {
                        // Add newline if it's not the very first global summary AND
                        // (either dimension details were printed OR it's not the first global summary item)
                        let needs_newline =
                            needs_newline_for_next_major_section || first_global_summary_printed;
                        print_section_content(
                            items,
                            &PrintSectionType::GlobalDataTypeSummary(data_type),
                            &args.view,
                            &mut print_items_fn,
                            needs_newline,
                        );
                        first_global_summary_printed = true;
                        needs_newline_for_next_major_section = true; // Ensure next major section (like Total) gets a newline
                    }
                }
            }
        }
    }
    // Always print grand total
    let grand_total_needs_newline =
        needs_newline_for_next_major_section && !report.grand_total.is_empty();
    print_section_content(
        &report.grand_total,
        &PrintSectionType::GrandTotal,
        &args.view,
        &mut print_items_fn,
        grand_total_needs_newline,
    );
}

pub fn print_detailed_counter(items: &[ReportItemDetailed]) {
    if items.is_empty() {
        return;
    }
    print_table(
        &["Count", "ID", "NBT"],
        items,
        |item| {
            vec![
                Cell::new(item.count),
                Cell::new(&item.id),
                Cell::new(item.nbt.clone().unwrap_or_else(|| "No NBT".into())),
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
        items,
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
        items,
        |item| {
            vec![
                Cell::new(item.count),
                Cell::new(item.nbt.clone().unwrap_or_else(|| "No NBT".into())),
            ]
        },
        Some(1),
    );
}

fn print_table<T, F>(
    headers: &[&str],
    data: &[T],
    mut row_formatter: F,
    left_align_col_idx: Option<usize>,
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

    if let Some(col_idx) = left_align_col_idx
        && let Some(column) = table.column_mut(col_idx)
    {
        column.set_cell_alignment(CellAlignment::Left);
    }

    for item in data {
        table.add_row(row_formatter(item));
    }
    println!("{table}");
}
