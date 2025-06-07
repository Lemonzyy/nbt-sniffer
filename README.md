# Minecraft NBT Scanner (mc_nbt_scanner)

`mc_nbt_scanner` is a command-line tool to scan Minecraft Java Edition world data and count items, with support for NBT filtering.

## Features

- Scans items in block entities and regular entities from `.mca` files.
- Filters items by ID and/or NBT data (SNBT format).
- Multiple views: `detailed` (ID+NBT), `by-id`, `by-nbt`.
- Optional summaries: per-dimension, per-data-type, per-source (tree view).
- Output formats: Formatted tables (or in the future: JSON).
- Parallel processing for speed.

## Usage

```bash
mc_nbt_scanner <WORLD_PATH> --item <ITEM_ID[{NBT_DATA}]> [OPTIONS]
mc_nbt_scanner <WORLD_PATH> --all [OPTIONS]
```

### Key Options:

- `<WORLD_PATH>`: (Required) Path to the Minecraft world directory.
- `--all`: Count all items.
- `--item <ITEM_ID[{NBT_DATA}]>`: Specify item(s) to count (e.g., `minecraft:diamond`, `stone{a:1b}`).
- `--view <MODE>`: `by-id` (default), `by-nbt`, `detailed`.
- `--show-nbt`: Show full NBT in detailed view.
- `--per-source-summary`: Tree summary for items per source.
- `--per-dimension-summary`: Summary per dimension.
- `--per-data-type-summary`: Summary per data type (Block Entity, Entity).
- `-v, --verbose`: Verbose output.

### Examples

1.  **Count all diamonds:**

    ```bash
    mc_nbt_scanner /path/to/your/world --item minecraft:diamond
    ```

2.  **Find all netherite swords specifically named "My Sword":**

    ```bash
    mc_nbt_scanner /path/to/your/world --item 'minecraft:netherite_sword{components:{"minecraft:custom_name":"My Sword"}}'
    ```

3.  **Find all enchanted books with Mending:**
    ```bash
    mc_nbt_scanner /path/to/your/world --item 'minecraft:enchanted_book{components:{"minecraft:stored_enchantments":{"minecraft:mending":1}}}'
    ```
