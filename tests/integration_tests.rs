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

fn setup_test_filesystem(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root.join("watch_dir"))?;
    fs::create_dir_all(root.join("dest_dir"))?;
    fs::write(root.join("watch_dir/sample1.txt"), "")?;
    fs::write(root.join("watch_dir/_sample2.txt"), "")?;

    Ok(())
}

fn serialize_config(at_path: &Path, config: Config) -> anyhow::Result<()> {
    let yml_string = serde_yml::to_string(&config)?;
    let mut file = File::create(at_path)?;
    file.write_all(yml_string.as_bytes())?;

    Ok(())
}

#[test]
fn run_env_test_1() -> anyhow::Result<()> {
    let (temp_dir, test_root) = create_test_env();

    setup_test_filesystem(&test_root).context("failed to setup test filesystem")?;
    let watch_dir = test_root.join("watch_dir");
    let dest_dir = test_root.join("dest_dir");

    // create a custom config file via serializing
    let config_path = test_root.join("config.yml");

    let config_tmp = Config {
        rules: vec![Rule {
            name: "test 1".into(),
            watch: vec![test_root.join("watch_dir")],
            dest: vec![test_root.join("dest_dir")],
            regex: vec![Regex::new("^_.*").expect("failed to create regex")],
            ..Rule::default()
        }],
        ..Config::default()
    };

    serialize_config(&config_path, config_tmp)?;

    let args = Args { config_path };

    // before running the main program, start several separate threads:
    // - perform filesystem operations some seconds later
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));

        match fs::rename(
            watch_dir.join("sample1.txt"),
            watch_dir.join("_sample1.txt"),
        ) {
            Ok(()) => println!("File renamed!"),
            Err(e) => eprintln!("Failed to rename file: {}", e),
        }
    });
    // - terminate this process some more seconds later
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(4));
        std::process::exit(0); // TODO: have a proper way to shut down
    });

    // start the main process loop
    run_with_args(args)?;

    Ok(())
}
