use clap::Parser;
use mc_nbt_scanner::{
    DataType, ScanTask, Scope,
    cli::{CliArgs, ViewMode, parse_item_args},
    counter::CounterMap,
    list_mca_files, process_task,
    view::{view_by_id, view_by_nbt, view_detailed},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

fn main() {
    let args = CliArgs::parse();
    let queries = if args.all {
        Vec::new()
    } else {
        parse_item_args(&args.items)
    };

    let world_root = args.world_path.clone();
    let dimension_roots = get_all_dimension_roots(&world_root);
    if dimension_roots.is_empty() {
        eprintln!(
            "No dimension folders (region/ or entities/) found under {}",
            world_root.display()
        );
        return;
    }

    let tasks = create_scan_tasks(&dimension_roots);
    let start = Instant::now();

    let counter_map = tasks
        .into_par_iter()
        .map(|task| process_task(task, &queries, &args))
        .reduce(CounterMap::new, |mut a, b| {
            for (scope, counter) in b.iter() {
                a.merge_scope(scope.clone(), counter);
            }
            a
        });

    match args.view {
        ViewMode::Detailed => view_detailed(&counter_map, &args),
        ViewMode::ById => view_by_id(&counter_map, &args),
        ViewMode::ByNbt => view_by_nbt(&counter_map, &args),
    }

    if !args.csv {
        println!("\nTotal items matched: {}", counter_map.combined().total());
        println!("Scan completed in {:?}", start.elapsed());
    }
}

fn create_scan_tasks(dimension_roots: &[PathBuf]) -> Vec<ScanTask> {
    let mut tasks = Vec::new();
    for dim_path in dimension_roots {
        let dimension = dim_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        for (subfolder, data_type) in [
            ("region", DataType::BlockEntity),
            ("entities", DataType::Entity),
        ] {
            let folder_path = dim_path.join(subfolder);
            if folder_path.is_dir() {
                match list_mca_files(&folder_path) {
                    Ok(files) => {
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
                            subfolder,
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
                !path.ancestors().skip(1).any(is_dim_root)
            }
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir() && is_dim_root(entry.path()))
        .map(|entry| entry.into_path())
        .collect()
}
