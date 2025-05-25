use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
};

use anyhow::Context;
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};
use regex::Regex;
use smart_default::SmartDefault;

#[derive(SmartDefault, Debug)]
struct Config {
    #[default(true)]
    create_missing_directories: bool,

    #[default("/home/sui/Music/prod/Samples/")]
    watch_dir: PathBuf,

    #[default("/home/sui/Music/prod/Samples/Favorites")]
    dest_dir: PathBuf,

    #[default(Regex::new(r"^_.*$").expect("failed to create default regex"))]
    pattern: Regex,
}

// TODO:
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

fn main() -> anyhow::Result<()> {
    // init ///////////////////////////////////////////////////////////////////

    let config = Config::default();

    init_dirs(&config)?;

    // TODO: first scan ///////////////////////////////////////////////////////
    // - delete all broken symlinks in dest_dir (later, run this every config.clean_interval).
    // - scan all files recursively under watch_dir, create symlinks as appropriate.
    //   - if symlink already exists at Dest,
    //     - if points to orig file, do nothing.
    //     - if points to another file, TODO process error appropriately.

    // set up watchers ////////////////////////////////////////////////////////

    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(tx)?;

    watcher.watch(&config.watch_dir, RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(event) => handle_event(&config, &event)?,
            Err(e) => println!("> Watch Error: {:?}", e),
        }
    }
    Ok(())
}

/// To be run at startup.
/// Initialize directories and catch errors early to prevent mild catastrophes.
fn init_dirs(config: &Config) -> anyhow::Result<()> {
    let path = &config.dest_dir;
    if !path.try_exists()? {
        eprintln!("PATH DOES NOT EXIST ({:?})", path);
        if config.create_missing_directories {
            println!("Creating path: {:?}", path);
            fs::create_dir_all(path)
                .with_context(|| format!("failed to create symlink directory: {:?}", path))?;
            println!("Created path at: {:?}", path);
        } else {
            anyhow::bail!("path does not exist! terminating...");
        }
    }
    Ok(())
}

/// Handle an event thrown by NotifyWatcher.
/// Depending on the kind of event that's thrown, it may run `handle_path`.
fn handle_event(config: &Config, event: &Event) -> anyhow::Result<()> {
    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in &event.paths {
                handle_path(config, path).context("failed to handle path")?;
            }
        }
        EventKind::Create(_) => {
            for path in &event.paths {
                handle_path(config, path).context("failed to handle path")?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Check if the filename of the path matches the Regex, and if so, create symlink.
fn handle_path(config: &Config, path: &Path) -> anyhow::Result<()> {
    let filename = path
        .file_name()
        .with_context(|| format!("cannot get OsStr filename of path: {:?}", path))?
        .to_str()
        .with_context(|| format!("cannot convert OsStr to str for path: {:?}", path))?;

    if config.pattern.is_match(filename) {
        eprintln!("REGEX MATCHES!: {}", filename);
    } else {
        eprintln!("Regex does not match: {}", filename);
    }
    Ok(())
}

/// Creates a symlink from orig_path to dest_dir.
fn create_symlink(orig_path: &Path, dest_dir: &Path) -> anyhow::Result<()> {
    eprintln!(
        "Creating symlink from ({:?}) to ({:?})...",
        orig_path, dest_dir
    );
    Ok(())
}
