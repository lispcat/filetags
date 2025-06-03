use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use filetags::{run_with_config, set_test_hook, Config, Message, Rule};
use regex::Regex;
use tempfile::TempDir;

fn create_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let test_path = temp_dir.path().to_path_buf();
    (temp_dir, test_path)
}

fn _serialize_config(at_path: &Path, config: Config) -> anyhow::Result<()> {
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

macro_rules! let_paths {
    (base = $base_dir:expr, $(($var:ident, $file:expr)),+ $(,)?) => {
        $(
            let $var = $base_dir.join($file);
        )+
    };
}

macro_rules! create_config {
    ($(($name:expr, $(($watch:expr))+, $(($dest:expr))+, $regex:tt)),+ $(,)?) => {
        Arc::new(Config {
            rules: vec![
                $(
                    Rule {
                        name: $name.into(),
                        watch: vec![
                            $(
                                $watch
                            )+
                        ],
                        dest: vec![
                            $(
                                $dest
                            )+
                        ],
                        regex: vec![
                            Regex::new($regex)
                            .expect("failed to create regex")
                        ],
                        ..Rule::default()
                    }
                )+
            ],
            ..Config::default()
        })
    }
}

macro_rules! create_tx_rx {
    () => {
        crossbeam_channel::unbounded::<Message>()
    };
}

#[test]
fn run_env_test_1() -> anyhow::Result<()> {
    // create temp testing env
    let (_, root) = create_test_env();

    // create dirs
    let_paths!(
        base = root,
        (watch_dir, "watch_dir"),
        (dest_dir, "dest_dir")
    );
    create_dirs!(&root, &watch_dir, &dest_dir);

    // create files
    let_paths!(
        base = watch_dir,
        (file1, "_sample1.txt"),
        (file2, "sample2.txt"),
        (file3, "sample3.txt")
    );
    create_files!(&file1, &file2, &file3);

    // for future reference...
    let_paths!(base = watch_dir, (file2_renamed, "_sample2.txt"));

    // define config
    let config = create_config!(("test1", (watch_dir), (dest_dir), "^_.*"));

    // create channel
    let (tx, rx) = create_tx_rx!();

    // adding test hook
    let tx_clone = tx.clone();
    set_test_hook(move || {
        match fs::rename(file2.clone(), file2_renamed.clone()) {
            Ok(()) => println!("File renamed!"),
            Err(e) => eprintln!("Failed to rename file: {}", e),
        }
        thread::sleep(Duration::from_millis(100));
        tx_clone
            .send(Message::Shutdown)
            .expect("failed to shutdown");
    });

    // start the main process loop
    run_with_config(config, tx, rx)?;

    Ok(())
}
