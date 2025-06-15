use std::path::PathBuf;

use clap::{ArgGroup, Parser, ValueEnum};
use valence_nbt::Value;

/// Count items in a Minecraft world, with optional per-item NBT filters and coordinates
#[derive(Parser, Debug)]
#[command(group(ArgGroup::new("mode").args(["all", "items"]).required(true)))]
pub struct CliArgs {
    #[arg(short, long, value_name = "PATH")]
    pub world_path: PathBuf,

    /// Count all items
    #[arg(long, group = "mode")]
    pub all: bool,

    /// Specify items to count
    #[arg(
        short,
        long = "item",
        value_name = "ITEM",
        group = "mode",
        num_args = 1..,
        long_help = "Specify items to count, each in the form: ITEM_ID{nbt}\n\nExamples:\n\n--item minecraft:diamond\n--item minecraft:shulker_box{components:{\"minecraft:item_name\":\"Portable Chest\"}}"
    )]
    pub items: Vec<String>,

    /// Which summary format to display.
    #[arg(short, long, value_enum, default_value_t = ViewMode::ById)]
    pub view: ViewMode,

    /// Show full NBT data in item summaries
    #[arg(long)]
    pub show_nbt: bool,

    /// Show a tree summary per source
    #[arg(long)]
    pub per_source_summary: bool,

    /// Show a summary per dimension in addition to the total counts across all dimensions
    #[arg(long)]
    pub per_dimension_summary: bool,

    /// Show a summary per data type in addition to the total counts across all dimensions
    #[arg(long)]
    pub per_data_type_summary: bool,

    /// Increase output verbosity
    #[arg(long)]
    pub verbose: bool,

    /// Specify the output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Which summary‐format to display.
#[derive(Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum ViewMode {
    /// List every distinct (ID, NBT) combination
    Detailed,

    /// Summarize counts by item ID
    ById,

    /// Summarize counts by NBT only
    ByNbt,
}

/// Which output format to use for the summary tables.
#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    PrettyJson,
}

impl OutputFormat {
    pub fn is_json(&self) -> bool {
        matches!(self, OutputFormat::Json | OutputFormat::PrettyJson)
    }
}

/// Represents a query for an item and its optional NBT filters
#[derive(Debug)]
pub struct ItemFilter {
    pub id: Option<String>,
    pub required_nbt: Option<Value>,
}

/// Parse raw CLI `item` arguments into `ItemFilter` structs
/// Each entry is of form `ITEM_ID{nbt}`
pub fn parse_item_args(raw_items: &[String]) -> Vec<ItemFilter> {
    raw_items
        .iter()
        .map(|entry| {
            let mut id_str = entry.as_str();
            let mut nbt_query = None;

            if let Some(start) = entry.find('{')
                && let Some(end) = entry.rfind('}')
            {
                id_str = &entry[..start];
                let nbt_str = &entry[start..=end];
                if !nbt_str.is_empty() {
                    match valence_nbt::snbt::from_snbt_str(nbt_str) {
                        Ok(parsed) => nbt_query = Some(parsed),
                        Err(e) => eprintln!("Failed to parse SNBT '{nbt_str}': {e}"),
                    }
                }
            }

            let id = if id_str.is_empty() {
                None
            } else if id_str.contains(':') {
                Some(id_str.to_string())
            } else {
                Some(format!("minecraft:{id_str}"))
            };

            ItemFilter {
                id,
                required_nbt: nbt_query,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence_nbt::compound;

    #[test]
    fn test_parse_item_args_simple_id() {
        let args = vec!["diamond".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].id, Some("minecraft:diamond".to_string()));
        assert!(filters[0].required_nbt.is_none());
    }

    #[test]
    fn test_parse_item_args_namespaced_id() {
        let args = vec!["custom:item".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].id, Some("custom:item".to_string()));
        assert!(filters[0].required_nbt.is_none());
    }

    #[test]
    fn test_parse_item_args_id_with_simple_nbt() {
        let args = vec!["stone{a:1b}".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].id, Some("minecraft:stone".to_string()));
        assert_eq!(
            filters[0].required_nbt,
            Some(compound! { "a" => 1i8 }.into())
        );
    }

    #[test]
    fn test_parse_item_args_id_with_complex_nbt() {
        let args = vec!["shulker_box{components:{\"minecraft:container\":[{slot:0b,item:{id:\"minecraft:diamond\",count:1b}}]}}".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].id, Some("minecraft:shulker_box".to_string()));
        let expected_nbt = valence_nbt::snbt::from_snbt_str("{components:{\"minecraft:container\":[{slot:0b,item:{id:\"minecraft:diamond\",count:1b}}]}}").unwrap();
        assert_eq!(filters[0].required_nbt, Some(expected_nbt));
    }

    #[test]
    fn test_parse_item_args_nbt_only() {
        let args = vec!["{components:{\"minecraft:custom_name\":\"Special\"}}".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert!(filters[0].id.is_none());
        let expected_nbt = valence_nbt::snbt::from_snbt_str(
            "{components:{\"minecraft:custom_name\":\"Special\"}}",
        )
        .unwrap();
        assert_eq!(filters[0].required_nbt, Some(expected_nbt));
    }

    #[test]
    fn test_parse_item_args_invalid_nbt_string() {
        // This test relies on eprintln! for error indication, actual behavior is that NBT is None
        let args = vec!["iron_ingot{invalid_nbt:}".to_string()];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].id, Some("minecraft:iron_ingot".to_string()));
        assert!(
            filters[0].required_nbt.is_none(),
            "NBT should be None for invalid SNBT"
        );
    }

    #[test]
    fn test_parse_item_args_multiple_items() {
        let args = vec![
            "diamond".to_string(),
            "gold_ingot{components:{\"minecraft:custom_data\":{foo:\"bar\"}}}".to_string(),
        ];
        let filters = parse_item_args(&args);
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].id, Some("minecraft:diamond".to_string()));
        assert!(filters[0].required_nbt.is_none());
        assert_eq!(filters[1].id, Some("minecraft:gold_ingot".to_string()));
        let expected_nbt_for_gold = valence_nbt::snbt::from_snbt_str(
            "{components:{\"minecraft:custom_data\":{foo:\"bar\"}}}",
        )
        .unwrap();
        assert_eq!(filters[1].required_nbt, Some(expected_nbt_for_gold));
    }
}
