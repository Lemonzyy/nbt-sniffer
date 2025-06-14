use clap::Parser;
use nbt_sniffer::{
    DataType, ScanTask, Scope,
    cli::{CliArgs, OutputFormat, ViewMode, parse_item_args},
    counter::CounterMap,
    extract_single_player_uuid_from_level_dat, list_mca_files, process_task,
    view::{aggregation::IsEmpty, view_by_id, view_by_nbt, view_detailed},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

#[derive(Deserialize, Debug)]
struct UserCacheEntry {
    name: String,
    uuid: String, // e.g. "f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
}

fn load_user_cache(world_root: &Path, cli_args: &CliArgs) -> HashMap<String, String> {
    let usercache_path = world_root.join("usercache.json");
    let mut uuid_to_name = HashMap::new();

    if let Ok(contents) = fs::read_to_string(usercache_path) {
        if let Ok(entries) = serde_json::from_str::<Vec<UserCacheEntry>>(&contents) {
            for entry in entries {
                uuid_to_name.insert(entry.uuid.to_lowercase(), entry.name);
            }
        } else if cli_args.verbose {
            eprintln!(
                "Warning: Failed to parse usercache.json. Player names might not be available for .dat files."
            );
        }
    } else if cli_args.verbose {
        eprintln!(
            "Warning: usercache.json not found. Player names might not be available for .dat files."
        );
    }
    uuid_to_name
}

fn main() {
    let args = CliArgs::parse();
    let queries = if args.all {
        Vec::new()
    } else {
        parse_item_args(&args.items)
    };

    let world_root = args.world_path.clone();
    let dimension_roots = get_all_dimension_roots(&world_root);
    let user_cache = load_user_cache(&world_root, &args);

    if dimension_roots.is_empty() && args.verbose {
        eprintln!(
            "No dimension folders (containing region/ or entities/) found under {}. Will still attempt to scan for player data.",
            world_root.display()
        );
    }

    let mut tasks = create_mca_scan_tasks(&dimension_roots, &args);
    let player_tasks = create_player_scan_tasks(&world_root, &dimension_roots, &args);
    tasks.extend(player_tasks);

    if tasks.is_empty() {
        eprintln!(
            "No scannable data (region/entities files, player data, or level.dat) found in {}. Nothing to do.",
            world_root.display()
        );
        return;
    }

    if args.verbose {
        eprintln!("Total scan tasks created: {}", tasks.len());
    }

    let start = Instant::now();
    let counter_map = tasks
        .into_par_iter()
        .map(|task| process_task(task, &queries, &args, &user_cache))
        .reduce(CounterMap::new, |mut a, b| {
            for (scope, counter) in b.iter() {
                a.merge_scope(scope.clone(), counter);
            }
            a
        });

    if counter_map.is_empty() {
        if queries.is_empty() || args.all {
            eprintln!(
                "No items found during scan. The world might be empty or data files unreadable."
            );
        } else {
            eprintln!("No items matched your query.");
        }
    }

    match args.view {
        ViewMode::Detailed => view_detailed(&counter_map, &args),
        ViewMode::ById => view_by_id(&counter_map, &args),
        ViewMode::ByNbt => view_by_nbt(&counter_map, &args),
    }

    if args.output_format == OutputFormat::Table && !counter_map.is_empty() {
        println!("\nTotal items matched: {}", counter_map.combined().total());
        println!("Scan completed in {:?}", start.elapsed());
    }
}

const DIMENSION_SUBFOLDER_MAPPINGS: [(&str, DataType); 2] = [
    ("region", DataType::BlockEntity),
    ("entities", DataType::Entity),
];

fn create_mca_scan_tasks(dimension_roots: &[PathBuf], cli_args: &CliArgs) -> Vec<ScanTask> {
    let mut tasks = Vec::new();
    for dim_path in dimension_roots {
        let dimension = dim_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        for (subfolder_name, data_type) in DIMENSION_SUBFOLDER_MAPPINGS {
            let folder_path = dim_path.join(subfolder_name);
            if folder_path.is_dir() {
                match list_mca_files(&folder_path) {
                    Ok(files) => {
                        if cli_args.verbose && !files.is_empty() {
                            eprintln!(
                                "Added {} MCA scan tasks for {} in dimension {}",
                                files.len(),
                                subfolder_name,
                                dimension
                            );
                        }
                        for file in files {
                            tasks.push(ScanTask {
                                path: file,
                                scope: Scope {
                                    dimension: dimension.clone(),
                                    data_type: data_type.clone(),
                                },
                            });
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Error reading {} folder '{}': {err}",
                            subfolder_name,
                            folder_path.display()
                        );
                        continue;
                    }
                }
            }
        }
    }
    tasks
}

fn create_player_scan_tasks(
    world_root: &Path,
    dimension_roots: &[PathBuf],
    cli_args: &CliArgs,
) -> Vec<ScanTask> {
    let mut tasks = Vec::new();
    let mut single_player_uuid_from_level_dat = None;

    let level_dat_path = world_root.join("level.dat");
    if level_dat_path.is_file() {
        if cli_args.verbose {
            eprintln!("Adding level.dat scan task: {}", level_dat_path.display());
        }
        tasks.push(ScanTask {
            path: level_dat_path.clone(),
            scope: Scope {
                dimension: "level".to_string(),
                data_type: DataType::Player,
            },
        });
        single_player_uuid_from_level_dat =
            extract_single_player_uuid_from_level_dat(&level_dat_path, cli_args);
        if cli_args.verbose
            && let Some(ref uuid) = single_player_uuid_from_level_dat
        {
            eprintln!("Successfully extracted single-player UUID from level.dat: {uuid}",);
        }
    }

    let mut potential_playerdata_parents = dimension_roots.to_vec();
    if !potential_playerdata_parents.contains(&world_root.to_path_buf()) {
        potential_playerdata_parents.push(world_root.to_path_buf());
    }
    potential_playerdata_parents.sort();
    potential_playerdata_parents.dedup();

    for parent_dir_for_playerdata in &potential_playerdata_parents {
        let playerdata_path = parent_dir_for_playerdata.join("playerdata");
        if playerdata_path.is_dir() {
            if cli_args.verbose {
                eprintln!("Scanning for player data in {}", playerdata_path.display());
            }
            match std::fs::read_dir(&playerdata_path) {
                Ok(entries) => {
                    for entry_res in entries {
                        match entry_res {
                            Ok(entry) => {
                                let path = entry.path();
                                if path.is_file()
                                    && path.extension().and_then(|e| e.to_str()) == Some("dat")
                                {
                                    let file_stem_uuid_str =
                                        path.file_stem().and_then(|s| s.to_str());

                                    if let Some(ref sp_uuid) = single_player_uuid_from_level_dat
                                        && file_stem_uuid_str
                                            .is_some_and(|s| s.eq_ignore_ascii_case(sp_uuid))
                                    {
                                        if cli_args.verbose {
                                            eprintln!(
                                                "Skipping playerdata file {} as it's overridden by level.dat (player UUID: {sp_uuid})",
                                                path.display(),
                                            );
                                        }
                                        continue;
                                    }
                                    let dimension_name_for_scope = parent_dir_for_playerdata
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("world");
                                    if cli_args.verbose {
                                        eprintln!(
                                            "Adding playerdata scan task: {} (Scope: {dimension_name_for_scope}/playerdata)",
                                            path.display(),
                                        );
                                    }
                                    tasks.push(ScanTask {
                                        path,
                                        scope: Scope {
                                            dimension: format!(
                                                "{dimension_name_for_scope}/playerdata",
                                            ),
                                            data_type: DataType::Player,
                                        },
                                    });
                                }
                            }
                            Err(e) => {
                                if cli_args.verbose {
                                    eprintln!(
                                        "Warning: failed to read an entry in '{}': {e}",
                                        playerdata_path.display(),
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    if cli_args.verbose {
                        eprintln!(
                            "Warning: failed to read playerdata directory '{}': {e}",
                            playerdata_path.display(),
                        );
                    }
                }
            }
        }
    }
    tasks
}

fn is_dim_root(dir: &Path) -> bool {
    dir.join("region").is_dir() || dir.join("entities").is_dir()
}

pub fn get_all_dimension_roots(world_root: &Path) -> Vec<PathBuf> {
    WalkDir::new(world_root)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            if is_dim_root(path) {
                true
            } else {
                // Only descend into directories that are not themselves dimension roots
                // if none of their parents were dimension roots.
                // This prevents exploring deep into an already identified dimension.
                // The `world_root` itself is an exception if it's not a dim_root.
                if path == world_root {
                    return true;
                }
                !path
                    .ancestors()
                    .skip(1)
                    .any(|p| p != world_root && is_dim_root(p))
            }
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir() && is_dim_root(entry.path()))
        .map(|entry| entry.into_path())
        .collect()
}
