<div align="center">

  <h1>NBT Sniffer</h1>

  <img src="https://raw.githubusercontent.com/Lemonzyy/nbt-sniffer/refs/heads/main/assets/sniffer_sniffing.gif" alt="NBT Sniffer Logo - A Minecraft Sniffer sniffing" width="20%">
  <br/>

  <p><em>Ever wondered what treasures lie hidden in your Minecraft world's NBT data? Let NBT Sniffer help you unearth them!</em></p>
</div>
<br/>

`nbt-sniffer` is a command-line tool that sniffs through Minecraft Java Edition[^version_note] world data. It locates and counts items across your world and player files, with powerful NBT-based filters so you can sniff out exactly what you want.

## Features

- Scans items in block entities and regular entities from `.mca` files.
- Scans player data from `.dat` files (including `level.dat` for single-player worlds).
- Filters items by ID and/or NBT data (SNBT[^snbt] format).
- Multiple views: `detailed` (ID+NBT), `by-id`, `by-nbt`.
- Optional summaries: per-dimension, per-data-type, per-source (tree view).
- Output formats: Formatted tables, JSON, and pretty JSON.
- Parallel processing for efficient scanning of large worlds.
- User-friendly player UUID to name resolution using `usercache.json`.

## Usage

```bash
nbt-sniffer --world-path <WORLD_PATH> --item <ITEM_ID[{NBT_DATA}]> [OPTIONS]
nbt-sniffer --world-path <WORLD_PATH> --all [OPTIONS]
```

### Key Options:

- `-w, --world-path <WORLD_PATH>`: (Required) Path to the Minecraft world directory.
- `--all`: Scan for all items.
- `-i, --item <ITEM_ID[{NBT_DATA}]>`: Specify item(s) to scan for (e.g., `minecraft:diamond`, `'minecraft:stone{components:{"minecraft:custom_data":{some_tag:1b}}}'`).
- `-v, --view <MODE>`: Set the output view. Options: `by-id` (default), `by-nbt`, `detailed`.
- `--show-nbt`: When `--per-source-summary` is active, this flag includes the NBT data for each item within the generated tree view. It does not affect other views.
- `--per-source-summary`: Display a tree summary showing where items are found.
- `--per-dimension-summary`: Display a summary of items found per dimension.
- `--per-data-type-summary`: Display a summary of items per data type (Block Entity, Entity, Player).
- `-f, --format <FORMAT>`: Specify the output format. Options: `table` (default), `json`, `pretty-json`.
- `--verbose`: Enable verbose output for more detailed logging.

### Examples

1.  **Count all diamonds in your world:**

    ```bash
    nbt-sniffer --world-path /path/to/your/world --item minecraft:diamond
    ```

2.  **Find all netherite swords specifically named "My Awesome Sword":**

    ```bash
    nbt-sniffer --world-path /path/to/your/world --item 'minecraft:netherite_sword{components:{"minecraft:custom_name":"My Awesome Sword"}}'
    ```

    _Note: SNBT often requires careful quoting, especially for custom names represented as JSON strings within NBT._

3.  **Find all enchanted books with the Mending enchantment:**

    ```bash
    nbt-sniffer --world-path /path/to/your/world --item 'minecraft:enchanted_book{components:{"minecraft:stored_enchantments":{"minecraft:mending":1}}}'
    ```

4.  **Count all items and output as pretty-printed JSON:**

    ```bash
    nbt-sniffer --world-path /path/to/your/world --all --format pretty-json
    ```

---

Happy sniffing!

---

<div align="center">
  <small><em>Minecraft is a trademark of Mojang Synergies AB. The Sniffer mob image/GIF is property of Mojang Synergies AB. This project is not affiliated with or endorsed by Mojang Synergies AB.</em></small>
</div>

[^version_note]: This tool is primarily tested and intended for recent versions of Minecraft Java Edition, specifically focusing on 1.21.5 due to potential NBT format changes in item data across different game versions. Functionality with other versions is not guaranteed.
[^snbt]: Stringified NBT format
