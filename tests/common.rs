use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use itertools::Itertools;
use tempfile::TempDir;
use tracing::debug;
use walkdir::WalkDir;

pub fn create_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let test_path = temp_dir.path().to_path_buf();
    (temp_dir, test_path)
}

// pub fn _serialize_config(at_path: &Path, config: Config) -> anyhow::Result<()> {
//     let yml_string = serde_yml::to_string(&config)?;
//     let mut file = File::create(at_path)?;
//     file.write_all(yml_string.as_bytes())?;

//     Ok(())
// }

#[macro_export]
macro_rules! let_paths {
    ($(( $var:ident = $base:tt / $file:tt $(: create = $create:literal $(=> $target:tt)?)? )),+ $(,)?) => {
        $(
            let $var = $base.join($file);
            $(
                match $create {
                    "f" => create_files!($var.clone()),
                    "dir" => create_dirs!($var.clone()),
                    "no" => (),
                    "symlink" => {
                        $(
                            create_symlinks!(($target.clone(), $var.clone()));
                        )?
                    },
                    _ => panic!("Invalid create type: {}", $create),
                }
            )?
        )+
    };
}

#[macro_export]
macro_rules! create_dirs {
    ($($dir:expr),+) => {{
        $(
            fs::create_dir_all($dir.clone()).expect("failed to create dirs");
        )+
    }};
}

#[macro_export]
macro_rules! create_files {
    ($($file:expr),+) => {{
        $(
            fs::File::create($file.clone()).expect("failed to create files");
        )+
    }};
}

#[macro_export]
macro_rules! create_symlinks {
    ($(($target:expr, $link:expr)),+) => {{
        $(
            std::os::unix::fs::symlink($target.clone(), $link.clone()).expect("failed to symlink");
        )+
    }};
}

#[macro_export]
macro_rules! create_tx_rx {
    () => {
        crossbeam_channel::unbounded::<Message>()
    };
}

#[macro_export]
macro_rules! create_config {
    ( $( ($name:expr, $(($watch:expr))+, $(($link:expr))+, $regex:tt) ),+ $(,)? ) => {{
        let raw_config = filetags::RawConfig {
            rules: vec![
                $(
                    Rule {
                        name: $name.into(),
                        watch_dirs: vec![
                            $(
                                $watch.clone()
                            )+
                        ],
                        link_dirs: vec![
                            $(
                                $link.clone()
                            )+
                        ],
                        regex: vec![
                            Regex::new($regex.clone())
                            .expect("failed to create regex")
                        ],
                        ..Rule::default()
                    }
                )+
            ],
            ..filetags::RawConfig::default()
        };
        let serialized_str = serde_yml::to_string(&raw_config).unwrap();
        let deserialized_config: Config = serde_yml::from_str(&serialized_str).unwrap();
        Arc::new(deserialized_config)
    }}
}

pub fn rename_file(orig: &Path, new: &Path) {
    // rename file3 to file3_renamed
    std::fs::rename(orig, new).expect("failed to rename file");
}

pub fn collect_tree(root: &Path) -> HashSet<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .map(|e| {
            e.unwrap()
                .path()
                .strip_prefix(root)
                .expect("failed to strip prefix")
                .to_path_buf()
        })
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<HashSet<PathBuf>>()
}

pub fn create_exp_tree(paths: Vec<&str>) -> HashSet<PathBuf> {
    paths.iter().map(|&s| PathBuf::from(s)).collect()
}

pub fn sort_hashset(set: &HashSet<PathBuf>) -> Vec<&PathBuf> {
    set.iter().sorted_by(|a, b| a.cmp(b)).collect::<Vec<_>>()
}

pub fn assert_cur_and_exp_trees_eq(root: &Path, paths: Vec<&str>) {
    let tree = collect_tree(root);
    debug!("Sorted hashset real: {:?}", sort_hashset(&tree));

    let expected_tree = create_exp_tree(paths);
    debug!("Sorted hashset expe: {:?}", sort_hashset(&expected_tree));

    assert_eq!(tree, expected_tree);
}
