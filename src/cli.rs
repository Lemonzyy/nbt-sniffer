use std::path::PathBuf;

use clap::{ArgGroup, Parser};

/// Count items in a Minecraft world (1.21.5+), with optional per-item NBT filters and coordinates
#[derive(Parser, Debug)]
#[command(
    group(ArgGroup::new("mode").args(["all", "items"]).required(true)),
    group(ArgGroup::new("view").args(["detailed", "by_id", "by_nbt"]))
)]
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

    /// List every distinct (ID, NBT)
    #[arg(long)]
    pub detailed: bool,

    /// Summarize counts by item ID
    #[arg(long)]
    pub by_id: bool,

    /// Summarize counts by NBT only
    #[arg(long)]
    pub by_nbt: bool,

    /// Also print each matching item's full NBT
    #[arg(long)]
    pub show_nbt: bool,

    /// Also output the coordinates of each matching item
    #[arg(long)]
    pub show_coords: bool,

    /// Increase output verbosity
    #[arg(short, long)]
    pub verbose: bool,
}
