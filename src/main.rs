use std::{
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use anyhow::Context;
use args::Args;
use config::{Config, Rule};
use crossbeam_channel::{self as channel, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};

mod args;
mod config;

// TODO:
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

fn main() -> anyhow::Result<()> {
    // init ///////////////////////////////////////////////////////////////////

    let args = Args {
        config_path: "examples/config.yml".into(),
    };

    let config = Arc::new(Config::new(&args)?);
    eprintln!("CONFIG: {:#?}", config);

    init_dirs(&config)?;

    // TODO: first scan ///////////////////////////////////////////////////////
    // - delete all broken symlinks in dest_dir (later, run this every config.clean_interval).
    // - scan all files recursively under watch_dir, create symlinks as appropriate.
    //   - if symlink already exists at Dest,
    //     - if points to orig file, do nothing.
    //     - if points to another file, TODO process error appropriately.

    // set up watchers ////////////////////////////////////////////////////////

    let (event_tx, event_rx) = channel::unbounded();

    start_watchers_for_each_watch_dir(&config, &event_tx)?;

    // disconnect channel by dropping sender
    drop(event_tx);

    // process one event at a time in main
    loop {
        if let Ok(event) = event_rx.recv() {
            handle_event(&config, &event)?;
        }
    }

    Ok(())
}

/// To be run at startup.
/// Initialize directories and catch errors early to prevent mild catastrophes.
fn init_dirs(config: &Config) -> anyhow::Result<()> {
    for rule in &config.rules {
        for path in &rule.watch {
            if !path.try_exists()? {
                eprintln!("PATH DOES NOT EXIST ({:?})", path);
                if get_setting!(config, rule, create_missing_directories) {
                    println!("Creating path: {:?}", path);
                    fs::create_dir_all(path).with_context(|| {
                        format!("failed to create symlink directory: {:?}", path)
                    })?;
                    println!("Created path at: {:?}", path);
                } else {
                    anyhow::bail!("path does not exist! terminating...");
                }
            }
        }
    }
    // let path = &config.dest_dir;
    // if !path.try_exists()? {
    //     eprintln!("PATH DOES NOT EXIST ({:?})", path);
    //     if config.create_missing_directories {
    //         println!("Creating path: {:?}", path);
    //         fs::create_dir_all(path)
    //             .with_context(|| format!("failed to create symlink directory: {:?}", path))?;
    //         println!("Created path at: {:?}", path);
    //     } else {
    //         anyhow::bail!("path does not exist! terminating...");
    //     }
    // }
    Ok(())
}

fn start_watchers_for_each_watch_dir(
    config: &Arc<Config>,
    tx: &Sender<Event>,
) -> anyhow::Result<()> {
    // start watcher for each watch_dir
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        for (watch_idx, _) in rule.watch.iter().enumerate() {
            let config_arc = Arc::clone(config);
            let tx: Sender<Event> = tx.clone();
            thread::spawn(move || -> anyhow::Result<()> {
                start_watcher(config_arc, rule_idx, watch_idx, tx)
            });
        }
    }
    Ok(())
}

fn start_watcher(
    config: Arc<Config>,
    rule_idx: usize,
    watch_idx: usize,
    tx: Sender<Event>,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch[watch_idx];

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => match tx.send(event) {
            Ok(x) => println!("SENT!!!: {:?}", x),
            Err(e) => println!("FAILED TO SEND: {:?}", e),
        },
        Err(e) => {
            println!("watch error...: {}", e);
        }
    })?;

    println!("Starting watcher at: {:?}", watch);
    watcher.watch(watch, RecursiveMode::Recursive)?;

    // Keep the watcher alive - it will send events via the closure
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

// fn create_watcher() {}

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
    println!("DEBUG: handle path: {:?}", path);
    //     let filename = path
    //         .file_name()
    //         .with_context(|| format!("cannot get OsStr filename of path: {:?}", path))?
    //         .to_str()
    //         .with_context(|| format!("cannot convert OsStr to str for path: {:?}", path))?;

    //     if config.pattern.is_match(filename) {
    //         eprintln!("REGEX MATCHES!: {}", filename);
    //     } else {
    //         eprintln!("Regex does not match: {}", filename);
    //     }
    Ok(())
}

// /// Creates a symlink from orig_path to dest_dir.
// fn create_symlink(orig_path: &Path, dest_dir: &Path) -> anyhow::Result<()> {
//     eprintln!(
//         "Creating symlink from ({:?}) to ({:?})...",
//         orig_path, dest_dir
//     );
//     Ok(())
// }
