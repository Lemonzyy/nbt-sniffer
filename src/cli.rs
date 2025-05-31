use std::path::PathBuf;

use clap::{ArgGroup, Parser, ValueEnum};

/// Count items in a Minecraft world (1.21.5+), with optional per-item NBT filters and coordinates
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
    #[arg(long, value_enum, default_value_t = ViewMode::ById)]
    pub view: ViewMode,

    /// Output raw CSV instead of a formatted table
    #[arg(long, conflicts_with = "per_source_summary")]
    pub csv: bool,

    /// Show full NBT data in item summaries
    #[arg(long)]
    pub show_nbt: bool,

    /// Show a tree summary per source
    #[arg(long, conflicts_with = "csv")]
    pub per_source_summary: bool,

    /// Increase output verbosity
    #[arg(short, long)]
    pub verbose: bool,
}

/// Which summary‐format to display.
#[derive(Clone, Debug, ValueEnum)]
pub enum ViewMode {
    /// List every distinct (ID, NBT) combination
    Detailed,

    /// Summarize counts by item ID
    ById,

    /// Summarize counts by NBT only
    ByNbt,
}
