use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::Context;
use filetags::{run_with_args, Args, Config, Rule};
use regex::Regex;
use tempfile::TempDir;

fn create_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let test_path = temp_dir.path().to_path_buf();
    (temp_dir, test_path)
}

fn serialize_config(at_path: &Path, config: Config) -> anyhow::Result<()> {
    let yml_string = serde_yml::to_string(&config)?;
    let mut file = File::create(at_path)?;
    file.write_all(yml_string.as_bytes())?;

    Ok(())
}

macro_rules! create_dirs {
    ($($dir:expr),+) => {{
        $(
            fs::create_dir_all($dir)?;
        )+
    }};
}

macro_rules! create_files {
    ($($file:expr),+) => {{
        $(
            fs::File::create($file)?;
        )+
    }};
}

#[test]
fn run_env_test_1() -> anyhow::Result<()> {
    // create temp testing env
    let (_, test_root) = create_test_env();

    // create dirs
    let watch_dir = test_root.join("watch_dir");
    let dest_dir = test_root.join("dest_dir");
    create_dirs!(&test_root, &watch_dir, &dest_dir);

    // create files
    let file1 = watch_dir.join("_sample1.txt");
    let file2 = watch_dir.join("sample2.txt");
    let file3 = watch_dir.join("sample3.txt");
    create_files!(&file1, &file2, &file3);

    // for future reference...
    let file2_renamed = watch_dir.join("_sample2.txt");
    let config_path = test_root.join("config.yml");

    // create a custom config via serializing
    serialize_config(
        &config_path,
        Config {
            rules: vec![Rule {
                name: "test 1".into(),
                watch: vec![watch_dir],
                dest: vec![dest_dir],
                regex: vec![Regex::new("^_.*").expect("failed to create regex")],
                ..Rule::default()
            }],
            ..Config::default()
        },
    )?;

    // create Args using config_path
    let args = Args { config_path };

    // before running the main program, start several separate threads...

    // - perform filesystem operations some seconds later
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));

        match fs::rename(file2, file2_renamed) {
            Ok(()) => println!("File renamed!"),
            Err(e) => eprintln!("Failed to rename file: {}", e),
        }
    });

    // - terminate this whole program some more seconds later
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));
        std::process::exit(0); // TODO: have a proper way to shut down
    });

    // start the main process loop
    run_with_args(args)?;

    Ok(())
}
