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
use walkdir::WalkDir;

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

macro_rules! let_paths {
    (base = $base_dir:expr, $(( $var:ident, $file:expr $(, create = $create:tt)? )),+ $(,)?) => {
        $(
            let $var = $base_dir.join($file);
            $(
                match $create {
                    "file" => create_files!(&$var),
                    "dir" => create_dirs!(&$var),
                    _ => compile_error!(concat!("Invalid input: ", stringify!($create))),
                }
            )?
        )+
    };
}

macro_rules! create_dirs {
    ($($dir:expr),+) => {{
        $(
            fs::create_dir_all($dir.clone()).expect("failed to create dirs");
        )+
    }};
}

macro_rules! create_files {
    ($($file:expr),+) => {{
        $(
            fs::File::create($file.clone()).expect("failed to create files");
        )+
    }};
}

macro_rules! create_symlinks {
    ($(($target:expr, $link:expr)),+) => {{
        $(
            std::os::unix::fs::symlink($target.clone(), $link.clone()).expect("failed to symlink");
        )+
    }};
}

macro_rules! create_tx_rx {
    () => {
        crossbeam_channel::unbounded::<Message>()
    };
}

macro_rules! create_config {
    ( $(( $name:expr, $(($watch:expr))+, $(($dest:expr))+, $regex:tt )),+ $(,)? ) => {
        Arc::new(Config {
            rules: vec![
                $(
                    Rule {
                        name: $name.into(),
                        watch: vec![
                            $(
                                $watch.clone()
                            )+
                        ],
                        dest: vec![
                            $(
                                $dest.clone()
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
            ..Config::default()
        })
    }
}

#[test]
fn run_env_test_1() -> anyhow::Result<()> {
    // init
    let (_, root) = create_test_env();
    let (tx, rx) = create_tx_rx!();

    // create dirs
    let_paths!(
        base = root,
        (watch_dir, "watch_dir"),
        (dest_dir, "dest_dir"),
    );
    create_dirs!(root, watch_dir, dest_dir);

    // create files
    let_paths!(
        base = watch_dir,
        // expect no action
        (file1, "file1.txt"),
        // expect init scan to symlink
        (file2, "_file2.txt"),
        // expect test hook to symlink
        (file3, "file3.txt"),
        (file3_renamed, "_file3.txt"),
        // expect init scan to take no action because already symlinked
        (file4, "_file4.txt"),
        // expect init scan to delete symlink since broken
        (file5_no_file, "file5.txt"),
    );
    create_files!(file1, file2, file3, file4);

    let_paths!(
        base = dest_dir,
        (file4_symlink, "_file4.txt"),
        (file5_broken_symlink, "_file5.txt"),
    );
    create_symlinks!(
        (file4, file4_symlink),
        (file5_no_file, file5_broken_symlink)
    );

    // define config
    let config = create_config!(("test1", (watch_dir), (dest_dir), "^_.*"));

    // test hook
    set_test_hook({
        let tx_clone = tx.clone();
        let file3 = file3.clone();
        let file3_renamed = file3_renamed.clone();
        move || {
            // rename file3 to file3_renamed
            fs::rename(file3.clone(), file3_renamed.clone()).expect("failed to rename file");

            // shutdown
            thread::sleep(Duration::from_millis(100));
            tx_clone
                .send(Message::Shutdown)
                .expect("failed to shutdown");
        }
    });

    // start the main process loop
    run_with_config(config, tx, rx)?;

    // verify fs
    // let file3_symlink = dest_dir.join(file3_renamed.file_name().unwrap());

    Ok(())
}
