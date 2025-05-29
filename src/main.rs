use std::path::PathBuf;

use clap::{ArgGroup, Parser};

/// Count items in a Minecraft world (1.21.5+), with optional per-item NBT filters and coordinates
#[derive(Parser, Debug)]
#[command(group(ArgGroup::new("mode").required(true).args(["all", "items"])))]
struct Args {
    #[arg(short, long, value_name = "PATH")]
    world_path: PathBuf,

    /// Count all items
    #[arg(long, group = "mode")]
    all: bool,

    /// Specify items to count
    #[arg(
        long = "item",
        value_name = "ITEM_QUERY",
        group = "mode",
        num_args = 1..,
        long_help = "Specify items to count, each in the form: ITEM_ID[:key=value,...]\n\nExamples:\n\n--item minecraft:diamond\n--item minecraft:shulker_box:CustomName=MyBox,Lock=secret"
    )]
    items: Vec<String>,

    /// Also output the coordinates of each matching item
    #[arg(long)]
    with_coords: bool,
}

fn main() {
    let args = Args::parse();

    println!("{args:?}");
}
