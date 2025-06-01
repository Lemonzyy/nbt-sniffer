use std::collections::{BTreeMap, HashMap};

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use csv::Writer;

use crate::{
    DataType,
    cli::CliArgs,
    counter::{Counter, CounterMap},
    escape_nbt_string,
};

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    // Group counters by dimension and data type
    let mut per_dimension: BTreeMap<String, BTreeMap<DataType, Counter>> = BTreeMap::new();

    // Group counters by data type globally (all dimensions)
    let mut per_data_type: BTreeMap<DataType, Counter> = BTreeMap::new();

    // Grand total for all dimensions and data types
    let mut grand_total = Counter::new();

    // Fill groups
    for (scope, counter) in counter_map.iter() {
        per_dimension
            .entry(scope.dimension.clone())
            .or_default()
            .entry(scope.data_type.clone())
            .or_default()
            .merge(counter);

        per_data_type
            .entry(scope.data_type.clone())
            .or_default()
            .merge(counter);

        grand_total.merge(counter);
    }

    // Helper to print a Counter with title
    fn print_counter(title: &str, counter: &Counter, args: &CliArgs) {
        println!("{title}:");
        display_detailed(counter, args);
    }

    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {
            // 1) No flag: single grand total for all dimensions and data types
            print_counter("Total (all dimensions, all data types)", &grand_total, args);
        }
        (true, false) => {
            // 2) --per-dimension-summary
            // For each dimension: total sum of all data types
            for (dimension, dt_map) in &per_dimension {
                let mut total_dim = Counter::new();
                for counter in dt_map.values() {
                    total_dim.merge(counter);
                }
                print_counter(
                    &format!("Dimension: {dimension} (all data types combined)"),
                    &total_dim,
                    args,
                );
            }
            // Grand total
            print_counter("Total (all dimensions, all data types)", &grand_total, args);
        }
        (false, true) => {
            // 3) --per-data-type-summary
            // For each data type: total sum of all dimensions for that data type
            for (data_type, counter) in &per_data_type {
                print_counter(
                    &format!("Data Type: {:?} (all dimensions)", data_type),
                    counter,
                    args,
                );
            }
            // Grand total
            print_counter("Total (all dimensions, all data types)", &grand_total, args);
        }
        (true, true) => {
            // 4) both flags
            // For each dimension:
            for (dimension, dt_map) in &per_dimension {
                // a) print table per data type
                for (data_type, counter) in dt_map {
                    print_counter(
                        &format!("Dimension: {dimension} - Data Type: {:?}", data_type),
                        counter,
                        args,
                    );
                }
                // b) total table of that dimension (sum of all data types)
                let mut total_dim = Counter::new();
                for counter in dt_map.values() {
                    total_dim.merge(counter);
                }
                print_counter(
                    &format!("Dimension: {dimension} (all data types combined)"),
                    &total_dim,
                    args,
                );
            }

            // For each data type: total sum all dimensions
            for (data_type, counter) in &per_data_type {
                print_counter(
                    &format!("Data Type: {:?} (all dimensions)", data_type),
                    counter,
                    args,
                );
            }

            // Grand total
            print_counter("Total (all dimensions, all data types)", &grand_total, args);
        }
    }
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
    // dimension -> data_type -> id -> count
    let mut grouped: BTreeMap<String, BTreeMap<DataType, HashMap<String, u64>>> = BTreeMap::new();

    let mut total_block_entity: HashMap<String, u64> = HashMap::new();
    let mut total_entity: HashMap<String, u64> = HashMap::new();
    let mut total_combined: HashMap<String, u64> = HashMap::new();

    for (scope, counter) in counter_map.iter() {
        let id_map = grouped
            .entry(scope.dimension.clone())
            .or_default()
            .entry(scope.data_type.clone())
            .or_default();

        for (id, count) in counter.total_by_id() {
            *id_map.entry(id.clone()).or_default() += count;

            match scope.data_type {
                DataType::BlockEntity => {
                    *total_block_entity.entry(id.clone()).or_default() += count;
                }
                DataType::Entity => {
                    *total_entity.entry(id.clone()).or_default() += count;
                }
                _ => {}
            }

            *total_combined.entry(id.clone()).or_default() += count;
        }
    }

    // No flags: just print combined total
    if !args.per_dimension_summary && !args.per_data_type_summary {
        println!("Total:");
        print_id_map(&total_combined, args);
        return;
    }

    // --per-dimension-summary only: print per dimension combined summary + grand total
    if args.per_dimension_summary && !args.per_data_type_summary {
        for (dimension, types_map) in &grouped {
            println!("\nDimension: {}", dimension);

            // Merge all data types for this dimension into one summary
            let mut dimension_summary: HashMap<String, u64> = HashMap::new();
            for map in types_map.values() {
                for (id, count) in map {
                    *dimension_summary.entry(id.clone()).or_default() += count;
                }
            }
            println!("Summary:");
            print_id_map(&dimension_summary, args);
        }

        println!("\nTotal:");
        print_id_map(&total_combined, args);
        return;
    }

    // --per-data-type-summary only: print one table per data type + grand total
    if !args.per_dimension_summary && args.per_data_type_summary {
        // Always print block entity total even if empty (to be consistent)
        println!("Block Entity:");
        print_id_map(&total_block_entity, args);

        println!("Entity:");
        print_id_map(&total_entity, args);

        println!("Total:");
        print_id_map(&total_combined, args);
        return;
    }

    // Both --per-dimension-summary and --per-data-type-summary:
    if args.per_dimension_summary && args.per_data_type_summary {
        for (dimension, types_map) in &grouped {
            println!("\nDimension: {}", dimension);

            // One table per data type for this dimension
            for data_type in &[DataType::BlockEntity, DataType::Entity] {
                if let Some(id_map) = types_map.get(data_type) {
                    println!("{}:", data_type);
                    print_id_map(id_map, args);
                }
            }

            // Combined dimension summary (all data types)
            let mut dimension_summary: HashMap<String, u64> = HashMap::new();
            for map in types_map.values() {
                for (id, count) in map {
                    *dimension_summary.entry(id.clone()).or_default() += count;
                }
            }
            println!("Summary:");
            print_id_map(&dimension_summary, args);
        }

        // Always print per data type totals (all dimensions), even if empty
        println!("\nBlock Entity:");
        print_id_map(&total_block_entity, args);

        println!("Entity:");
        print_id_map(&total_entity, args);

        // Grand total
        println!("Total:");
        print_id_map(&total_combined, args);
        return;
    }
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    // dimension -> data_type -> Counter
    let mut grouped: BTreeMap<String, BTreeMap<DataType, Counter>> = BTreeMap::new();

    let mut total_block_entity = Counter::new();
    let mut total_entity = Counter::new();
    let mut total_combined = Counter::new();

    for (scope, counter) in counter_map.iter() {
        grouped
            .entry(scope.dimension.clone())
            .or_default()
            .entry(scope.data_type.clone())
            .or_insert_with(Counter::new)
            .merge(counter);

        match scope.data_type {
            DataType::BlockEntity => total_block_entity.merge(counter),
            DataType::Entity => total_entity.merge(counter),
            _ => {}
        }
        total_combined.merge(counter);
    }

    let per_dim = args.per_source_summary;
    let per_type = args.per_data_type_summary;

    match (per_dim, per_type) {
        (false, false) => {
            // No flags - print total only
            print_nbt_counter(&total_combined, args);
        }
        (true, false) => {
            // Per dimension only
            println!("== Per-dimension summary ==");

            for (dimension, types_map) in &grouped {
                println!("\nDimension: {}", dimension);

                // Sum all data types for this dimension
                let mut dimension_combined = Counter::new();
                for counter in types_map.values() {
                    dimension_combined.merge(counter);
                }

                print_nbt_counter(&dimension_combined, args);
            }

            println!("\nTotal:");
            print_nbt_counter(&total_combined, args);
        }
        (false, true) => {
            // Per data type only
            println!("== Per-data-type summary ==");

            println!("Total Block Entity:");
            print_nbt_counter(&total_block_entity, args);

            println!("Total Entity:");
            print_nbt_counter(&total_entity, args);

            println!("Total:");
            print_nbt_counter(&total_combined, args);
        }
        (true, true) => {
            // Both flags
            println!("== Both per-dimension and per-data-type summary ==");

            for (dimension, types_map) in &grouped {
                println!("\nDimension: {}", dimension);

                if let Some(block_entity_counter) = types_map.get(&DataType::BlockEntity) {
                    println!("Block Entity:");
                    print_nbt_counter(block_entity_counter, args);
                }

                if let Some(entity_counter) = types_map.get(&DataType::Entity) {
                    println!("Entity:");
                    print_nbt_counter(entity_counter, args);
                }

                let mut dimension_combined = Counter::new();
                for counter in types_map.values() {
                    dimension_combined.merge(counter);
                }
                println!("Summary:");
                print_nbt_counter(&dimension_combined, args);
            }

            println!("\nTotal Block Entity:");
            print_nbt_counter(&total_block_entity, args);

            println!("Total Entity:");
            print_nbt_counter(&total_entity, args);

            println!("Total:");
            print_nbt_counter(&total_combined, args);
        }
    }
}

/// Helper: print ID map as table or CSV.
fn print_id_map(map: &HashMap<String, u64>, args: &CliArgs) {
    let mut vec = map
        .iter()
        .map(|(id, &count)| (id.clone(), count))
        .collect::<Vec<_>>();
    // Sort descending by count, then lex by id
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    if args.csv {
        print_csv(&["Count", "Item ID"], vec.clone(), |(id, count)| {
            vec![count.to_string(), id.clone()]
        });
    } else {
        let mut table = new_table(&["Count", "Item ID"]);
        for (id, count) in vec {
            table.add_row(vec![Cell::new(count), Cell::new(id)]);
        }
        println!("{table}");
    }
}

/// Helper: print NBT counters as table or CSV.
fn print_nbt_counter(counter: &Counter, args: &CliArgs) {
    let mut by_nbt_vec = counter.total_by_nbt().into_iter().collect::<Vec<_>>();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    if args.csv {
        print_csv(&["Count", "NBT"], by_nbt_vec.clone(), |(nbt_opt, count)| {
            let nbt_str = nbt_opt
                .as_deref()
                .map(escape_nbt_string)
                .unwrap_or_else(|| "No NBT".into());
            vec![count.to_string(), nbt_str]
        });
    } else {
        let mut table = new_table(&["Count", "NBT"]);
        if let Some(col) = table.column_mut(1) {
            col.set_cell_alignment(CellAlignment::Left);
        }
        for (nbt_opt, count) in by_nbt_vec {
            let nbt_str = nbt_opt
                .as_deref()
                .map(escape_nbt_string)
                .unwrap_or_else(|| "No NBT".into());
            table.add_row(vec![Cell::new(count), Cell::new(nbt_str)]);
        }
        println!("{table}");
    }
}

/// Helper: display a detailed counter as table or CSV.
fn display_detailed(counter: &Counter, args: &CliArgs) {
    let mut by_id = counter.total_by_id().into_iter().collect::<Vec<_>>();
    by_id.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    if args.csv {
        print_csv(&["Count", "ID"], by_id.clone(), |(id, count)| {
            vec![count.to_string(), id.clone()]
        });
    } else {
        let mut table = new_table(&["Count", "ID"]);
        for (id, count) in by_id {
            table.add_row(vec![Cell::new(count), Cell::new(id)]);
        }
        println!("{table}");
    }
}

/// Helper: create a new formatted table with headers.
fn new_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|&h| Cell::new(h).set_alignment(CellAlignment::Center)),
        );
    table
}

/// Helper: print CSV rows using a closure to convert each element to strings.
fn print_csv<T, F>(headers: &[&str], rows: Vec<T>, mut row_to_fields: F)
where
    F: FnMut(&T) -> Vec<String>,
{
    let mut wtr = Writer::from_writer(std::io::stdout());
    wtr.write_record(headers)
        .expect("Failed to write CSV headers");
    for row in rows {
        let fields = row_to_fields(&row);
        wtr.write_record(fields).expect("Failed to write CSV row");
    }
    wtr.flush().expect("Failed to flush CSV writer");
}
