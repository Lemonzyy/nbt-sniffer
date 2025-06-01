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

fn print_summaries<F>(data: &AggregatedData, args: &CliArgs, mut print_fn: F)
where
    F: FnMut(&Counter, &str),
{
    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => print_fn(&data.total_combined, "Total"),
        (true, false) => {
            for dimension in data.grouped.keys() {
                let combined = data.dimension_combined(dimension);
                print_fn(&combined, &format!("Dimension: {dimension}"));
            }
            print_fn(&data.total_combined, "Total");
        }
        (false, true) => {
            print_fn(&data.total_block_entity, "Block Entity");
            print_fn(&data.total_entity, "Entity");
            print_fn(&data.total_combined, "Total");
        }
        (true, true) => {
            for (dimension, types_map) in &data.grouped {
                println!("\nDimension: {dimension}");

                if let Some(counter) = types_map.get(&DataType::BlockEntity) {
                    print_fn(counter, "Block Entity");
                }
                if let Some(counter) = types_map.get(&DataType::Entity) {
                    print_fn(counter, "Entity");
                }

                let combined = data.dimension_combined(dimension);
                print_fn(&combined, "Summary");
            }

            print_fn(&data.total_block_entity, "\nBlock Entity");
            print_fn(&data.total_entity, "Entity");
            print_fn(&data.total_combined, "Total");
        }
    }
}

pub fn view_detailed(counter_map: &CounterMap, args: &CliArgs) {
    let data = AggregatedData::new(counter_map);
    print_summaries(&data, args, |counter, label| {
        if !label.starts_with('\n') {
            println!("{label}:");
        } else {
            println!("{}:", &label[1..]);
        }
        print_detailed_counter(counter, args);
    });
}

pub fn view_by_nbt(counter_map: &CounterMap, args: &CliArgs) {
    let data = AggregatedData::new(counter_map);

    if args.per_dimension_summary && !args.per_data_type_summary {
        println!("== Per-dimension summary ==");
    } else if !args.per_dimension_summary && args.per_data_type_summary {
        println!("== Per-data-type summary ==");
    } else if args.per_dimension_summary && args.per_data_type_summary {
        println!("== Both per-dimension and per-data-type summary ==");
    }

    print_summaries(&data, args, |counter, label| {
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
        print_nbt_counter(counter, args);
    });
}

pub fn view_by_id(counter_map: &CounterMap, args: &CliArgs) {
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
            *total_combined.entry(id.clone()).or_default() += count;

            match scope.data_type {
                DataType::BlockEntity => {
                    *total_block_entity.entry(id.clone()).or_default() += count
                }
                DataType::Entity => *total_entity.entry(id.clone()).or_default() += count,
            }
        }
    }

    match (args.per_dimension_summary, args.per_data_type_summary) {
        (false, false) => {
            println!("Total:");
            print_id_map(&total_combined, args);
        }
        (true, false) => {
            for (dimension, types_map) in &grouped {
                println!("\nDimension: {dimension}");
                let mut combined: HashMap<String, u64> = HashMap::new();
                for map in types_map.values() {
                    for (id, count) in map {
                        *combined.entry(id.clone()).or_default() += count;
                    }
                }
                println!("Summary:");
                print_id_map(&combined, args);
            }
            println!("\nTotal:");
            print_id_map(&total_combined, args);
        }
        (false, true) => {
            println!("Block Entity:");
            print_id_map(&total_block_entity, args);
            println!("Entity:");
            print_id_map(&total_entity, args);
            println!("Total:");
            print_id_map(&total_combined, args);
        }
        (true, true) => {
            for (dimension, types_map) in &grouped {
                println!("\nDimension: {dimension}");

                for data_type in &[DataType::BlockEntity, DataType::Entity] {
                    if let Some(id_map) = types_map.get(data_type) {
                        println!("{data_type}:");
                        print_id_map(id_map, args);
                    }
                }

                let mut combined: HashMap<String, u64> = HashMap::new();
                for map in types_map.values() {
                    for (id, count) in map {
                        *combined.entry(id.clone()).or_default() += count;
                    }
                }
                println!("Summary:");
                print_id_map(&combined, args);
            }

            println!("\nBlock Entity:");
            print_id_map(&total_block_entity, args);
            println!("Entity:");
            print_id_map(&total_entity, args);
            println!("Total:");
            print_id_map(&total_combined, args);
        }
    }
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
